use platform_custody::temporary_directory::TemporaryDirectory;
use std::process::{Command, Output, Stdio};

/// Each engine and checker receives its own copy of the source.
fn project(source: &str) -> TemporaryDirectory {
    let scratch = TemporaryDirectory::create("omega-test-program", false).unwrap();
    std::fs::write(scratch.path().join("main.omg"), source).unwrap();
    scratch
}

pub fn invoke(source: &str, arguments: &[&str]) -> (Output, String) {
    run(&project(source), arguments)
}

fn run(scratch: &TemporaryDirectory, arguments: &[&str]) -> (Output, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(arguments)
        .arg("main.omg")
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
pub fn assert_silent_return(source: &str) {
    for engine in ["native", "psi"] {
        let scratch = project(source);
        let (output, report) = run(
            &scratch,
            &["run", "--engine", engine, "--outputs", "observed"],
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
