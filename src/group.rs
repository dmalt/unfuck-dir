use chrono::{DateTime, Local};
use std::collections::HashMap;
use std::fs::ReadDir;
use std::path::PathBuf;
use std::sync::LazyLock;

const TYPE_TO_EXTS: &[(&str, &[&str])] = &[
    ("Images", &["jpg", "jpeg", "png", "svg", "gif", "ai"]),
    ("Books", &["epub", "mobi"]),
    ("Documents", &["txt", "pdf", "md", "docx", "doc", "html"]),
    ("Data", &["csv", "parquet", "xml"]),
    ("Archives", &["zip", "rar", "tar", "gz"]),
    ("Code", &["py", "sh"]),
    ("Apps", &["dmg"]),
    ("Keyboard Layouts", &["uf2", "keymap"]),
    ("Tracks", &["gpx"]),
];

static EXT_TO_TYPE: LazyLock<HashMap<&str, &str>> = LazyLock::new(|| {
    let mut ext_to_type = HashMap::new();
    for (file_type, extensions) in TYPE_TO_EXTS {
        for ext in *extensions {
            ext_to_type.insert(*ext, *file_type);
        }
    }
    ext_to_type
});

const UNKNOWN_FILE_TYPE: &str = "Other";

fn get_file_type(ext: &str) -> String {
    EXT_TO_TYPE
        .get(ext)
        .unwrap_or(&UNKNOWN_FILE_TYPE)
        .to_string()
}

pub fn format_type_to_exts() -> String {
    let mut res = String::new();

    let mut sorted: Vec<_> = TYPE_TO_EXTS.iter().collect();
    sorted.sort_by_key(|(file_type, _)| file_type);

    for (file_type, extensions) in sorted {
        res.push_str(format!("\n{:<20} :: ", file_type).as_str());

        let mut ext_sorted: Vec<_> = extensions.iter().collect();
        ext_sorted.sort();

        res.push_str(extensions.join(", ").as_str());
    }
    res.push_str(format!("\n{:<20} :: ", UNKNOWN_FILE_TYPE).as_str());
    res.push_str("<everything else>");
    res
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
        let file_type = get_file_type(ext);
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
