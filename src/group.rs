use chrono::{DateTime, Local};
use std::collections::HashMap;
use std::{fs, io, path, sync};

const TYPE_TO_EXTS: &[(&str, &[&str])] = &[
    (
        "Images",
        &[
            "jpg", "jpeg", "png", "gif", "bmp", "tif", "tiff", "webp", "avif", "heic", "heif",
            "svg", "ico", "psd", "ai", "eps", "dng", "cr2", "nef", "arw", "raw",
        ],
    ),
    (
        "Books",
        &["epub", "mobi", "azw3", "fb2", "djvu", "cbz", "cbr"],
    ),
    (
        "Documents",
        &[
            "txt", "rtf", "md", "pdf", "doc", "docx", "odt", "pages", "tex", "html", "htm", "xls",
            "xlsx", "ods", "numbers", "ppt", "pptx", "odp", "keynote",
        ],
    ),
    (
        "Data",
        &[
            "csv", "tsv", "json", "jsonl", "ndjson", "xml", "yaml", "yml", "toml", "ini",
            "parquet", "avro", "orc", "db", "sqlite", "sqlite3",
        ],
    ),
    (
        "Archives",
        &[
            "zip", "rar", "7z", "tar", "gz", "tgz", "bz2", "xz", "zst", "iso",
        ],
    ),
    (
        "Code",
        &[
            "py", "ipynb", "sh", "bash", "zsh", "fish", "go", "rs", "lua", "c", "h", "cpp", "hpp",
            "cc", "java", "kt", "swift", "rb", "php", "pl", "r", "sql", "js", "mjs", "cjs", "ts",
            "tsx", "jsx", "css", "scss", "hs", "ex", "exs", "zig", "vim", "el", "patch", "diff",
        ],
    ),
    (
        "Apps",
        &[
            "dmg", "pkg", "app", "exe", "msi", "deb", "rpm", "appimage", "apk", "snap",
        ],
    ),
    ("Fonts", &["ttf", "otf", "ttc", "woff", "woff2"]),
    ("Keyboard Layouts", &["uf2", "keymap"]),
    ("Tracks", &["gpx", "tcx", "fit", "kml", "kmz"]),
    (
        "Videos",
        &[
            "mp4", "m4v", "mkv", "avi", "mov", "webm", "wmv", "flv", "mpg", "mpeg", "3gp",
        ],
    ),
    ("Torrents", &["torrent"]),
    (
        "Music",
        &[
            "mp3", "m4a", "aac", "flac", "alac", "wav", "aiff", "aif", "ogg", "opus", "wma", "mid",
            "midi",
        ],
    ),
];

static EXT_TO_TYPE: sync::LazyLock<HashMap<&str, &str>> = sync::LazyLock::new(|| {
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

        let mut ext_sorted = extensions.to_vec();
        ext_sorted.sort();

        res.push_str(ext_sorted.join(", ").as_str());
    }
    res.push_str(format!("\n{:<20} :: ", UNKNOWN_FILE_TYPE).as_str());
    res.push_str("<everything else>");
    res
}

/// Group files by the filetype
pub fn by_type(files: impl Iterator<Item = fs::DirEntry>) -> HashMap<String, Vec<path::PathBuf>> {
    let mut files_by_type: HashMap<String, Vec<path::PathBuf>> = HashMap::new();

    for file in files {
        let path = file.path();

        if !path.is_file() {
            continue;
        }
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("no_extension")
            .to_lowercase();
        let file_type = get_file_type(&ext);
        files_by_type.entry(file_type).or_default().push(path);
    }
    files_by_type
}

/// Group files by modification date
pub fn by_date(
    files: impl Iterator<Item = fs::DirEntry>,
) -> io::Result<HashMap<String, Vec<path::PathBuf>>> {
    let mut files_by_date: HashMap<String, Vec<path::PathBuf>> = HashMap::new();

    for file in files {
        let path = file.path();
        if !path.is_file() {
            continue;
        }
        let meta = path.metadata()?;

        let datetime: DateTime<Local> = meta.modified()?.into();
        let date_string: String = datetime.format("%d-%m-%Y").to_string();
        files_by_date.entry(date_string).or_default().push(path);
    }
    Ok(files_by_date)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::group::TYPE_TO_EXTS;

    #[test]
    fn no_ext_appears_in_two_categories() {
        let mut seen: HashMap<&str, &str> = HashMap::new();
        for (ftype, exts) in TYPE_TO_EXTS {
            for ext in *exts {
                if let Some(prev) = seen.insert(ext, ftype) {
                    panic!("'{ext}' appears in both {prev} and {ftype}");
                }
            }
        }
    }
}
