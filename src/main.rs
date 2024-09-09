use anyhow::Result;
use clap::{Parser, ValueEnum};
use std::fs::DirEntry;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

enum FileModeMask {
    Executable = 0o111,
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum FileType {
    Directory,
    RegularFile,
    SymLink,
    Executable,
}

struct ParsedEntry {
    file_type: FileType,
    name: String,
    path: String,
}

impl From<DirEntry> for ParsedEntry {
    fn from(entry: DirEntry) -> Self {
        let file_type = entry.file_type().unwrap();
        let mut file_type = match (
            file_type.is_dir(),
            file_type.is_file(),
            file_type.is_symlink(),
        ) {
            (true, false, false) => FileType::Directory,
            (false, true, false) => FileType::RegularFile,
            (false, false, true) => FileType::SymLink,
            _ => unreachable!(),
        };
            
        let permissions = entry.metadata().unwrap().permissions();
        if file_type != FileType::Directory && permissions.mode() & FileModeMask::Executable as u32 != 0 {
            file_type = FileType::Executable 
        }

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

        // FIXME: we should call the callback also on directories
        // before traversing the latter, because we might be looking
        // for a directory.
        match entry.file_type {
            FileType::Directory => walk_directory(entry.path, callback)?,
            _ => callback(&entry),
        }
    }

    Ok(())
}

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Target to find
    target: String,

    /// Directory from where to start searching
    #[clap(default_value = ".")]
    start_directory: String,

    /// File type to look for
    #[clap(long, short, value_enum, default_value_t = FileType::RegularFile)]
    file_type: FileType,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let target = cli.target;
    let start_directory = cli.start_directory;
    let target_type = cli.file_type;

    walk_directory(start_directory, &|entry: &ParsedEntry| {
        if entry.name == *target && entry.file_type == target_type {
            println!("Found {} in {}", target, entry.path);
        }
    })?;

    Ok(())
}
