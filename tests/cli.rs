use std::path::Path;
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_unfuck");

fn unfuck(home: &Path) -> Command {
    let mut cmd = Command::new(BIN);
    cmd.env("HOME", home);
    cmd.env_remove("XDG_STATE_HOME");
    cmd
}

#[test]
fn binary_runs() {
    let out = std::process::Command::new(BIN)
        .arg("--show-categories")
        .output()
        .unwrap();
    assert!(out.status.success());
}

#[cfg(unix)]
#[test]
fn state_is_written_under_the_overridden_home() {
    let home = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    std::fs::write(work.path().join("a.pdf"), b"x").unwrap();

    let out = unfuck(home.path())
        .current_dir(work.path())
        .args(["-p", "."])
        .output()
        .unwrap();
    assert!(out.status.success());

    assert!(home.path().read_dir().unwrap().next().is_some());
}
