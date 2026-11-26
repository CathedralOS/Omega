//! Installed Build files use the same captured paths and file-local aliases as other sources.
mod support;

use platform_custody::temporary_directory::TemporaryDirectory;
use std::fs;
use std::path::Path;
use std::process::Command;

fn copy_directory(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let target = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_directory(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

#[test]
fn build_files_retain_their_paths_aliases_and_checked_scope() {
    let temp = TemporaryDirectory::create("installed-build-files", false).unwrap();
    let project = temp.path().join("app");
    let library = temp.path().join("library");
    copy_directory(&support::repository_root().join("source/library"), &library);
    fs::create_dir(&project).unwrap();
    let providers = library.join("build");
    for directory in ["left", "right"] {
        fs::create_dir_all(providers.join(directory)).unwrap();
    }
    fs::write(
        providers.join("inner.omg"),
        "public machine inner_value() -> u32 { 7 }",
    )
    .unwrap();
    fs::write(
        providers.join("other.omg"),
        "public machine inner_value() -> u32 { 11 }",
    )
    .unwrap();
    fs::write(
        providers.join("left/score.omg"),
        "public machine score() -> u32 { 3 }",
    )
    .unwrap();
    fs::write(
        providers.join("right/score.omg"),
        r#"use inner.omg as inside;
use other.omg as other;
public machine score() -> u32 { |inside|::inner_value() +% |other|::inner_value() }"#,
    )
    .unwrap();
    let build = r#"use {omega}/build/left/score.omg as left;
use {omega}/build/right/score.omg as right;
machine build(builder: @mutable Build) {
    Build::package(builder, "checks");
    transition |left|::score() == 3u32 && |right|::score() == 18u32 {
        true -> BuildLog::write_line(@mutable builder.log, "read both"),
        _ -> Build::package(builder, "duplicate")
    }
}"#;
    let main = "public machine answer() -> u32 { 42 }";
    let check = |build: &str, main: &str| {
        fs::write(project.join("build.omg"), build).unwrap();
        fs::write(project.join("main.omg"), main).unwrap();
        let result = Command::new(env!("CARGO_BIN_EXE_omega"))
            .current_dir(&project)
            .env("OMEGA_LIBRARY_ROOT", &library)
            .args(["--check", "build.omg"])
            .output()
            .unwrap();
        (
            result.status.success(),
            String::from_utf8_lossy(&result.stderr).into_owned(),
        )
    };
    let (ok, reason) = check(build, main);
    assert!(ok, "{reason}");
    let (ok, reason) = check(&build.replace("== 18u32", "== 17u32"), main);
    assert!(!ok && reason.contains("more than once"), "{reason}");
    let (ok, reason) = check(&build.replace("|right|::score()", "inner_value()"), main);
    assert!(
        !ok && reason.contains("no visible declaration in its source scope"),
        "{reason}"
    );
    let (ok, reason) = check(&build.replace("left/score.omg", "missing/score.omg"), main);
    assert!(!ok && reason.contains("missing/score.omg"), "{reason}");
    let (ok, reason) = check(
        build,
        "use {omega}/build/inner.omg; public machine value() -> u32 { |inner|::inner_value() }",
    );
    assert!(!ok && reason.contains("Build scope"), "{reason}");
}
