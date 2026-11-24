use platform_custody::temporary_directory::TemporaryDirectory;
use std::process::Output;

#[path = "observations.rs"]
mod observations;

/// Each engine and checker receives its own copy of the source.
fn project(source: &str) -> TemporaryDirectory {
    let scratch = TemporaryDirectory::create("omega-test-program", false).unwrap();
    std::fs::write(scratch.path().join("main.omg"), source).unwrap();
    scratch
}

pub fn invoke(source: &str, arguments: &[&str]) -> (Output, String) {
    observations::run(&project(source), arguments, &["main.omg"])
}

pub fn assert_silent_return(source: &str) {
    observations::assert_silent_return(|| project(source), &["main.omg"]);
}
