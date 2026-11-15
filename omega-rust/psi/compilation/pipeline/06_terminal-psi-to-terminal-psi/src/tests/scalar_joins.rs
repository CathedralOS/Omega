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

/// `choose` returns `body`, under `requires true` so no caller's actual can
/// fix `x`, and promises `result <= bound`.
fn masked_choice(body: &str, bound: u64) -> RouteVerdict {
    let source = format!(
        "machine choose(x: u64, flag: bool) -> u64\nrequires true\n\
         ensures result <= {bound}u64\n{{\n{body}\n}}\n\
         machine build_entry() crashes Trap {{ let chosen: u64 = choose(build_x(), true); }}\n\
         machine build_x() -> u64 {{ 466u64 }}\n"
    );
    judge_build(&source).unwrap()
}

/// The false promise stays unproved. Whether that is a rejection or an
/// `Incomplete` verdict is the goal-form reading of `prove/reach.rs`, which
/// a mask outside a join reaches as an operation the rules do not bound.
fn assert_ensures_unproved(verdict: RouteVerdict) {
    let unproved = match verdict {
        RouteVerdict::Rejected { unproved, .. } => unproved,
        RouteVerdict::Incomplete(undecided) => undecided,
        other => panic!("{other:?}"),
    };
    assert!(
        unproved.iter().any(|row| row
            .statement
            .contains("ensures contract for exit from choose")),
        "{unproved:?}"
    );
}

#[test]
fn a_match_arms_masked_narrowing_keeps_its_bound_across_the_join() {
    // Each arm's bound comes from its own result equations (the mask, then
    // the casts); the join parameter only has those arms as unfolded
    // alternatives, so the arms' ranges must cross the join with them.
    let arms = |mask: u64, other: &str| {
        format!("match flag {{ true -> (x & {mask}u64) as u8 as u64, false -> {other}, }}")
    };
    for other in ["(x & 15u64) as u8 as u64", "x & 15u64", "3u64"] {
        let verdict = masked_choice(&arms(32, other), 32);
        assert!(
            matches!(verdict, RouteVerdict::Accepted(_)),
            "{other}: {verdict:?}"
        );
        // `x & 32` may be 32 itself.
        assert_ensures_unproved(masked_choice(&arms(32, other), 31));
    }
    // The mask alone, without casts, crosses the same way.
    let verdict = masked_choice("match flag { true -> x & 32u64, false -> x & 15u64, }", 32);
    assert!(matches!(verdict, RouteVerdict::Accepted(_)), "{verdict:?}");
    assert_ensures_unproved(masked_choice(
        "match flag { true -> x & 32u64, false -> x & 48u64, }",
        32,
    ));
}

#[test]
fn a_returned_masked_narrowing_is_summarized_by_its_bound() {
    // The summary's equation `result == (x & 32) as u8 as u64` casts a
    // term that is not a value, which has no canonical formation condition;
    // it is left out and the returned range is still proposed and proved.
    let verdict = masked_choice("(x & 32u64) as u8 as u64", 32);
    assert!(matches!(verdict, RouteVerdict::Accepted(_)), "{verdict:?}");
    assert_ensures_unproved(masked_choice("(x & 32u64) as u8 as u64", 31));
}
