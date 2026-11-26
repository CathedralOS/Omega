//! Stage a scratch project from source text and run `omega` in it.
//!
//! Each test binary uses only some of these helpers.
#![allow(dead_code)]

use platform_custody::temporary_directory::TemporaryDirectory;
use std::path::Path;
use std::process::{Command, Output, Stdio};

/// Writes each `(relative path, contents)` pair, creating parent directories,
/// into a scratch directory named after `name`. The project is removed when
/// the returned directory is dropped.
pub fn stage(name: &str, files: &[(&str, &str)]) -> TemporaryDirectory {
    let scratch = TemporaryDirectory::create(name, false).unwrap();
    write(scratch.path(), files);
    scratch
}

pub fn write(root: &Path, files: &[(&str, &str)]) {
    for (path, contents) in files {
        let path = root.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    }
}

/// Runs `omega` in `directory` with stdin closed.
pub fn omega(directory: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(arguments)
        .current_dir(directory)
        .stdin(Stdio::null())
        .output()
        .expect("omega runs")
}

/// Whether `omega` succeeded, and its stderr.
pub fn status(output: &Output) -> (bool, String) {
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// Stages `files`, runs `omega` there, and returns whether it succeeded with its stderr.
pub fn check(name: &str, files: &[(&str, &str)], arguments: &[&str]) -> (bool, String) {
    status(&omega(stage(name, files).path(), arguments))
}
