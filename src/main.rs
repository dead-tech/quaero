use anyhow::Result;
use clap::{Parser, ValueEnum};
use regex::Regex;
use std::ffi::OsStr;
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
}

impl TryFrom<DirEntry> for ParsedEntry {
    type Error = anyhow::Error;

    fn try_from(entry: DirEntry) -> Result<Self> {
        let file_type = FileType::try_from(&entry)?;

        Ok(Self {
            file_type,
            name: entry.file_name().into_string().unwrap(),
            path: entry.path().into_os_string().into_string().unwrap(),
        })
    }
}

fn walk_directory<T: AsRef<Path>>(
    directory: T,
    avoids: &Option<Vec<PathBuf>>,
    depth: usize,
    callback: &dyn Fn(&ParsedEntry),
) -> Result<()> {
    if depth <= 0 {
        return Ok(());
    }

    'outer: for entry in std::fs::read_dir(directory)? {
        let entry = ParsedEntry::try_from(entry?)?;

        if let Some(excludes) = avoids {
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
            walk_directory(entry.path, avoids, depth - 1, callback)?;
        }
    }

    Ok(())
}

enum SearchMode {
    Target,
    Type,
    TargetAndType,
    Extension,
    Regex,
}

fn match_target(target: &String, entry: &ParsedEntry) {
    if entry.name == *target {
        println!("{}", entry.path);
    }
}

fn match_type(target_type: &FileType, entry: &ParsedEntry) {
    if entry.file_type == *target_type {
        println!("{}", entry.path);
    }
}

fn extension_from_path(path: &String) -> Option<&str> {
    Path::new(path).extension().and_then(OsStr::to_str)
}

fn match_extensions(extensions: &Vec<String>, entry: &ParsedEntry) {
    let target_extension = extension_from_path(&entry.path);
    if let Some(target_extension) = target_extension {
        for extension in extensions {
            if extension == target_extension {
                println!("{}", entry.path);
            }
        }
    }
}

fn match_regex(regex: &Regex, entry: &ParsedEntry) {
    let path = &entry.path;
    if regex.is_match(path) {
        println!("{}", path);
    }
}

fn match_target_and_type(target: &String, target_type: &FileType, entry: &ParsedEntry) {
    if entry.name == *target && entry.file_type == *target_type {
        println!("{}", entry.path);
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
    #[clap(name = "avoid", long, short, num_args = 0.., value_delimiter = ' ')]
    avoids: Option<Vec<PathBuf>>,

    /// Extension to look for (without .)
    #[clap(name = "extension", long, short, num_args = 0.., value_delimiter = ' ')]
    extensions: Option<Vec<String>>,

    /// How many nested subdirectories to walk into
    #[clap(name = "depth", long, short)]
    depth: Option<usize>,

    #[clap(name = "regex", long, short)]
    regex: Option<Regex>,
}

fn deduce_search_mode(
    target: &Option<String>,
    target_type: &Option<FileType>,
    extensions: &Option<Vec<String>>,
    regex: &Option<Regex>,
) -> Result<SearchMode> {
    match (target, target_type, extensions, regex) {
        (Some(_), None, ..) => return Ok(SearchMode::Target),
        (None, Some(_), ..) => return Ok(SearchMode::Type),
        (.., Some(_), _) => return Ok(SearchMode::Extension),
        (.., Some(_)) => return Ok(SearchMode::Regex),
        (Some(_), Some(_), ..) => return Ok(SearchMode::TargetAndType),
        _ => {
            return Err(anyhow::anyhow!(
                "Either a target to find or a file type to search must be specified"
            ))
        }
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let target = cli.target;
    let start_directory = cli.start_directory;
    let target_type = cli.file_type;
    let avoids = cli.avoids;
    let extensions = cli.extensions;
    let depth = cli.depth.unwrap_or(std::usize::MAX);
    let regex = cli.regex;

    let search_mode = deduce_search_mode(&target, &target_type, &extensions, &regex)?;

    walk_directory(
        start_directory,
        &avoids,
        depth,
        &|entry: &ParsedEntry| match search_mode {
            SearchMode::Target => match_target(&target.as_ref().unwrap(), entry),
            SearchMode::Type => match_type(&target_type.unwrap(), entry),
            SearchMode::Extension => match_extensions(&extensions.as_ref().unwrap(), entry),
            SearchMode::Regex => match_regex(&regex.as_ref().unwrap(), entry),
            SearchMode::TargetAndType => {
                match_target_and_type(target.as_ref().unwrap(), &target_type.unwrap(), entry)
            }
        },
    )?;

    Ok(())
}
