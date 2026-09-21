//! Borrowed byte-buffer writes through Unit parameters: the `ByteSequenceWrite`
//! and `StructuralByteSequenceFieldStore` lanes, the upstream check fences that
//! gate them, and the currently unreachable `...FieldByteStore` arm.
//!
//! These tests pin observed behavior. The borrowed-view write lane is admitted
//! only once the checker can prove the index — today that means an authored
//! `requires` premise on the slice length, since a `&mut [u8]` parameter
//! carries no length of its own. Indexed byte stores into a bounded byte-array
//! field (`param.field[i] = b`) stay checker-fenced: the bounds proof cannot
//! see the field's literal length through the borrow, so the
//! `StructuralByteSequenceFieldByteStore` plan variant is unreachable from
//! source today; if it ever becomes reachable these pins must be re-derived.
use super::CheckedUnitEffectOperationPlan;
use crate::execution::terminal_unit::ShapeCollector;
use crate::execution::terminal_unit::structural_scalar_store::build_structural_scalar_field_store_sequence;

fn checked(source: &str) -> Result<checked_trees::CheckedTrees, Vec<diagnostics::Diagnostic>> {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    crate::lower_typed_trees(typed)
}

fn free_machine_stores(source: &str) -> Vec<CheckedUnitEffectOperationPlan> {
    let checked =
        checked(source).unwrap_or_else(|diagnostics| panic!("program must check: {diagnostics:?}"));
    let program = &checked.typed;
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "store")
        .unwrap();
    let state = &program.machine_states(machine)[0];
    let mut shapes = ShapeCollector::new(program);
    let (structural, scalar) =
        super::super::super::free_structural_scalar_signature(program, &mut shapes, state, &[])
            .expect("free machine signature");
    build_structural_scalar_field_store_sequence(
        program,
        &checked.facts,
        machine,
        state,
        &structural,
        &scalar,
        0,
        None,
    )
    .expect("statement sequence plans")
}

/// `target[i] = b` on a `&mut [u8]` view produces a `ByteSequenceWrite` — but
/// only because the authored `requires` premise proves the index. Both the
/// literal-index and premise-discharged dynamic-index forms are admitted.
#[test]
fn borrowed_byte_view_writes_use_their_length_premise() {
    let stores = free_machine_stores(
        "machine store(target: &mut [u8], index: u8)
         requires target.len > 0 { target[0] = index; }",
    );
    assert!(
        matches!(
            stores.as_slice(),
            [CheckedUnitEffectOperationPlan::ByteSequenceWrite(write)]
                if write.destination_parameter_position == 0
        ),
        "premise-proved literal index must emit ByteSequenceWrite: {stores:?}"
    );

    let stores = free_machine_stores(
        "machine store(target: &mut [u8], index: u8, at: u64)
         requires at < target.len { target[at] = index; }",
    );
    assert!(
        matches!(
            stores.as_slice(),
            [CheckedUnitEffectOperationPlan::ByteSequenceWrite(write)]
                if write.destination_parameter_position == 0
                && matches!(
                    write.index,
                    checked_trees::CheckedScalarExpression::Parameter { .. }
                )
        ),
        "premise-proved parameter index must emit ByteSequenceWrite: {stores:?}"
    );
}

/// The checker, not this lane, answers the missing-premise spellings: without
/// `requires`, `target[0]` cannot prove its bound on an unknown slice length,
/// and a shared `&[u8]` view is not mutable at all. Both fences stay upstream
/// of plan admission, which is why these assert on the diagnostic rather than
/// a declined plan.
#[test]
fn byte_view_writes_without_a_length_premise_are_checker_fenced() {
    for (source, fragment) in [
        (
            "machine store(target: &mut [u8], index: u8) { target[0] = index; }",
            "cannot prove index `0` is within unknown slice length",
        ),
        (
            "machine store(target: &[u8], index: u8) { target[0] = index; }",
            "cannot write `target` because it is not mutable",
        ),
    ] {
        let Err(diagnostics) = checked(source) else {
            panic!("expected the checker to reject {source:?}")
        };
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(fragment)),
            "missing fence diagnostic {fragment:?}: {diagnostics:?}"
        );
    }
}

/// A byte-literal assignment into a domain-gated bounded byte-array field is
/// admitted as `StructuralByteSequenceFieldStore` when the literal satisfies
/// the field's comptime byte predicate (here `valid_utf8` on `"abc"`).
#[test]
fn domain_gated_byte_field_accepts_a_satisfying_literal_write() {
    let stores = free_machine_stores(
        "domain [u8;3]::Utf8 requires valid_utf8(self);
         data Record { out: [u8;3] in Utf8; }
         machine store(record: &mut Record) { record.out = \"abc\"; }",
    );
    assert!(
        matches!(
            stores.as_slice(),
            [CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldStore(store)]
                if store.field_identity == "out" && store.bytes == b"abc"
        ),
        "literal satisfying the field's byte predicate must plan the buffer store: {stores:?}"
    );
}

/// The same destination declines when nothing can grant the carrier's domain:
/// a predicate-free `Writable` domain offers no comptime byte predicate a
/// literal could satisfy, and a domain-free `[u8;3]` field is not a bounded
/// byte-sequence carrier at all. This lane's plan vocabulary only names owned
/// bounded carriers with provable literals — arbitrary source values stay
/// outside it.
#[test]
fn byte_field_stores_decline_without_a_provable_byte_domain() {
    for source in [
        "domain [u8;3]::Writable;
         data Record { out: [u8;3] in Writable; }
         machine store(record: &mut Record) { record.out = \"abc\"; }",
        "data Record { out: [u8;3]; }
         machine store(record: &mut Record) { record.out = \"abc\"; }",
    ] {
        let checked = checked(source)
            .unwrap_or_else(|diagnostics| panic!("program must check: {diagnostics:?}"));
        let program = &checked.typed;
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "store")
            .unwrap();
        let state = &program.machine_states(machine)[0];
        let mut shapes = ShapeCollector::new(program);
        let (structural, scalar) =
            super::super::super::free_structural_scalar_signature(program, &mut shapes, state, &[])
                .expect("free machine signature");
        assert!(
            build_structural_scalar_field_store_sequence(
                program,
                &checked.facts,
                machine,
                state,
                &structural,
                &scalar,
                0,
                None,
            )
            .is_none(),
            "no byte-buffer plan without a provable byte-predicate domain: {source:?}"
        );
    }
}

/// `param.field[i] = b` into a bounded byte-array field is checker-fenced
/// today: the bounds proof sees an unknown slice length through the borrow
/// (and the domain's field requirement cannot be re-established for return),
/// so `StructuralByteSequenceFieldByteStore` has no reachable source spelling.
/// A lane reaching it must first give the index proof the field's literal
/// length through a borrowed root.
#[test]
fn indexed_byte_field_store_is_checker_fenced_today() {
    for source in [
        // Borrowed root: the field's literal length is invisible.
        "domain [u8;3]::Utf8 requires valid_utf8(self);
         data Record { out: [u8;3] in Utf8; }
         machine store(record: &mut Record, index: u8) { record.out[1] = index; }",
        // Attached receiver: same bound blindness plus domain re-proof.
        "domain [u8;3]::Utf8 requires valid_utf8(self);
         data Record { out: [u8;3] in Utf8; }
         machine Record::store(&mut self, index: u8) { self.out[1] = index; }",
    ] {
        let Err(diagnostics) = checked(source) else {
            panic!("indexed byte-field store is currently checker-fenced: {source:?}")
        };
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("unknown slice length")),
            "missing bounds fence diagnostic: {diagnostics:?}"
        );
    }
}
