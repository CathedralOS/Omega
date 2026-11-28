//! Forwarding keeps all possible sources when a loop selects an array element.
//! Forming a new borrow still requires one exact parent.

#[path = "support/project.rs"]
mod project;
mod support;

fn observe(fixture: &str, arguments: &[&str]) -> (bool, String) {
    let project = support::repository_root().join(fixture);
    let output = project::observe(&project, arguments);
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
    for command in ["run", "build"] {
        let (success, report) = observe(
            "source-rust/application/tests/fixtures/references/run/array_reference_loop",
            &[command, "main.omg"],
        );
        assert!(success, "{report}");
    }
}

#[test]
fn a_new_borrow_does_not_choose_an_arbitrary_incoming_parent() {
    let fixture = "source-rust/application/tests/fixtures/references/fail/array_reference_loop_mutable_from_shared";
    let expected = std::fs::read_to_string(
        support::repository_root()
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
