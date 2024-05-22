use clap::{arg, command, Command};
use glob::glob;
use relative_path::RelativePath;
use std::{
    collections::HashMap,
    error::Error,
    fs::{self, Metadata},
    io,
    os::unix::fs::{symlink, MetadataExt},
    path,
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
        if self.paths_other_links.contains(&path) {
            return;
        } else if self.path == path {
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

fn create_source_counters_from_source_files<'a>(
    source_files: impl Iterator<Item = &'a str>,
) -> Result<Vec<SourceCounter>, Box<dyn Error>> {
    let mut source_counters = Vec::<SourceCounter>::with_capacity(source_files.size_hint().0);
    for source_file in source_files {
        let metadata_result = fs::metadata(source_file);
        if let Err(err) = metadata_result {
            // return Err(Box::new(err));
            panic!("Error: {}", err);
        }
        let metadata = metadata_result.unwrap();

        if !metadata.is_file() {
            return Err(Box::new(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Only files are supported at the moment",
            )));
        }
    }

    Ok(vec![])
}

fn stat_source_files(source_files: Vec<&str>) -> Result<(), Box<dyn Error>> {
    for source_file in source_files.iter() {
        let metadata_result = fs::metadata(source_file);
        // if metadata_result.is_err() {
        if let Err(err) = metadata_result {
            return Err(Box::new(err));
        }
        let metadata = metadata_result.unwrap();
        let inode = metadata.ino();
        let num_other_links = metadata.nlink();

        // println!("Source file: {:?}", source_file);
        // println!("Inode: {:?}", inode);
        // println!("Num other links: {:?}", num_other_links);
    }

    Ok(())
}

fn search_and_count(
    search: &str,
    mut counters: HashMap<u64, SourceCounter>,
) -> Result<HashMap<u64, SourceCounter>, Box<dyn Error>> {
    // println!("Search: {:?}", search);
    for entry in glob(search).expect("Failed to read glob pattern") {
        // println!("Entry: {:?}", entry);
        match entry {
            Ok(entry) => {
                let metadata = entry.metadata();
                match metadata {
                    Ok(metadata) => {
                        let inode = metadata.ino();

                        if let Some(counter) = counters.get_mut(&inode) {
                            counter.add_path_other_link(entry.to_string_lossy().to_string());
                        }
                    }
                    Err(e) => {
                        return Err(Box::new(e));
                    }
                }
            }
            Err(e) => {
                return Err(Box::new(e));
            }
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
        // println!("Destination is a directory: {:?}", destination);
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

    for path in source.paths_other_links.iter() {
        replace_with_symlink(&destination_str, path.as_str())?;
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

    // println!("Search: {:?}", search);
    // println!("Source: {:?}", source.clone().collect::<Vec<_>>());
    // println!("Destination: {:?}", destination);

    // Stat source files
    // And check if the "file" is a directory, if it is a directory, it is not support for now
    // let source_files: Vec<_> = source.map(|s| (s, fs::metadata(s))).collect();
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
        // .filter_map(|x| if x.is_ok() { Some(x.unwrap()) } else { None })
        .collect();

    if let Some(err) = source_files.iter().find(|x| x.is_err()) {
        return Err(Box::new(io::Error::new(
            io::ErrorKind::Other,
            err.to_owned().unwrap_err(),
        )));
    }

    let source_files_unwrapped: Vec<&(&String, Metadata)> =
        source_files.iter().map(|x| x.as_ref().unwrap()).collect();

    // println!("Source files: {:?}", source_files.first().unwrap().ino());
    // println!("Source files: {:?}", source_files.first().unwrap().nlink());
    // println!("Source files: {:?}", source_files.first().unwrap().dev());

    // for source_file in source_files.iter() {
    //     let inode = source_file.ino();
    //     let num_other_links = source_file.nlink();
    //     let path = source_file.path().to_str().unwrap();
    //
    //     println!("Source file: {:?}", path);
    //     println!("Inode: {:?}", inode);
    //     println!("Num other links: {:?}", num_other_links);
    // }

    let counters = source_files_unwrapped.iter().map(|(path, s)| {
        let inode = s.ino();
        // The file itself is also a links, so to find other links, we need to subtract one
        let num_other_links = s.nlink() - 1;
        let path = path.to_string();

        // println!("Source file: {:?}. links: {}", path, num_other_links);

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

    for (_, counter) in updated_counters.into_iter() {
        move_counter(counter, destination)?;
    }

    Ok(())
}
