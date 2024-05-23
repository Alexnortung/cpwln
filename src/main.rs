use clap::{arg, command, Command};
use glob::glob;
use relative_path::RelativePath;
use std::{
    collections::HashMap,
    error::Error,
    fs::{self, Metadata},
    io,
    os::unix::fs::{symlink, MetadataExt},
};

struct SourceCounter {
    path: String,
    inode: u64,
    num_other_links: u64,
    paths_other_links: Vec<String>,
}

impl SourceCounter {
    fn new(path: String, inode: u64, num_other_links: u64) -> Self {
        SourceCounter {
            path,
            inode,
            num_other_links,
            paths_other_links: vec![],
        }
    }

    fn new_by_stat(path: String, stat: &fs::Metadata) -> Self {
        SourceCounter {
            path,
            inode: stat.ino(),
            num_other_links: stat.nlink(),
            paths_other_links: vec![],
        }
    }

    fn get_remaning_other_links(&self) -> u64 {
        self.num_other_links - self.paths_other_links.len() as u64
    }

    fn add_path_other_link(&mut self, path: String) {
        if self.paths_other_links.contains(&path) || self.path == path {
            return;
        }
        self.paths_other_links.push(path);
    }

    fn is_all_links_found(&self) -> bool {
        self.get_remaning_other_links() == 0
    }
}

fn cli() -> Command {
    command!()
        .arg(arg!(<search> "A glob pattern for where to search for the links"))
        .arg(arg!(<source> ... "The source file or directory to copy."))
        .arg(arg!(<destination> "The destination file or directory to copy to."))
}

type INodeCounterMap = HashMap<u64, SourceCounter>;

fn search_and_count(
    search: &str,
    mut counters: INodeCounterMap,
) -> Result<INodeCounterMap, Box<dyn Error>> {
    for entry in glob(search).expect("Failed to read glob pattern") {
        let entry = entry?;
        let metadata = entry.metadata()?;
        let inode = metadata.ino();

        if let Some(counter) = counters.get_mut(&inode) {
            counter.add_path_other_link(entry.to_string_lossy().to_string());
        }
    }

    Ok(counters)
}

/// Replaces destination file with a symlink to the source file
/// If the destination is a directory, it will symlink the source file into the directory
fn replace_with_symlink(source: &str, destination: &str) -> Result<(), std::io::Error> {
    // If the destination is a directory
    let destination_metadata = fs::metadata(destination)?;

    let relative_source = RelativePath::new(source);

    if destination_metadata.is_dir() {
        let relative_destination_dir = RelativePath::new(destination);
        let relative_destination =
            relative_destination_dir.join(relative_source.file_name().unwrap());

        let relative_path_object = relative_destination_dir.relative(relative_source);
        let relative_path = relative_path_object.to_string();
        let destination = relative_destination.to_string();

        // FIXME: Make work cross platform
        return symlink(relative_path, destination);
    }

    fs::remove_file(destination)?;

    let relative_destination = RelativePath::new(destination);
    let relative_destination_dir = relative_destination.parent().unwrap();

    let relative_path_object = relative_destination_dir.relative(relative_source);
    let relative_path = relative_path_object.to_string();
    let destination = relative_destination.to_string();

    // FIXME: Make work cross platform
    symlink(relative_path, destination)
}

#[allow(clippy::needless_pass_by_value)]
fn move_counter(source: SourceCounter, destination: &str) -> Result<(), Box<dyn Error>> {
    let destination_file_path = match fs::metadata(destination) {
        Ok(metadata) => {
            if metadata.is_dir() {
                let destination_dir_str = destination;
                let relative_source = RelativePath::new(&source.path);
                let destination = RelativePath::new(destination_dir_str)
                    .join(relative_source.file_name().unwrap());
                destination.to_string()
            } else {
                destination.to_string()
            }
        }
        Err(_) => destination.to_string(),
    };
    let destination_str = destination_file_path.as_str();
    let copy_result = fs::copy(source.path.as_str(), destination_str);
    if let Err(err) = copy_result {
        return Err(Box::new(err));
    }

    replace_with_symlink(destination_str, source.path.as_str())?;

    for path in source.paths_other_links {
        replace_with_symlink(destination_str, path.as_str())?;
    }

    Ok(())
}

fn ensure_dir(path: &str) -> Result<(), Box<dyn Error>> {
    let metadata_option = fs::metadata(path);
    if metadata_option.is_err() {
        fs::create_dir_all(path)?;

        return Ok(());
    }

    let metadata = metadata_option.unwrap();

    if !metadata.is_dir() {
        fs::remove_file(path)?;
        fs::create_dir_all(path)?;
    }

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let matches = cli().get_matches_from(wild::args());

    let search = matches
        .get_one::<String>("search")
        .expect("No search provided.");
    let source = matches
        .get_many::<String>("source")
        .expect("No source provided.");
    let destination = matches
        .get_one::<String>("destination")
        .expect("No destination provided.");

    // Stat source files
    // And check if the "file" is a directory, if it is a directory, it is not support for now
    let source_files: Vec<Result<(&String, Metadata), _>> = source
        .map(|s| {
            let metadata = fs::metadata(s).expect("Failed to read metadata");
            if metadata.is_dir() {
                // panic!("Directories are not supported yet.");
                return Err("Directories are not supported yet.");
            }

            if !metadata.is_file() {
                return Err("Only files are supported at the moment");
            }

            Ok((s, metadata))
        })
        .collect();

    if let Some(err) = source_files.iter().find(|x| x.is_err()) {
        return Err(Box::new(io::Error::new(
            io::ErrorKind::Other,
            err.to_owned().unwrap_err(),
        )));
    }

    let source_files_unwrapped: Vec<&(&String, Metadata)> =
        source_files.iter().map(|x| x.as_ref().unwrap()).collect();

    let counters = source_files_unwrapped.iter().map(|(path, s)| {
        let inode = s.ino();
        // The file itself is also a links, so to find other links, we need to subtract one
        let num_other_links = s.nlink() - 1;
        let path = (*path).to_string();

        SourceCounter {
            path,
            inode,
            num_other_links,
            paths_other_links: vec![],
        }
    });

    let is_multiple_sources = source_files_unwrapped.len() > 1;

    let counter_map = counters
        .map(|c| (c.inode, c))
        .collect::<std::collections::HashMap<u64, SourceCounter>>();

    let updated_counters = search_and_count(search, counter_map)?;

    if updated_counters
        .iter()
        .any(|(_, c)| !c.is_all_links_found())
    {
        return Err(Box::new(io::Error::new(
            io::ErrorKind::Other,
            "Not all links were found, try a broader search",
        )));
    }

    if is_multiple_sources {
        ensure_dir(destination)?;
    }

    for (_, counter) in updated_counters {
        move_counter(counter, destination)?;
    }

    Ok(())
}
