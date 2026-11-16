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

/// A guard over a call result, written with `&&`, which lowers to a join.
/// `subtrahend` is what the taken edge subtracts from the guarded value.
fn guarded_call_result(subtrahend: u64) -> String {
    format!(
        "public machine helper(val: u64) -> u64 {{ val }}
machine main() {{
    let s: u64 = helper(25u64);
    transition s >= 100u64 && s <= 200u64 {{
        true -> done(s - {subtrahend}u64)
        _ -> other()
    }}
    machine done(d: u64) {{}}
    machine other() {{}}
}}
machine build_entry() crashes Trap {{ main(); }}
"
    )
}

#[test]
fn a_guard_over_a_dominating_call_result_crosses_the_join() {
    // `s` is defined before the branch, so every arrival at the `&&` join
    // sees the same `s`. Before the join's scope admitted dominating values
    // its predicate fell to `Truth`, and `s - 100` was refused.
    let verdict = judge_build(&guarded_call_result(100)).unwrap();
    assert!(matches!(verdict, RouteVerdict::Accepted(_)), "{verdict:?}");
}

#[test]
fn a_guard_over_a_call_result_still_proves_only_what_it_says() {
    let verdict = judge_build(&guarded_call_result(101)).unwrap();
    let RouteVerdict::Rejected { unproved, .. } = verdict else {
        panic!("`s >= 100` cannot pay for `s - 101`: {verdict:?}");
    };
    assert!(
        unproved.iter().any(|row| {
            matches!(row.kind, ObligationKind::OperationSafety { .. })
                && row.statement.contains("machine `main`")
        }),
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

#[test]
fn a_padding_match_keeps_its_strict_bound() {
    // Test the library's exact arithmetic. Zero and the calculated padding
    // must each be strictly below the same alignment.
    const SOURCE: &str = r#"
machine c_abi_padding(offset: u64, alignment: u64) -> u64
reaches none terminates
requires alignment >= 1 && alignment <= 16
ensures result < alignment
{
    let remainder: u64 = offset % alignment;
    match remainder == 0 { true -> 0u64, _ -> alignment - remainder }
}
"#;
    let verdict = super::entryless::judged(SOURCE, false);
    assert!(matches!(verdict, RouteVerdict::Accepted(_)), "{verdict:?}");

    // Returning alignment itself on the zero arm breaks the strict bound.
    let source = SOURCE.replace("true -> 0u64", "true -> alignment");
    let verdict = super::entryless::judged(&source, false);
    let RouteVerdict::Rejected { unproved, .. } = verdict else {
        panic!("{verdict:?}");
    };
    assert!(
        unproved.iter().any(|row| {
            matches!(row.kind, ObligationKind::NormalGuarantee { .. })
                && row.statement.contains("exit from c_abi_padding")
        }),
        "{unproved:?}"
    );
}

#[test]
fn a_join_keeps_the_strict_progress_of_an_addition() {
    const SOURCE: &str = r#"
machine choose(base: u64, increase: u64, fallback: u64, add: bool) -> u64
requires base <= 16 && increase >= 1 && increase <= 16 && fallback > base
ensures result > base
{ match add { true -> base + increase, _ -> fallback } }
"#;
    let verdict = super::entryless::judged(SOURCE, false);
    assert!(matches!(verdict, RouteVerdict::Accepted(_)), "{verdict:?}");
    assert_ensures_unproved(super::entryless::judged(
        &SOURCE.replace("_ -> fallback", "_ -> base"),
        false,
    ));
}

#[test]
fn a_nonzero_computed_signed_difference_can_still_be_negative() {
    const SOURCE: &str = r#"
machine choose(offset: i64, limit: i64) -> i64
requires offset >= -4 && offset <= 4 && limit >= 1 && limit <= 16
ensures result < limit
{
    let delta: i64 = offset + 1;
    match delta == 0 { true -> 0i64, _ -> limit - delta }
}
"#;
    assert_ensures_unproved(super::entryless::judged(SOURCE, false));
}

#[test]
fn a_value_defined_in_one_arm_does_not_enter_the_join_scope() {
    // `s` is the join's own parameter here, chosen per arm; neither arm's
    // call result dominates the join, so neither arm's bound survives it.
    let source = "public machine helper(val: u64) -> u64 { val }
public machine pick(flag: bool) -> u64 {
    let s: u64 = match flag {
        true -> helper(150u64)
        false -> helper(5u64)
    };
    s - 100u64
}
machine main() { let r: u64 = pick(true); }
";
    assert_join_subtraction_refused(source);
}

#[test]
fn shift_count_literals_retain_their_own_integer_carrier() {
    for (body, expected) in [
        (
            "let value: i64 = 0i64 - 1i64; value >> 63u64",
            semantic_vocabulary::IntegerValue::Signed(-1),
        ),
        (
            "let value: u64 = 1u64; value << 3u8",
            semantic_vocabulary::IntegerValue::Unsigned(8),
        ),
    ] {
        let result_type = if matches!(expected, semantic_vocabulary::IntegerValue::Signed(_)) {
            "i64"
        } else {
            "u64"
        };
        let source = format!("machine build_entry() -> {result_type} {{ {body} }}");
        let positive = judge_build(&source).unwrap();
        let RouteVerdict::Accepted(accepted) = positive else {
            panic!("{positive:?}")
        };
        let publication = accepted.publish_invocation().unwrap();
        let program = terminal_interpreter::TerminalProgram::borrow(publication.artifact.view());
        let mut instance = program.instantiate(Default::default()).unwrap();
        let status = instance
            .invoke(&mut terminal_work::WorkMeter::with_budget(100))
            .unwrap();
        assert!(
            matches!(status,
            terminal_interpreter::TerminalExecutionStatus::Complete(
                terminal_interpreter::TerminalExecutionResult::Scalar(
                    terminal_interpreter::TerminalScalarValue::Integer { value, .. }
                )
            ) if value == expected),
            "{status:?}"
        );
    }
    let negative =
        judge_build("machine build_entry() -> u64 { let value: u64 = 1u64; value << 64u8 }")
            .unwrap();
    assert!(
        matches!(negative, RouteVerdict::Rejected { ref unproved, .. }
        if unproved.iter().any(|proof| matches!(proof.kind, ObligationKind::OperationSafety { .. }))),
        "{negative:?}"
    );
}
