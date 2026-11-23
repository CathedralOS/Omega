use platform_custody::temporary_directory::TemporaryDirectory;
use std::{env::consts::EXE_SUFFIX, fs, process::Command};

#[test]
fn build_dir_accepts_one_destination_and_refuses_two() {
    let project = TemporaryDirectory::create("build-output-directory", false).unwrap();
    fs::write(project.path().join("main.omg"), "machine main() {}\n").unwrap();
    let build = |options: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_omega"))
            .current_dir(project.path())
            .args(options)
            .arg("main.omg")
            .output()
            .unwrap()
    };

    let ambiguous = build(&["build", "--build-dir", "first", "--build-dir", "second"]);
    assert_eq!(ambiguous.status.code(), Some(64), "{ambiguous:?}");
    assert!(
        String::from_utf8_lossy(&ambiguous.stderr).starts_with("duplicate --build-dir\n"),
        "{ambiguous:?}"
    );
    for path in ["first", "second"] {
        assert!(!project.path().join(path).exists(), "{path}");
    }

    let single = build(&["build", "--build-dir", "first"]);
    assert!(single.status.success(), "{single:?}");
    assert!(
        project
            .path()
            .join("first")
            .join(format!("omega-program{EXE_SUFFIX}"))
            .is_file()
    );
}
