use chrono::{DateTime, Local};
use std::collections::HashMap;
use std::fs::ReadDir;
use std::path::PathBuf;

fn get_file_type(ext: &str) -> &str {
    match ext {
        "jpg" | "jpeg" | "png" | "svg" | "gif" | "ai" | "webp" => "Images",
        "epub" | "mobi" => "Books",
        "txt" | "pdf" | "md" | "odp" | "xlsx" | "docx" | "doc" | "html" => "Documents",
        "csv" | "parquet" | "xml" => "Data",
        "zip" | "rar" | "tar" | "gz" => "Archives",
        "py" | "sh" | "rs" | "js" | "ts" | "tsx" => "Code",
        "dmg" => "Apps",
        "uf2" | "keymap" => "Keyboard Layouts",
        "gpx" => "Tracks",
        _ => "Other",
    }
}

/// Group files by the filetype
pub fn by_type(files: ReadDir) -> std::io::Result<HashMap<String, Vec<PathBuf>>> {
    let mut files_by_type: HashMap<String, Vec<PathBuf>> = HashMap::new();

    for file in files {
        let file = file?;
        let path = file.path();

        if !path.is_file() {
            continue;
        }
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("no_extension");
        let file_type = get_file_type(ext).to_string();
        files_by_type
            .entry(file_type)
            .or_insert(Vec::new())
            .push(path);
    }
    Ok(files_by_type)
}

/// Group files by modification date
pub fn by_date(files: ReadDir) -> std::io::Result<HashMap<String, Vec<PathBuf>>> {
    let mut files_by_date: HashMap<String, Vec<PathBuf>> = HashMap::new();

    for file in files {
        let file = file?;
        let path = file.path();
        if !path.is_file() {
            continue;
        }
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
