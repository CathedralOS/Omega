//! Run the file-import example with a file, a value and a type all named cell.
mod support;

use platform_custody::temporary_directory::TemporaryDirectory;
use std::process::{Command, Stdio};

#[test]
fn the_imported_callable_reads_the_stored_value() {
    let fixture = support::repository_root().join("samples/cli/probes/direct_file_imports");
    let scratch = TemporaryDirectory::create("direct-file-imports", false).unwrap();
    std::fs::create_dir(scratch.path().join("m")).unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_omega"));
    command.args(["run", "--both"]);
    for file in ["build.omg", "main.omg", "m/b.omg", "m/value-cell.omg"] {
        std::fs::copy(fixture.join(file), scratch.path().join(file)).unwrap();
        command.args(["--build-input", file]);
    }
    let output = command
        .arg("main.omg")
        .current_dir(scratch.path())
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.status.success(), "{report}");
    for receipt in ["native exit 0", "interp exit: 0"] {
        assert!(report.lines().any(|line| line == receipt), "{report}");
    }
}
