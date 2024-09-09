use anyhow::Result;
use clap::Parser;
use std::fs::DirEntry;
use std::path::Path;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Target to find
    target: String,
}

enum FileType {
    Directory,
    RegularFile,
    SymLink,
}

struct ParsedEntry {
    file_type: FileType,
    name: String,
    path: String,
}

impl From<DirEntry> for ParsedEntry {
    fn from(entry: DirEntry) -> Self {
        let file_type = entry.file_type().unwrap();
        let file_type = match (file_type.is_dir(), file_type.is_file(), file_type.is_symlink()) {
            (true, false, false) => FileType::Directory,
            (false, true, false) => FileType::RegularFile,
            (false, false, true) => FileType::SymLink,
            _ => unreachable!(),
        };

        Self {
            file_type,
            name: entry.file_name().into_string().unwrap(),
            path: entry.path().into_os_string().into_string().unwrap(),
        }
    }
}


fn walk_directory<T: AsRef<Path>>(directory: T, callback: &dyn Fn(&ParsedEntry)) -> Result<()> {
    for entry in std::fs::read_dir(directory)? {
        let entry = ParsedEntry::from(entry?);

        match entry.file_type {
            FileType::Directory => walk_directory(entry.path, callback)?,
            FileType::RegularFile | FileType::SymLink => callback(&entry),
        }
    }

    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let target = cli.target;

    // Now we need to walk the current working directory
    // and try to find any file that matches target.
    walk_directory(".", &|entry: &ParsedEntry | {
        if entry.name == *target {
            println!( "Found {} in {}", target, entry.path);
        }
    })?;

    Ok(())
}
