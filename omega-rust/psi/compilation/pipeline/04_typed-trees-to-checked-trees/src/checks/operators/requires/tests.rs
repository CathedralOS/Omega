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
fn named_float_requires_use_stronger_operand_bounds() {
    check(
        "boundary operator Ns::probe(value: f64) -> bool
         requires value == value && value > -1.0 && value < 101.0;
         machine compare(value: f64) -> bool
         requires value >= 0.0 && value <= 100.0 { Ns::probe(value) }",
    )
    .expect("positive ordered facts prove non-NaN and weaker bounds");
}

#[test]
fn named_float_requires_keep_bounds_of_an_already_copied_operand() {
    check(
        "boundary operator Ns::probe(left: f64, right: i32) -> bool
         requires left == left && left > -1.0;
         machine reset(value: &mut f64) -> i32 { value = -10.0; 0 }
         machine compare(mut value: f64) -> bool requires value >= 0.0 {
             Ns::probe(value, reset(&mut value))
         }",
    )
    .expect("a later storage write cannot change the captured scalar's bounds");
}

#[test]
fn named_float_requires_retain_an_assigned_field_across_a_state_edge() {
    check(
        "data Cell { value: f64; }
         boundary operator Ns::probe(value: f64) -> bool
         requires value == value && value > 0.0 && value < 2.0;
         machine Cell::compare(&mut self) -> bool {
             self.value = 1.5;
             transition true { true -> next() }
             state next(&mut self) -> bool { Ns::probe(self.value) }
         }",
    )
    .expect("an immutable literal observation survives an unchanged state transfer");
}

#[test]
fn named_float_requires_reject_unestablished_bounds() {
    for source in [
        // A false IEEE ordered comparison may describe NaN.
        "boundary operator Ns::probe(value: f64) -> bool requires value >= 0.0;
         machine compare(value: f64) -> bool requires !(value < 0.0) {
             Ns::probe(value)
         }",
        // An inclusive bound cannot imply a strict bound at the same endpoint.
        "boundary operator Ns::probe(value: f64) -> bool requires value > 0.0;
         machine compare(value: f64) -> bool requires value >= 0.0 {
             Ns::probe(value)
         }",
        // Equal spelling on another place does not identify the captured value.
        "boundary operator Ns::probe(value: f64) -> bool requires value > -1.0;
         machine compare(left: f64, right: f64) -> bool requires left >= 0.0 {
             Ns::probe(right)
         }",
        // Facts established after capture describe the later storage version.
        "boundary operator Ns::probe(left: f64, right: i32) -> bool requires left > -1.0;
         machine prepare(value: &mut f64) -> i32 ensures value >= 0.0 { value = 1.0; 0 }
         machine compare(mut value: f64) -> bool {
             Ns::probe(value, prepare(&mut value))
         }",
        // A write retires the old literal observation, including its bounds.
        "boundary operator Ns::probe(value: f64) -> bool requires value > 0.0;
         machine compare() -> bool {
             let mut value: f64 = 1.0;
             value = -1.0;
             Ns::probe(value)
         }",
        // f32 rounds 16777217 to 16777216, so the strict bound is not proved.
        "boundary operator Ns::probe(value: f32) -> bool requires value > 16777216.0;
         machine compare(value: f32) -> bool requires value >= 16777217.0 {
             Ns::probe(value)
         }",
        // An arbitrary selected relation cannot supply IEEE order laws.
        "boundary operator >= Custom::at_least(left: f64, right: f64) -> bool;
         boundary operator Ns::probe(value: f64) -> bool requires value > -1.0;
         machine compare(value: f64) -> bool requires value >= 0.0 {
             Ns::probe(value)
         }",
        // Every incoming edge must deliver the same observed field value.
        "data Cell { value: f64; }
         boundary operator Ns::probe(value: f64) -> bool requires value > 0.0;
         machine Cell::compare(&mut self, choose: bool) -> bool {
             transition choose { true -> good() false -> bad() }
             state good(&mut self) -> bool {
                 self.value = 1.0;
                 transition true { true -> joined() }
             }
             state bad(&mut self) -> bool {
                 self.value = -1.0;
                 transition true { true -> joined() }
             }
             state joined(&mut self) -> bool { Ns::probe(self.value) }
         }",
    ] {
        let diagnostics = check(source).expect_err("missing bounds must remain unproven");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("cannot prove")),
            "{diagnostics:#?}"
        );
    }
}

#[test]
fn named_float_requires_transport_guarded_scalar_state_arguments() {
    check(
        "boundary operator Ns::probe(value: f64) -> bool
         requires value == value && value > -1.0 && value < 101.0;
         machine compare(input: f64) -> bool {
             transition input >= 0.0 && input <= 100.0 {
                 true -> bounded(input) false -> denied()
             }
             state bounded(renamed: f64) -> bool { Ns::probe(renamed) }
             state denied() -> bool { false }
         }",
    )
    .expect("the guarded value retains its predicates under a new state parameter identity");
}

#[test]
fn named_float_requires_transport_live_implicit_receiver_predicates() {
    check(
        "data Cell { value: f64; }
         boundary operator Ns::probe(value: f64) -> bool requires value > -1.0;
         machine Cell::compare(&mut self) -> bool {
             transition self.value >= 0.0 { true -> bounded() false -> denied() }
             state bounded(&mut self) -> bool { Ns::probe(self.value) }
             state denied(&mut self) -> bool { false }
         }",
    )
    .expect("an implicit receiver carries live predicates, not a historical read");
}

#[test]
fn named_float_requires_state_arguments_keep_operand_time_predicates() {
    check(
        "boundary operator Ns::probe(value: f64) -> bool requires value > -1.0;
         machine reset(value: &mut f64) -> i32 { value = -10.0; 0 }
         machine compare(mut value: f64) -> bool requires value >= 0.0 {
             transition true { true -> bounded(value, reset(&mut value)) }
             state bounded(captured: f64, ignored: i32) -> bool { Ns::probe(captured) }
         }",
    )
    .expect("a later operand write cannot revoke the earlier scalar copy's predicates");
    check(
        "data Cell { value: f64; }
         boundary operator Ns::probe(value: f64) -> bool requires value > -1.0;
         machine reset(value: &mut f64) -> i32 { value = -10.0; 0 }
         machine Cell::compare(&mut self) -> bool requires self.value >= 0.0 {
             transition true { true -> bounded(self.value, reset(&mut self.value)) }
             state bounded(&mut self, captured: f64, ignored: i32) -> bool {
                 Ns::probe(captured)
             }
         }",
    )
    .expect("an explicitly copied receiver field uses the ordinary argument snapshot");
}

#[test]
fn named_float_requires_join_weaker_bounds_from_every_predecessor() {
    check(
        "boundary operator Ns::probe(value: f64) -> bool requires value > 0.0;
         machine compare(value: f64, choose: bool) -> bool {
             transition choose { true -> one(value) false -> two(value) }
             state one(first: f64) -> bool {
                 transition first >= 1.0 { true -> joined(first) false -> denied() }
             }
             state two(second: f64) -> bool {
                 transition second >= 2.0 { true -> joined(second) false -> denied() }
             }
             state joined(value: f64) -> bool { Ns::probe(value) }
             state denied() -> bool { false }
         }",
    )
    .expect("the common lower bound is independent of predecessor order");
}

#[test]
fn named_float_requires_state_edges_cannot_mint_missing_predicates() {
    for source in [
        "boundary operator Ns::probe(value: f64) -> bool requires value > -1.0;
         machine compare(value: f64) -> bool {
             transition value >= 0.0 { true -> joined(value) false -> joined(value) }
             state joined(value: f64) -> bool { Ns::probe(value) }
         }",
        "boundary operator Ns::probe(value: f64) -> bool requires value > -1.0;
         machine prepare(value: &mut f64) -> i32 ensures value >= 0.0 { value = 1.0; 0 }
         machine compare(mut value: f64) -> bool {
             transition true { true -> joined(value, prepare(&mut value)) }
             state joined(value: f64, ignored: i32) -> bool { Ns::probe(value) }
         }",
        "data Cell { value: f64; }
         boundary operator Ns::probe(value: f64) -> bool requires value > -1.0;
         machine Cell::compare(&mut self) -> bool {
             transition self.value >= 0.0 { true -> overwrite() false -> denied() }
             state overwrite(&mut self) -> bool {
                 self.value = -10.0;
                 transition true { true -> joined() }
             }
             state joined(&mut self) -> bool { Ns::probe(self.value) }
             state denied(&mut self) -> bool { false }
         }",
    ] {
        let diagnostics =
            check(source).expect_err("state transport cannot strengthen missing or stale evidence");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("cannot prove")),
            "{diagnostics:#?}"
        );
    }
}

#[test]
fn named_call_requires_keep_a_copied_generic_record_after_a_right_operand_write() {
    // A generic instantiation is the same detached copy: `h`'s bound snapshot
    // for `left` predates `reset`'s write to the source storage. `Holder` is
    // `[copy]` so the operand really is a copy; an affine `h` would move into
    // `left` and leave nothing for `&mut h` to borrow.
    check(
        "pub data Holder<T [copy]> [copy] { value: T; other: i32; }
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
    // discharges the clause. The later operand cannot write `value` while the
    // shared loan is held, so it establishes the newer fact through a
    // requirement's guarantee on a second shared loan of the same referent.
    let checked = check(
        "boundary operator == Reference::equal(left: &i32, right: i32) -> bool
         requires left >= 0;
         boundary trait Probe { machine certify(value: &i32) -> i32 ensures value >= 0; }
         machine compare(value: i32) -> bool reaches Probe {
             Reference::equal(&value, Probe::certify(&value))
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
        .operands
        .span_or_empty(invocation.operands)
        .len();
    for ordinal in 0..operand_count as u32 {
        let handle = arena::Handle::from_parts(
            invocation.operands.start().arena_index() + ordinal,
            invocation.operands.start().generation(),
        );
        facts.flow.control.operands.get_mut(handle).referents = arena::HandleSpan::empty();
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
