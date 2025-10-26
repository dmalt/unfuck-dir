use chrono::{DateTime, Local};
use clap::{Parser, ValueEnum};
use std::collections::HashMap;
use std::fs::ReadDir;
use std::{fs, path::PathBuf};

fn group_by_type(files: ReadDir) -> std::io::Result<HashMap<String, Vec<PathBuf>>> {
    let mut files_by_type: HashMap<String, Vec<PathBuf>> = HashMap::new();
    let ext2type = HashMap::from([
        ("jpg", "Images"),
        ("jpeg", "Images"),
        ("png", "Images"),
        ("svg", "Images"),
        ("gif", "Images"),
        ("ai", "Images"),
        ("epub", "Books"),
        ("mobi", "Books"),
        ("txt", "Documents"),
        ("pdf", "Documents"),
        ("md", "Documents"),
        ("odp", "Documents"),
        ("xlsx", "Documents"),
        ("docx", "Documents"),
        ("doc", "Documents"),
        ("html", "Documents"),
        ("csv", "Data"),
        ("parquet", "Data"),
        ("xml", "Data"),
        ("zip", "Archives"),
        ("rar", "Archives"),
        ("tar", "Archives"),
        ("gz", "Archives"),
        ("py", "Code"),
        ("sh", "Code"),
        ("dmg", "Apps"),
        ("uf2", "Keyboard Layouts"),
        ("keymap", "Keyboard Layouts"),
        ("gpx", "Tracks"),
    ]);

    for file in files {
        let file = file?;
        let path = file.path();

        let file_type = if path.is_file() {
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("no_extension");
            ext2type.get(ext).unwrap_or(&"Other").to_string()
        } else {
            "Folders".to_string()
        };
        files_by_type
            .entry(file_type)
            .or_insert(Vec::new())
            .push(path);
    }
    Ok(files_by_type)
}

fn group_by_date(files: ReadDir) -> std::io::Result<HashMap<String, Vec<PathBuf>>> {
    let mut files_by_date: HashMap<String, Vec<PathBuf>> = HashMap::new();

    for file in files {
        let file = file?;
        let path = file.path();
        let meta = path.metadata()?;

        let datetime: DateTime<Local> = meta.modified()?.into();
        let date_string: String = datetime.format("%d-%m-%Y").to_string();
        files_by_date
            .entry(date_string)
            .or_insert(Vec::new())
            .push(path);
    }
    Ok(files_by_date)
}

fn move_files(files_grouping: HashMap<String, Vec<PathBuf>>, path: PathBuf) -> std::io::Result<()> {
    for (dirname, group_files) in &files_grouping {
        if dirname == "Folders" {
            continue;
        }
        let folder_path = path.join(dirname);

        std::fs::create_dir(&folder_path).ok();
        for file in group_files {
            let fname = file.file_name().unwrap();
            println!(
                "Moving {:#?} -> {:#?}",
                fname,
                folder_path.file_name().unwrap()
            );
            let dst = folder_path.join(&fname);
            std::fs::rename(&file, &dst)?;
        }
    }
    Ok(())
}

fn expand_tilde(path: &str) -> PathBuf {
    if path.starts_with("~") {
        let home = std::env::var("HOME").unwrap();
        PathBuf::from(path.replacen("~", &home, 1))
    } else {
        PathBuf::from(path)
    }
}

#[derive(Parser)]
#[command(name = "downloads-sorter")]
#[command(about = "Sort files by date or type")]
struct Args {
    /// Path to the folder to organize
    #[arg(short, long, default_value = "~/Downloads")]
    path: String,

    /// Group files by type or date
    #[arg(short, long, default_value = "type")]
    by: GroupMode,
}

#[derive(Clone, ValueEnum)]
enum GroupMode {
    Type,
    Date,
}

fn main() -> std::io::Result<()> {
    let args = Args::parse();

    let path = expand_tilde(&args.path);
    let files = fs::read_dir(&path)?;
    let files_grouping = match args.by {
        GroupMode::Date => group_by_date(files)?,
        GroupMode::Type => group_by_type(files)?,
    };
    move_files(files_grouping, path)?;
    // println!("{:#?}", files);

    Ok(())
}
