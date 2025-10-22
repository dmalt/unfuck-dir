use std::fs;
use chrono::{DateTime, Local};

fn main() -> std::io::Result<()> {
    let files = fs::read_dir("/Users/dmitriialtukhov/Downloads/")?;

    for file in files {
        let file = file?;
        let path = file.path();
        println!("{}", path.display());
        let meta = path.metadata()?;
        let datetime: DateTime<Local> = meta.modified()?.into();
        println!("{}", datetime.format("%d-%m-%Y"));
    }
    Ok(())
}
