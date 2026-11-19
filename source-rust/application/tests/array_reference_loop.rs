//! Forwarding keeps all possible sources when a loop selects an array element.
//! Forming a new borrow still requires one exact parent.
use std::path::Path;
use std::process::{Command, Stdio};

fn observe(fixture: &str, arguments: &[&str]) -> (bool, String) {
    let project = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures")
        .join(fixture);
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(arguments)
        .current_dir(&project)
        .stdin(Stdio::null())
        .output()
        .unwrap_or_else(|error| panic!("run {fixture}: {error}"));
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    for path in [project.join("build"), project.join("omega.lock")] {
        let result = if path.is_dir() {
            std::fs::remove_dir_all(&path)
        } else {
            std::fs::remove_file(&path)
        };
        if let Err(error) = result {
            assert_eq!(
                error.kind(),
                std::io::ErrorKind::NotFound,
                "clean generated {}: {error}",
                path.display()
            );
        }
    }
    (output.status.success(), report)
}

#[test]
fn a_loop_keeps_the_selected_shape_in_native_and_psi_execution() {
    let (success, report) = observe(
        "run/references/array_reference_loop",
        &["run", "--both", "main.omg"],
    );
    assert!(success, "{report}");
    for receipt in ["native exit 0", "interp exit: 0"] {
        assert!(report.lines().any(|line| line == receipt), "{report}");
    }
}

#[test]
fn a_new_borrow_does_not_choose_an_arbitrary_incoming_parent() {
    let fixture = "fail/references/array_reference_loop_mutable_from_shared";
    let expected = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../tests/fixtures")
            .join(fixture)
            .join("expected.txt"),
    )
    .unwrap();
    assert!(!expected.trim().is_empty());
    let (success, report) = observe(fixture, &["--check", "main.omg"]);
    assert!(!success, "{report}");
    for fragment in expected.lines().filter(|line| !line.trim().is_empty()) {
        assert!(report.contains(fragment), "{report}");
    }
}
