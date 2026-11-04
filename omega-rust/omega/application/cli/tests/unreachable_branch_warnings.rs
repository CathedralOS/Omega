//! The unreachable-branch warning is advisory and reaches the author on the
//! compiler's warning channel, standard error, while the program still
//! compiles. These checks run `omega --check` on the focused fixtures under
//! `tests/omega/pass/contracts/` and count the warnings each one reports: a
//! branch that contradicts its machine's entry requirement warns once, while
//! consistent branches and alternatives removed by generic specialization stay
//! quiet. Whether each fixture runs, and agrees between the Terminal Psi
//! interpreter and native code, is the corpus gate's question, not this one.

use std::path::PathBuf;
use std::process::Command;

const WARNING: &str = "warning: unreachable branch:";

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../tests/omega/pass/contracts")
        .join(name)
}

/// The unreachable-branch warnings a successful check of `name` reports.
fn warnings(name: &str) -> Vec<String> {
    let directory = fixture(name);
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .arg("--check")
        .arg("main.omg")
        .current_dir(&directory)
        .output()
        .expect("omega runs");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "{name} must still compile: {stderr}"
    );
    stderr
        .lines()
        .filter(|line| line.trim_start().starts_with(WARNING))
        .map(str::to_owned)
        .collect()
}

#[track_caller]
fn assert_one_warning(name: &str, fragments: &[&str]) {
    let reported = warnings(name);
    assert_eq!(reported.len(), 1, "{name}: {reported:?}");
    for fragment in fragments {
        assert!(
            reported[0].contains(fragment),
            "{name}: {reported:?} lacks {fragment}"
        );
    }
}

#[test]
fn a_transition_arm_contradicting_the_entry_requirement_warns() {
    for name in [
        "contradictory_branch_closes_obligation_exit",
        "contradictory_branch_call_admission_exit",
        "contradictory_branch_result_guarantee_exit",
    ] {
        assert_one_warning(
            name,
            &[
                "the `true` arm of a transition in machine `pick`",
                "count >= 10",
            ],
        );
    }
}

#[test]
fn a_relational_contradiction_names_both_values() {
    assert_one_warning(
        "contradictory_relational_branch_exit",
        &["the `true` arm of a transition", "high >= low"],
    );
}

#[test]
fn a_match_arm_contradicting_the_entry_requirement_warns() {
    assert_one_warning(
        "contradictory_match_predecessor_join_exit",
        &["the `true` arm of a match", "count >= 10"],
    );
    assert_one_warning(
        "contradictory_boolean_match_arm_exit",
        &["the `false` arm of a match", "count >= 10"],
    );
}

#[test]
fn a_generic_contradiction_warns_once_for_the_authored_machine() {
    assert_one_warning(
        "generic_branch_contract_contradiction_exit",
        &["machine `pick` state `entry`"],
    );
}

#[test]
fn specialization_and_consistent_branches_stay_quiet() {
    for name in [
        "specialized_branch_selection_quiet_exit",
        "consistent_branches_quiet_exit",
    ] {
        assert_eq!(warnings(name), Vec::<String>::new(), "{name}");
    }
}
