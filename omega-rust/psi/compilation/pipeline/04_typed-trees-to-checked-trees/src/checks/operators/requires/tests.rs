//! Named `Namespace::requirement(...)` calls carry the same selected
//! `requires` obligations as spelled uses. Their operand-time capture keys on
//! the `named_use` handle rather than a `uses` row, so these tests exercise
//! the checker through the named operand vocabulary: operator-parameter
//! ordering, reference formals whose predicate value is the referent, and the
//! fail-closed custody corruptions the spelled suite already covers.

use crate::tests::front_end::checked_program_result;
fn check(source: &str) -> Result<checked_trees::CheckedTrees, Vec<diagnostics::Diagnostic>> {
    checked_program_result(source)
}

#[test]
fn named_call_requires_keep_a_copied_generic_record_after_a_right_operand_write() {
    // A generic instantiation is the same detached copy: `h`'s bound snapshot
    // for `left` predates `reset`'s write to the source storage.
    check(
        "pub data Holder<T> { value: T; other: i32; }
         boundary operator Ns::probe(left: Holder<i32>, right: i32) -> bool
         requires left.value >= 0;
         machine reset(h: &mut Holder<i32>) -> i32 { h.value = -1; 0 }
         machine compare(mut h: Holder<i32>) -> bool requires h.value >= 0 {
             Ns::probe(h, reset(&mut h))
         }",
    )
    .expect("a later write cannot revoke facts about the already-copied generic record");
}

#[test]
fn named_call_requires_cannot_give_a_copied_record_a_later_storage_guarantee() {
    // `prepare` establishes `rec.count >= 0` on the source storage after the
    // `left` copy was already taken — the newer guarantee describes current
    // storage, never the bound operand.
    let diagnostics = check(
        "pub data Rec { count: i32; other: i32; }
         boundary operator Ns::probe(left: Rec, right: i32) -> bool
         requires left.count >= 0;
         machine prepare(rec: &mut Rec) -> i32 ensures rec.count >= 0 { rec.count = 1; 0 }
         machine compare(mut rec: Rec) -> bool {
             Ns::probe(rec, prepare(&mut rec))
         }",
    )
    .expect_err("a guarantee newer than the copy cannot discharge its clause");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("requires")),
        "{diagnostics:#?}"
    );
}

#[test]
fn named_call_referent_custody_has_no_snapshot_fallback() {
    // Clearing the referent custody row drops the operand onto the old
    // context-identity transport, where the dead captured context no longer
    // discharges the clause.
    let checked = check(
        "boundary operator == Reference::equal(left: &i32, right: i32) -> bool
         requires left >= 0;
         machine prepare(value: &mut i32) -> i32 ensures value >= 0 { value = 1; 0 }
         machine compare(mut value: i32) -> bool {
             Reference::equal(&value, prepare(&mut value))
         }",
    )
    .unwrap();
    let invocation = checked
        .facts
        .flow
        .control
        .operator_invocations
        .iter()
        .next()
        .unwrap()
        .1;
    let mut facts = checked.facts.clone();
    let operand_count = facts
        .flow
        .control
        .operator_operands
        .span_or_empty(invocation.operands)
        .len();
    for ordinal in 0..operand_count as u32 {
        let handle = arena::Handle::from_parts(
            invocation.operands.start().arena_index() + ordinal,
            invocation.operands.start().generation(),
        );
        facts
            .flow
            .control
            .operator_operands
            .get_mut(handle)
            .referents = arena::HandleSpan::empty();
    }
    let diagnostics = crate::checks::check_checked_facts(&checked.typed, &facts)
        .expect_err("without custody the retained snapshot no longer discharges the clause");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("requires")),
        "{diagnostics:#?}"
    );
}

/// The rewrite can only carry a selection with a declared result type — a
/// `boundary operator` without one stays a `StatementNode::Call` with no
/// `named_uses` row or operand capture, so its `requires` fails closed.
#[test]
fn unrewritten_statement_call_to_a_requires_operator_fails_closed() {
    let diagnostics = check(
        "boundary operator Ns::store(v: i32)
         requires v >= 0;
         machine caller(value: i32) requires value >= 0 { Ns::store(value); }",
    )
    .expect_err("a statement call with no operand capture must not pass");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("carries `requires` obligations no checked capture can prove")
        }),
        "{diagnostics:#?}"
    );
}

/// A statement-position named call to a result-returning operator must not
/// silently drop the value; `_ = Ns::probe(...)` is the admitted spelling and
/// still selects the requirement for contract checking.
#[test]
fn statement_position_named_call_discard_gate_and_checked_discard() {
    let diagnostics = check(
        "boundary operator Ns::probe(v: i32) -> bool
         requires v >= 0;
         machine caller(value: i32) requires value >= 0 { Ns::probe(value); }",
    )
    .expect_err("discarding a non-unit named requirement result rejects");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("discards its non-unit `bool` result")
        }),
        "{diagnostics:#?}"
    );

    check(
        "boundary operator Ns::probe(v: i32) -> bool
         requires v >= 0;
         machine caller(value: i32) requires value >= 0 { _ = Ns::probe(value); }",
    )
    .expect("an explicitly discarded call is selected and its requires checks");

    let diagnostics = check(
        "boundary operator Ns::probe(v: i32) -> bool
         requires v >= 0;
         machine caller(value: i32) { _ = Ns::probe(value); }",
    )
    .expect_err("an explicitly discarded call still owes the requires");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("cannot prove `value >= 0`")
                && diagnostic.message.contains("Ns::probe")
        }),
        "{diagnostics:#?}"
    );
}
