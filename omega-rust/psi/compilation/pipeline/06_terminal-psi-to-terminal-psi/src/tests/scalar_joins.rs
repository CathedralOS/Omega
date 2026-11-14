//! Arithmetic values survive a join only through checked arrival predicates.

use super::build_purpose::judge_build;
use super::*;

const SOURCE: &str = include_str!(
    "../../../../../../../tests/fixtures/pass/contracts/contradictory_match_predecessor_join_exit/main.omg"
);

#[test]
fn an_arithmetic_match_arm_reaches_the_join_after_the_other_arm_is_ruled_out() {
    // Keep the fixture bytes intact. The test entry only selects its existing
    // main as a source-level compilation; ordinary Build/native proof is separate.
    let source = format!("{SOURCE}\nmachine build_entry() crashes Trap {{ main(); }}\n");
    let verdict = judge_build(&source).unwrap();
    assert!(matches!(verdict, RouteVerdict::Accepted(_)), "{verdict:?}");
}

fn assert_join_subtraction_refused(source: &str) {
    let source = format!("{source}\nmachine build_entry() crashes Trap {{ main(); }}\n");
    let verdict = judge_build(&source).unwrap();
    let RouteVerdict::Rejected { unproved, .. } = verdict else {
        panic!("{verdict:?}");
    };
    assert!(
        unproved.iter().any(|row| {
            matches!(row.kind, ObligationKind::OperationSafety { .. })
                && row.cause == UnprovedCause::NoDerivation
                && row
                    .statement
                    .contains("machine `pick` state `entry` return value")
        }),
        "{unproved:?}"
    );
}

#[test]
fn a_reachable_unsafe_predecessor_cannot_borrow_the_other_arms_arithmetic_value() {
    // The zero-valued arm is now reachable. Its actual arrival still owes
    // the post-join subtraction; the other arm cannot establish its bound.
    let source = SOURCE.replace("count >= 10", "count >= 0");
    assert_join_subtraction_refused(&source);
}

#[test]
fn overwriting_the_join_result_does_not_retain_its_previous_arithmetic_bound() {
    let source = SOURCE
        .replace("let chosen: u64", "let mut chosen: u64")
        .replace(
            "    chosen - 100u64",
            "    chosen = 0u64;\n    chosen - 100u64",
        );
    assert_join_subtraction_refused(&source);
}
