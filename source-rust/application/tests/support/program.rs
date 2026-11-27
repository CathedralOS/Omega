//! Stage a source program, or a fixture's `build.omg` and `main.omg`, and run
//! `omega` on it. Each test binary uses only some of these helpers.
#![allow(dead_code)]

use platform_custody::temporary_directory::TemporaryDirectory;
use std::path::Path;
use std::process::Output;

#[path = "observations.rs"]
mod observations;

const BUILD_INPUTS: &[&str] = &[
    "--build-input",
    "build.omg",
    "--build-input",
    "main.omg",
    "build.omg",
];

/// Each engine and checker receives its own copy of the source.
fn project(source: &str) -> TemporaryDirectory {
    let scratch = TemporaryDirectory::create("omega-test-program", false).unwrap();
    std::fs::write(scratch.path().join("main.omg"), source).unwrap();
    scratch
}

fn fixture_project(fixture: &Path) -> TemporaryDirectory {
    let scratch = TemporaryDirectory::create("omega-build-fixture", false).unwrap();
    for name in ["build.omg", "main.omg"] {
        std::fs::copy(fixture.join(name), scratch.path().join(name)).unwrap();
    }
    scratch
}

pub fn invoke(source: &str, arguments: &[&str]) -> (Output, String) {
    observations::run(&project(source), arguments, &["main.omg"])
}

pub fn assert_silent_return(source: &str) {
    observations::assert_silent_return(|| project(source), &["main.omg"]);
}

pub fn check_fixture(fixture: &Path) -> (Output, String) {
    observations::run(&fixture_project(fixture), &["--check"], BUILD_INPUTS)
}

pub fn assert_fixture_silent_return(fixture: &Path) {
    observations::assert_silent_return(|| fixture_project(fixture), BUILD_INPUTS);
}
