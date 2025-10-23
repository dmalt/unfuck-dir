use chrono::{DateTime, Local};
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
        println!("{}", path.display());

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

fn main() -> std::io::Result<()> {
    let files = fs::read_dir("/Users/dmitriialtukhov/Downloads/")?;
    let by_date = false;
    let files_grouping = if by_date {
        group_by_date(files)?
    } else {
        group_by_type(files)?
    };
    println!("{:#?}", files_grouping);
    // println!("{:#?}", files);

    Ok(())
}
