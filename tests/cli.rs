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

#[cfg(unix)]
#[test]
fn undo_removes_folders_when_run_from_another_directory() {
    let home = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();

    std::fs::write(work.path().join("a.pdf"), b"hello").unwrap();

    let out = unfuck(home.path())
        .current_dir(work.path())
        .args(["-p", "."])
        .output()
        .unwrap();
    assert!(out.status.success());

    // check the forward pass moved the file
    assert!(work.path().join("Documents/a.pdf").exists());
    assert!(!work.path().join("a.pdf").exists());

    let out = unfuck(home.path())
        .current_dir(elsewhere.path())
        .arg("--undo")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "undo failed with {:?}\n--- stdtout ---\n{}\n--- stderr ---\n{}",
        out.status.code(),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        work.path().join("a.pdf").exists(),
        "the file should be restored"
    );
    assert!(
        !work.path().join("Documents").exists(),
        "created folder should be removed"
    );
}
