/*

End-to-end CLI checks for advisory unreachable-branch warnings (`omega --check`).

An unreachable-branch warning is advisory: it is printed to `stderr` while compilation
still succeeds (`exit 0`). We run `omega --check` on the contract fixtures under
the CLI and shared `tests/fixtures/contracts/pass/` directories and count the `warning: unreachable branch:` lines:
- A transition or match arm that contradicts its machine's `requires` clause
  warns once.
- A contradiction inside a generic machine warns once for the authored machine
  rather than once per instantiation.
- A `_` arm after arms naming every declared case warns once.
- Consistent branches and branches pruned only after generic specialization stay
  quiet.

*/

#[path = "support/project.rs"]
mod project;
mod support;

use std::path::PathBuf;

const WARNING: &str = "warning: unreachable branch:";

fn fixture(name: &str) -> PathBuf {
    support::repository_root().join(name)
}

fn warnings(name: &str) -> Vec<String> {
    let directory = fixture(name);
    let output = project::omega(&directory, &["--check", "main.omg"]);
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
        "source-rust/application/tests/fixtures/contracts/pass/contradictory_branch_closes_obligation_exit",
        "source-rust/application/tests/fixtures/contracts/pass/contradictory_branch_call_admission_exit",
        "source-rust/application/tests/fixtures/contracts/pass/contradictory_branch_result_guarantee_exit",
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
        "source-rust/application/tests/fixtures/contracts/pass/contradictory_relational_branch_exit",
        &["the `true` arm of a transition", "low <= high"],
    );
}

#[test]
fn a_match_arm_contradicting_the_entry_requirement_warns() {
    assert_one_warning(
        "source-rust/application/tests/fixtures/contracts/pass/contradictory_boolean_match_arm_exit",
        &["the `false` arm of a match", "count >= 10"],
    );
}

/// The arm ruled out here feeds a join, and the subtraction after the join
/// needs `chosen >= 100` from the live arm's `count + 100u64`. Each actual
/// arrival proves the inferred relation; the impossible arm closes by
/// contradiction without lending its facts to the reachable arm.
#[test]
fn a_match_arm_ruled_out_before_a_join_warns() {
    assert_one_warning(
        "tests/fixtures/contracts/pass/contradictory_match_predecessor_join_exit",
        &["the `true` arm of a match", "count >= 10"],
    );
}

#[test]
fn a_generic_contradiction_warns_once_for_the_authored_machine() {
    assert_one_warning(
        "tests/fixtures/contracts/pass/generic_branch_contract_contradiction_exit",
        &["machine `pick` state `entry`"],
    );
}

#[test]
fn specialization_and_consistent_branches_stay_quiet() {
    for name in [
        "tests/fixtures/contracts/pass/specialized_branch_selection_quiet_exit",
        "source-rust/application/tests/fixtures/contracts/pass/consistent_branches_quiet_exit",
    ] {
        assert_eq!(warnings(name), Vec::<String>::new(), "{name}");
    }
}

/// A `_` arm after arms naming every declared case can never run: the
/// program is accepted and each such arm warns once, in a transition and in
/// a value `match`. A `_` arm some case reaches stays quiet.
#[test]
fn a_wildcard_after_every_declared_case_warns() {
    let reported =
        warnings("source-rust/application/tests/fixtures/contracts/pass/dead_wildcard_arm_exit");
    assert_eq!(reported.len(), 2, "{reported:?}");
    for (warning, machine) in reported.iter().zip(["`next`", "`weight`"]) {
        assert!(
            warning.contains("the `_` arm in machine ")
                && warning.contains(machine)
                && warning.contains("an earlier arm names every declared case"),
            "{reported:?}"
        );
    }
}

#[test]
fn recursive_branches_warn_once_at_each_declaring_machines_own_arm() {
    let directory =
        fixture("source-rust/application/tests/fixtures/contracts/pass/recursive_branch_locations");
    let output = project::omega(&directory, &["--timings", "--check", "main.omg"]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    for phase in ["Psi 06:", "selection admission:", "advisories:"] {
        let rows = stderr
            .lines()
            .filter(|line| line.contains(" ms  Product (") && line.contains(phase));
        assert_eq!(
            rows.count(),
            1,
            "{phase} must be measured exactly once: {stderr}"
        );
    }
    assert!(output.status.success(), "{stderr}");
    let mut lines = stderr.lines();
    let mut warnings = Vec::new();
    while let Some(line) = lines.next() {
        if line.trim_start().starts_with(WARNING) {
            warnings.push((line, lines.next().expect("source location")));
        }
    }
    assert_eq!(warnings.len(), 2, "{stderr}");
    for ((warning, location), (machine, position)) in warnings
        .into_iter()
        .zip([("first", "main.omg:4:40"), ("second", "main.omg:11:40")])
    {
        assert!(
            warning.contains(&format!("the `true` arm of a match in machine `{machine}`")),
            "{warning}"
        );
        assert!(location.ends_with(position), "{location}");
    }
}
