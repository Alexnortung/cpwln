use clap::{arg, command, Command};
use glob::glob;
use std::{error::Error, fs, io, os::unix::fs::MetadataExt};

struct SourceCounter<'a> {
    path: &'a str,
    inode: u64,
    num_other_links: u64,
    paths_other_links: Vec<String>,
}

fn cli() -> Command {
    command!()
        .arg(arg!(<search> "A glob pattern for where to search for the links"))
        .arg(arg!(<source> ... "The source file or directory to copy."))
        .arg(arg!(<destination> "The destination file or directory to copy to."))
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
    let source_files = source
        .map(|s| {
            let metadata = fs::metadata(s).expect("Failed to read metadata");
            if metadata.is_dir() {
                // panic!("Directories are not supported yet.");
                return Err("Directories are not supported yet.");
            }

            metadata
        })
        .collect();

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

    let xd = source_files.iter().map(|s| {
        let inode = s.ino();
        let num_other_links = s.nlink();
        let path = s.path().to_str().unwrap();

        SourceCounter {
            path,
            inode,
            num_other_links,
            paths_other_links: vec![],
        }
    });

    for entry in glob(search).expect("Failed to read glob pattern") {
        // if let Err(entry) = entry {
        //     println!("Error: {}", entry.into_error().to_string());
        //
        //     // return error
        //     return Err(Box::new(entry.into_error()));
        // }
        // let metadata = entry?.metadata()?;

        match entry {
            Ok(entry) => {
                let metadata = entry.metadata()?;
                let inode = metadata.ino();
                let num_other_links = metadata.nlink();
                let path = entry.display().to_string();

                println!("Path: {:?}", path);
                println!("Inode: {:?}", inode);
                println!("Num other links: {:?}", num_other_links);

                // Check if the inode is in the source files
                if source_files.iter().any(|s| s.ino() == inode) {
                    println!("Found source file: {:?}", path);
                } else {
                    println!("Found other link: {:?}", path);
                }
            }
            Err(e) => {
                // println!("Error: {}", e.to_string());
                return Err(Box::new(e));
            }
        }
    }

    Ok(())
}
