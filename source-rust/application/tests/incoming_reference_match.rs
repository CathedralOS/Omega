//! A value `match` whose arms each borrow an incoming record forms one loan
//! per arm, and the joined reference keeps every one of those loans live.
//! A call can borrow the joined reference in both engines whether the arms
//! name one incoming record or two. Because each arm's loan stays live, a
//! write through a lender while the joined reference is live is refused,
//! even the lender of the arm the program does not take.

mod support;

use std::process::{Command, Stdio};

fn run_both(fixture: &str) -> (bool, String) {
    let project = support::repository_root().join(fixture);
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(["run", "--both", "main.omg"])
        .current_dir(&project)
        .stdin(Stdio::null())
        .output()
        .expect("omega runs on the hosted target");
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let _ = std::fs::remove_dir_all(project.join("build"));
    let _ = std::fs::remove_file(project.join("omega.lock"));
    (output.status.success(), report)
}

fn assert_runs(fixture: &str) {
    let (success, report) = run_both(fixture);
    assert!(success, "{report}");
    for receipt in ["native exit 0", "interp exit: 0"] {
        assert!(report.lines().any(|line| line == receipt), "{report}");
    }
}

#[test]
fn a_match_of_one_incoming_reference_passes_to_a_borrowing_call() {
    assert_runs("source-rust/application/tests/fixtures/ownership/run/incoming_reference_match");
}

#[test]
fn a_match_of_two_incoming_references_passes_to_a_borrowing_call() {
    assert_runs(
        "source-rust/application/tests/fixtures/ownership/run/incoming_reference_match_two_lenders",
    );
}

#[test]
fn a_write_to_the_untaken_lender_while_the_match_is_live_is_refused() {
    let fixture = "source-rust/application/tests/fixtures/ownership/fail/incoming_reference_match_two_lenders_write";
    let expected = std::fs::read_to_string(
        support::repository_root()
            .join(fixture)
            .join("expected.txt"),
    )
    .unwrap();
    let (success, report) = run_both(fixture);
    assert!(!success, "{report}");
    assert!(report.contains(expected.trim()), "{report}");
}
