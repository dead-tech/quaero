use clap::Parser;
use std::fs::DirEntry;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Target to find
    target: String,
}

fn dir_entry_path<'a>(dir_entry: &DirEntry) -> String {
    dir_entry.path().into_os_string().into_string().unwrap()
}

fn visit_directory(directory: &str, callback: &dyn Fn(&DirEntry)) -> Result<(), std::io::Error> {
    for entry in std::fs::read_dir(directory)? {
        let entry = entry?;
        let file_type = entry.file_type()?;

        if file_type.is_dir() {
            let inner_directory = dir_entry_path(&entry);
            visit_directory(&inner_directory, callback)?;
        } else {
            callback(&entry);
        }
    }

    Ok(())
}

fn main() -> Result<(), std::io::Error> {
    let cli = Cli::parse();

    let target = cli.target;

    // Now we need to walk the current working directory
    // and try to find any file that matches target.
    visit_directory(".", &|entry: &DirEntry| {
        let file_name = entry.file_name();
        let path = dir_entry_path(entry);
        if file_name == *target {
            println!("Found {target} in {path}");
        }
    })?;

    Ok(())
}
