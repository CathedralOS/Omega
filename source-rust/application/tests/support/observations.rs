#[path = "project.rs"]
pub(super) mod project;

use platform_custody::temporary_directory::TemporaryDirectory;
use std::process::{Command, Output, Stdio};

pub fn run(scratch: &TemporaryDirectory, arguments: &[&str], inputs: &[&str]) -> (Output, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(arguments)
        .args(inputs)
        .current_dir(scratch.path())
        .stdin(Stdio::null())
        .output()
        .expect("the compiler runs on the fixture");
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    eprintln!("{report}");
    (output, report)
}

/// These programs return Unit without producing output. Check the guest's
/// observation separately from the host command that captured it.
pub fn assert_silent_return(project: impl Fn() -> TemporaryDirectory, inputs: &[&str]) {
    let scratch = project();
    let (output, report) = run(&scratch, &["run", "--outputs", "observed"], inputs);
    assert!(output.status.success(), "{report}");
    let observations = scratch.path().join("observed");
    for stream in ["stdout", "stderr"] {
        let bytes = std::fs::read(observations.join(stream)).unwrap();
        assert!(bytes.is_empty(), "interpreted {stream}: {bytes:?}");
    }
    let termination = std::fs::read_to_string(observations.join("termination.tsv")).unwrap();
    assert_eq!(termination, "psi\treturn\n", "{report}");

    let scratch = project();
    let mut command = Command::new(env!("CARGO_BIN_EXE_omega"));
    command
        .arg("build")
        .args(inputs)
        .current_dir(scratch.path());
    let result = project::build_and_run_command(&mut command);
    assert!(result.compiler.status.success(), "{:?}", result.compiler);
    let compiled = result.execution.unwrap();
    assert!(compiled.status.success(), "{compiled:?}");
    assert!(
        compiled.stdout.is_empty() && compiled.stderr.is_empty(),
        "{compiled:?}"
    );
    let report = result.report;
    assert!(
        report.lines().any(|line| line == "process-exit-excluded"),
        "{report}"
    );
}
