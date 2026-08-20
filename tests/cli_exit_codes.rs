use std::process::Command;
#[test]
fn invalid_arguments_use_clap_error() {
    let code = Command::new(env!("CARGO_BIN_EXE_unlockm"))
        .arg("--jobs")
        .arg("0")
        .arg("does-not-exist")
        .status()
        .unwrap()
        .code()
        .unwrap();
    assert_ne!(code, 0);
}
#[test]
fn help_works() {
    assert!(Command::new(env!("CARGO_BIN_EXE_unlockm"))
        .arg("--help")
        .status()
        .unwrap()
        .success());
}
