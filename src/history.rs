use std::env;
use std::path::PathBuf;

const STATE_DIRNAME: &str = "unfk";

pub fn get_state_dir(os: &str) -> Option<PathBuf> {
    match os {
        "macos" => env::var("HOME")
            .map(|x| {
                PathBuf::from(x)
                    .join("Library")
                    .join("Application Support")
                    .join(STATE_DIRNAME)
            })
            .ok(),
        "linux" | "freebsd" | "openbsd" | "netbsd" | "dragonfly" => env::var("XDG_STATE_HOME")
            .ok()
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| {
                env::var("HOME")
                    .ok()
                    .map(|home| PathBuf::from(home).join(".local").join("state"))
            })
            .map(|x| x.join(STATE_DIRNAME)),
        "windows" => env::var("LOCALAPPDATA")
            .ok()
            .map(|p| PathBuf::from(p).join(STATE_DIRNAME)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn it_works() {
        hello();
    }
}
