use clap::{arg, command, Command};
use glob::glob;
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

        println!("Source file: {:?}", source_file);
        println!("Inode: {:?}", inode);
        println!("Num other links: {:?}", num_other_links);
    }

    Ok(())
}

fn search_and_count(
    search: &str,
    mut counters: HashMap<u64, SourceCounter>,
) -> Result<HashMap<u64, SourceCounter>, Box<dyn Error>> {
    for entry in glob(search).expect("Failed to read glob pattern") {
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

fn move_counter(source: SourceCounter, destination: &str) -> Result<(), Box<dyn Error>> {
    let copy_result = fs::copy(source.path.as_str(), destination);
    if let Err(err) = copy_result {
        return Err(Box::new(err));
    }
    let remove_result = fs::remove_file(source.path.as_str());
    if let Err(err) = remove_result {
        return Err(Box::new(err));
    }
    // FIXME: Make work cross platform
    symlink(destination, source.path.as_str());

    for path in source.paths_other_links.iter() {
        fs::remove_file(path);
        // FIXME: Make work cross platform
        symlink(destination, path);
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
        let num_other_links = s.nlink();
        let path = path.to_string();

        SourceCounter {
            path,
            inode,
            num_other_links,
            paths_other_links: vec![],
        }
    });

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

    for (_, counter) in updated_counters.into_iter() {
        move_counter(counter, destination)?;
    }

    Ok(())
}
