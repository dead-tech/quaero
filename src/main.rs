use anyhow::Result;
use clap::{Parser, ValueEnum};
use std::fs::DirEntry;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

enum FileModeMask {
    Executable = 0o111,
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum FileType {
    #[clap(name = "dir")]
    Directory,

    #[clap(name = "file")]
    RegularFile,
    
    #[clap(name = "link")]
    SymLink,

    #[clap(name = "exec")]
    Executable,
}

impl TryFrom<&DirEntry> for FileType {
    fn try_from(entry: &DirEntry) -> Result<Self> {
        let file_type = entry.file_type()?;

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

        let permissions = entry.metadata()?.permissions();
        if file_type != FileType::Directory
            && permissions.mode() & FileModeMask::Executable as u32 != 0
        {
            file_type = FileType::Executable;
        }

        Ok(file_type)
    }

    type Error = anyhow::Error;
}

struct ParsedEntry {
    file_type: FileType,
    name: String,
    path: String,
    display_path: PathBuf,
}

impl TryFrom<DirEntry> for ParsedEntry {
    type Error = anyhow::Error;

    fn try_from(entry: DirEntry) -> Result<Self> {
        let file_type = FileType::try_from(&entry)?;

        let display_path = match file_type {
            FileType::Directory => {
                let path = entry.path();
                let mut components = path.components();
                components.next_back();
                components.as_path().to_path_buf()
            }
            _ => entry.path(),
        };

        Ok(Self {
            file_type,
            name: entry.file_name().into_string().unwrap(),
            path: entry.path().into_os_string().into_string().unwrap(),
            display_path,
        })
    }
}

fn walk_directory<T: AsRef<Path>>(
    directory: T,
    excludes: &Option<Vec<PathBuf>>,
    callback: &dyn Fn(&ParsedEntry),
) -> Result<()> {
    'outer: for entry in std::fs::read_dir(directory)? {
        let entry = ParsedEntry::try_from(entry?)?;

        if let Some(excludes) = excludes {
            for exclude in excludes {
                let lhs = std::fs::canonicalize(&entry.path)?;
                let rhs = std::fs::canonicalize(exclude)?;
                if lhs.starts_with(rhs) {
                    continue 'outer;
                }
            }
        }

        callback(&entry);
        if entry.file_type == FileType::Directory {
            walk_directory(entry.path, excludes, callback)?;
        }
    }

    Ok(())
}

enum SearchMode {
    Target,
    Type,
    TargetAndType,
}

fn match_target(target: &String, entry: &ParsedEntry) {
    if entry.name == *target {
        println!(
            "Found {} in {}",
            target,
            entry.display_path.to_str().unwrap()
        );
    }
}

fn match_type(target_type: &FileType, entry: &ParsedEntry) {
    if entry.file_type == *target_type {
        println!(
            "Found {} in {}",
            entry.name,
            entry.display_path.to_str().unwrap()
        );
    }
}

fn match_target_and_type(target: &String, target_type: &FileType, entry: &ParsedEntry) {
    if entry.name == *target && entry.file_type == *target_type {
        println!(
            "Found {} in {}",
            target,
            entry.display_path.to_str().unwrap()
        );
    }
}

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Target to find
    target: Option<String>,

    /// Directory from where to start searching
    #[clap(default_value = ".")]
    start_directory: String,

    /// File type to look for
    #[clap(name = "type", long, short, value_enum)]
    file_type: Option<FileType>,

    /// Directories to avoid
    #[clap(name = "exclude", long, short, num_args = 0.., value_delimiter = ' ')]
    excludes: Option<Vec<PathBuf>>,
}

fn deduce_search_mode(
    target: &Option<String>,
    target_type: &Option<FileType>,
) -> Result<SearchMode> {
    match (target, target_type) {
        (Some(_), None) => return Ok(SearchMode::Target),
        (None, Some(_)) => return Ok(SearchMode::Type),
        (Some(_), Some(_)) => return Ok(SearchMode::TargetAndType),
        _ => {
            return Err(anyhow::anyhow!(
                "Either a target to find or a file type to search must be specified"
            ))
        }
    }
}

// TODO:
//   #1: Extension matching
//   #2: Raw output to be able to do command piping?
//   #4: Maximum recursion depth

fn main() -> Result<()> {
    let cli = Cli::parse();

    let target = cli.target;
    let start_directory = cli.start_directory;
    let target_type = cli.file_type;
    let excludes = cli.excludes;

    let search_mode = deduce_search_mode(&target, &target_type)?;

    walk_directory(
        start_directory,
        &excludes,
        &|entry: &ParsedEntry| match search_mode {
            SearchMode::Target => match_target(&target.as_ref().unwrap(), entry),
            SearchMode::Type => match_type(&target_type.unwrap(), entry),
            SearchMode::TargetAndType => {
                match_target_and_type(target.as_ref().unwrap(), &target_type.unwrap(), entry)
            }
        },
    )?;

    Ok(())
}
