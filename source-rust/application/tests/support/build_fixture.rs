use platform_custody::temporary_directory::TemporaryDirectory;
use std::path::Path;
use std::process::Output;

#[path = "observations.rs"]
mod observations;

const INPUTS: &[&str] = &[
    "--build-input",
    "build.omg",
    "--build-input",
    "main.omg",
    "build.omg",
];

fn project(fixture: &Path) -> TemporaryDirectory {
    let scratch = TemporaryDirectory::create("omega-build-fixture", false).unwrap();
    for name in ["build.omg", "main.omg"] {
        std::fs::copy(fixture.join(name), scratch.path().join(name)).unwrap();
    }
    scratch
}

pub fn check(fixture: &Path) -> (Output, String) {
    observations::run(&project(fixture), &["--check"], INPUTS)
}

pub fn assert_silent_return(fixture: &Path) {
    observations::assert_silent_return(|| project(fixture), INPUTS);
}
