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
    for engine in ["native", "psi"] {
        let scratch = project();
        let (output, report) = run(
            &scratch,
            &["run", "--engine", engine, "--outputs", "observed"],
            inputs,
        );
        assert!(output.status.success(), "{engine}: {report}");
        let observations = scratch.path().join("observed");
        for stream in ["stdout", "stderr"] {
            let bytes = std::fs::read(observations.join(stream)).unwrap();
            assert!(bytes.is_empty(), "{engine} {stream}: {bytes:?}");
        }
        let termination = std::fs::read_to_string(observations.join("termination.tsv")).unwrap();
        if engine == "psi" {
            assert_eq!(termination, "psi\treturn\n", "{report}");
        } else {
            let columns: Vec<_> = termination.trim_end().split('\t').collect();
            assert!(
                matches!(
                    columns.as_slice(),
                    ["native", "exit", "0", _, "process-exit-excluded"]
                ),
                "{termination}"
            );
        }
    }
}
