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
use crate::execution::terminal_unit::types::ShapeCollector;

use crate::execution::terminal_unit::structural_scalar_store::build_structural_scalar_field_store_sequence;
use crate::tests::front_end::checked_program_result;

fn checked(source: &str) -> Result<checked_trees::CheckedTrees, Vec<diagnostics::Diagnostic>> {
    checked_program_result(source)
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
