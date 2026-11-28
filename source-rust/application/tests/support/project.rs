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

/// Build an executable outside the fixture, then launch it with the fixture as its working directory.
pub fn build_and_run(directory: &Path, arguments: &[&str]) -> Output {
    observe(directory, &[&["build"], arguments].concat())
}

/// Observe a CLI action, including the process outcome when exercising a built artifact.
pub fn observe(directory: &Path, arguments: &[&str]) -> Output {
    observe_command(
        Command::new(env!("CARGO_BIN_EXE_omega"))
            .args(arguments)
            .current_dir(directory)
            .stdin(Stdio::null()),
    )
}

pub fn observe_command(command: &mut Command) -> Output {
    if command.get_args().next() == Some(std::ffi::OsStr::new("build")) {
        let result = build_and_run_command(command);
        result.execution.unwrap_or(result.compiler)
    } else {
        command.output().expect("omega runs")
    }
}

pub struct BuiltProgram {
    pub compiler: Output,
    pub report: String,
    pub execution: Option<Output>,
}

/// Return compiler diagnostics separately from the executable's raw streams and status.
pub fn build_and_run_command(command: &mut Command) -> BuiltProgram {
    let scratch = TemporaryDirectory::create("fixture-executable", false).unwrap();
    let saved = command
        .get_args()
        .last()
        .and_then(|arg| Path::new(arg).extension())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("psi"));
    let report_path = scratch.path().join("build-report.txt");
    if !saved {
        command.arg("--report-file").arg(&report_path);
    }
    let build = command
        .arg("--build-dir")
        .arg(scratch.path().join("product"))
        .output()
        .expect("omega build runs");
    if !build.status.success() {
        return BuiltProgram {
            compiler: build,
            report: String::new(),
            execution: None,
        };
    }
    let (report, prefix) = if saved {
        (
            String::from_utf8(build.stdout.clone()).unwrap(),
            "executable ",
        )
    } else {
        (
            std::fs::read_to_string(report_path).unwrap(),
            "published native output to ",
        )
    };
    let executable = report
        .lines()
        .find_map(|line| line.strip_prefix(prefix))
        .unwrap_or_else(|| panic!("missing executable in build report: {report}"));
    let mut process = Command::new(executable);
    if let Some(directory) = command.get_current_dir() {
        process.current_dir(directory);
    }
    let result = process
        .stdin(Stdio::null())
        .output()
        .expect("built program runs");
    BuiltProgram {
        compiler: build,
        report,
        execution: Some(result),
    }
}

/// A working native supplier must return. Bound the run so a lock regression
/// fails the test even when both admission and execution would otherwise hang.
pub fn assert_native_returns(executable: &Path, case: &str) {
    let mut native = Command::new(executable).spawn().unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        if let Some(status) = native.try_wait().unwrap() {
            assert!(status.success(), "{case}: {status}");
            return;
        }
        if std::time::Instant::now() >= deadline {
            native.kill().unwrap();
            native.wait().unwrap();
            panic!("{case}: the selected native supplier did not return");
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}
