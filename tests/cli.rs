const BIN: &str = env!("CARGO_BIN_EXE_unfuck");

#[test]
fn binary_runs() {
    let out = std::process::Command::new(BIN)
        .arg("--show-categories")
        .output()
        .unwrap();
    assert!(out.status.success());
}
