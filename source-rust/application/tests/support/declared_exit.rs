use platform_custody::temporary_directory::TemporaryDirectory;
use std::process::{Command, Stdio};

/// Execute the fixture against its declared process status and compare the raw streams.
pub fn assert_declared_exit(project: &std::path::Path, entry: &str) {
    let expected: i32 = std::fs::read_to_string(project.join("expected_native_exit.txt"))
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    let scratch = TemporaryDirectory::create("application-observations", false).unwrap();
    let streams = ["native", "psi"].map(|engine| {
        let directory = scratch.path().join(engine);
        let output = Command::new(env!("CARGO_BIN_EXE_omega"))
            .args(["run", "--engine", engine, "--outputs"])
            .arg(&directory)
            .arg(entry)
            .current_dir(project)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{engine}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let termination = std::fs::read_to_string(directory.join("termination.tsv")).unwrap();
        let columns: Vec<_> = termination.trim_end().split('\t').collect();
        let actual = match columns.as_slice() {
            ["native", "exit", status, _, _] | ["psi", "exit", status] => {
                status.parse::<i32>().unwrap()
            }
            ["psi", "return"] => 0,
            _ => panic!("{engine}: {termination}"),
        };
        assert_eq!(actual, expected, "{termination}");
        ["stdout", "stderr"].map(|stream| std::fs::read(directory.join(stream)).unwrap())
    });
    assert_eq!(
        streams[0], streams[1],
        "native and Psi stream observations differ"
    );
}
