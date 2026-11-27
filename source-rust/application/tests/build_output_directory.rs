#[path = "support/project.rs"]
mod project;

use std::env::consts::EXE_SUFFIX;

#[test]
fn build_dir_accepts_one_destination_and_refuses_two() {
    let scratch = project::stage(
        "build-output-directory",
        &[("main.omg", "machine main() {}\n")],
    );
    let build =
        |options: &[&str]| project::omega(scratch.path(), &[options, &["main.omg"]].concat());

    let ambiguous = build(&["build", "--build-dir", "first", "--build-dir", "second"]);
    assert_eq!(ambiguous.status.code(), Some(64), "{ambiguous:?}");
    assert!(
        String::from_utf8_lossy(&ambiguous.stderr).starts_with("duplicate --build-dir\n"),
        "{ambiguous:?}"
    );
    for path in ["first", "second"] {
        assert!(!scratch.path().join(path).exists(), "{path}");
    }

    let single = build(&["build", "--build-dir", "first"]);
    assert!(single.status.success(), "{single:?}");
    assert!(
        scratch
            .path()
            .join("first")
            .join(format!("omega-program{EXE_SUFFIX}"))
            .is_file()
    );
}
