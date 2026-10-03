use crate::CheckingRequest;
use crate::tests::front_end::typed_program as parse_typed_trees;

const SOURCE: &str = r#"
machine accumulate(mut remaining: u64, mut accumulator: u64, total: u64) -> u64
requires remaining > 0 && remaining <= 1000 && accumulator <= 1000 && total <= 1000
requires remaining + accumulator == total
terminates by remaining -> Nat::Descending;
ensures result == total
{
    remaining = remaining - 1;
    accumulator = accumulator + 1;
    transition remaining > 0 {
        true -> accumulate(remaining, accumulator, total)
        false -> accumulator
    }
}
"#;

#[test]
fn current_scalar_assignment_transport_proves_named_arrivals() {
    for source in [SOURCE.to_owned(),
        SOURCE.replace("remaining = remaining - 1;\n    accumulator = accumulator + 1;",
            "let next_remaining: u64 = remaining - 1;\n    let next_accumulator: u64 = accumulator + 1;\n    remaining = next_remaining;\n    accumulator = next_accumulator;"),
        SOURCE.replace("remaining = remaining - 1;\n    accumulator = accumulator + 1;",
            "accumulator = accumulator + 1;\n    remaining = remaining - 1;")]
    {
        crate::lower_typed_trees(parse_typed_trees(&source), &CheckingRequest::settled())
            .unwrap_or_else(|diagnostics| panic!("current assignment values must compose: {diagnostics:#?}\n{source}"));
    }
}

#[test]
fn current_scalar_assignment_transport_rejects_wrong_preservation_and_stale_returns() {
    for source in [
        SOURCE.replace("accumulator + 1", "accumulator + 2"),
        SOURCE
            .replace(
                "    remaining = remaining - 1;",
                "    let before: u64 = accumulator;\n    remaining = remaining - 1;",
            )
            .replace("false -> accumulator", "false -> before"),
    ] {
        let diagnostics =
            crate::lower_typed_trees(parse_typed_trees(&source), &CheckingRequest::settled())
                .expect_err("a wrong update or stale captured result must not prove the guarantee");
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("cannot prove ensures")
                || diagnostic.message.contains("cannot prove state arrival")
                || diagnostic.message.contains("cannot prove requires")),
            "{diagnostics:#?}"
        );
    }
}

#[test]
fn current_scalar_assignment_transport_rejects_restored_rank() {
    let source = SOURCE.replace(
        "    remaining = remaining - 1;\n    accumulator = accumulator + 1;",
        "    let saved: u64 = remaining;\n    remaining = remaining - 1;\n    remaining = saved;",
    );
    let diagnostics =
        crate::lower_typed_trees(parse_typed_trees(&source), &CheckingRequest::settled())
            .expect_err("an intermediate decrement is not descent after restoration");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("cannot prove the `terminates by` ranking")),
        "{diagnostics:#?}"
    );
}

#[test]
fn current_scalar_assignment_transport_does_not_restore_invalidated_safety() {
    let source = SOURCE.replace(
        "    remaining = remaining - 1;",
        "    remaining = 0;\n    remaining = remaining - 1;",
    );
    let diagnostics =
        crate::lower_typed_trees(parse_typed_trees(&source), &CheckingRequest::settled())
            .expect_err(
                "the original positive bound cannot authorize a later subtraction from zero",
            );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("overflow")
                || diagnostic.message.contains("underflow")),
        "{diagnostics:#?}"
    );
}

#[test]
fn current_scalar_assignment_transport_does_not_identify_reentered_total_with_invocation() {
    let source = SOURCE
        .replace(
            "requires remaining > 0",
            "requires accumulator >= 1 && remaining > 0",
        )
        .replace(
            "requires remaining + accumulator == total",
            "requires total >= 2\nrequires remaining + accumulator == total",
        )
        .replace(
            "true -> accumulate(remaining, accumulator, total)",
            "true -> accumulate(remaining, accumulator - 1, total - 1)",
        );
    let diagnostics =
        crate::lower_typed_trees(parse_typed_trees(&source), &CheckingRequest::settled())
            .expect_err("the current sum does not establish the invocation's different total");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove ensures")),
        "{diagnostics:#?}"
    );
}

#[test]
fn current_scalar_assignment_transport_invalidates_effectful_guards_and_actuals() {
    for source in [
        r#"
machine restore(value: &mut u64) -> bool { value = 3; true }
machine walk(mut remaining: u64) -> u64
requires remaining > 0 && remaining <= 3
terminates by remaining -> Nat::Descending;
{
    remaining = remaining - 1;
    transition restore(&mut remaining) {
        true -> walk(remaining)
        false -> remaining
    }
}
"#,
        r#"
machine restore_zero(value: &mut u64) -> u64 { value = 3; 0 }
machine walk(marker: u64, mut remaining: u64) -> u64
requires remaining > 0 && remaining <= 3
terminates by remaining -> Nat::Descending;
{
    remaining = remaining - 1;
    transition remaining > 0 {
        true -> walk(restore_zero(&mut remaining), remaining)
        false -> remaining
    }
}
"#,
    ] {
        let diagnostics =
            crate::lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
                .expect_err(
                    "restoring the counter through an operand call cannot preserve descent",
                );
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("cannot prove the `terminates by` ranking")),
            "{diagnostics:#?}"
        );
    }
}

#[test]
fn current_scalar_assignment_transport_observes_earlier_sibling_call_writes() {
    let source = r#"
machine restore_zero(value: &mut u64) -> u64 { value = 0; 0 }
machine positive(value: u64) -> u64 requires value > 0 { value }
machine combine(first: u64, second: u64) -> u64 { second }
machine caller(mut remaining: u64, mut scratch: u64) -> u64
requires remaining > 0
{
    combine(restore_zero(&mut remaining), positive(remaining))
}
"#;
    let diagnostics =
        crate::lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
            .expect_err("the later call cannot borrow the pre-write positive fact");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("cannot prove requires contract for call positive")),
        "{diagnostics:#?}"
    );
    let disjoint = source.replace("restore_zero(&mut remaining)", "restore_zero(&mut scratch)");
    crate::lower_typed_trees(parse_typed_trees(&disjoint), &CheckingRequest::settled())
        .unwrap_or_else(|diagnostics| {
            panic!("a disjoint operand write preserves the scalar fact: {diagnostics:#?}")
        });
}
