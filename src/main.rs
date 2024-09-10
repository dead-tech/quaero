use anyhow::Result;
use clap::{Parser, ValueEnum};
use regex::Regex;
use std::ffi::OsStr;
use std::fs::DirEntry;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

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
    type Error = anyhow::Error;

    fn try_from(entry: &DirEntry) -> Result<Self> {
        let file_type = entry.file_type()?;

        let is_dir = file_type.is_dir();
        let is_file = file_type.is_file();
        let is_symlink = file_type.is_symlink();

        let permissions = entry.metadata()?.permissions();
        let executable_mask = 0o111;
        let is_executable = permissions.mode() & executable_mask != 0;

        let file_type = match (is_dir, is_file, is_symlink) {
            (true, ..) => FileType::Directory,
            (_, true, _) if !is_executable => FileType::RegularFile,
            (.., true) if !is_executable => FileType::SymLink,
            (false, ..) if is_executable => FileType::Executable,
            _ => unreachable!(
                "a file can be either one of these: directory, regular file, symbolic link"
            ),
        };

        Ok(file_type)
    }
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
    callback: &impl Fn(&ParsedEntry),
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

fn match_target(target: &Option<String>, entry: &ParsedEntry) {
    if entry.name == target.as_deref().unwrap() {
        println!("{}", entry.path);
    }
}

fn match_type(target_type: &Option<FileType>, entry: &ParsedEntry) {
    if entry.file_type == target_type.unwrap() {
        println!("{}", entry.path);
    }
}

fn match_extensions(extensions: &Option<Vec<String>>, entry: &ParsedEntry) {
    let target_extension = Path::new(&entry.path).extension().and_then(OsStr::to_str);
    if let Some(target_extension) = target_extension {
        for extension in extensions.as_deref().unwrap() {
            if extension == target_extension {
                println!("{}", entry.path);
            }
        }
    }
}

fn match_regex(regex: &Option<Regex>, entry: &ParsedEntry) {
    let path = &entry.path;
    if regex.as_ref().unwrap().is_match(path) {
        println!("{}", path);
    }
}

fn match_target_and_type(
    target: &Option<String>,
    target_type: &Option<FileType>,
    entry: &ParsedEntry,
) {
    if entry.name == *target.as_deref().unwrap() && entry.file_type == target_type.unwrap() {
        println!("{}", entry.path);
    }
}

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Target to find
    target: Option<String>,

    /// Directory from where to start searching
    #[clap(name = "from", long, short)]
    start_directory: Option<String>,

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

fn deduce_search_mode(args: &Cli) -> Result<SearchMode> {
    match (&args.target, args.file_type, &args.extensions, &args.regex) {
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
    let args = Cli::parse();

    let search_mode = deduce_search_mode(&args)?;
    let dispatcher = |entry: &ParsedEntry| match search_mode {
        SearchMode::Target => match_target(&args.target, entry),
        SearchMode::Type => match_type(&args.file_type, entry),
        SearchMode::Extension => match_extensions(&args.extensions, entry),
        SearchMode::Regex => match_regex(&args.regex, entry),
        SearchMode::TargetAndType => match_target_and_type(&args.target, &args.file_type, entry),
    };

    let start_directory = args.start_directory.unwrap_or(".".to_owned());
    let depth = args.depth.unwrap_or(std::usize::MAX);
    walk_directory(start_directory, &args.avoids, depth, &dispatcher)?;

    Ok(())
}
