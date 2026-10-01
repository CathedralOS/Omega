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

const MUTABLE_BOUNDED_LOCAL: &str = r#"
    domain [u8; 8]::Utf8 requires valid_utf8(self);
    machine store(index: u64) requires index < 3 {
        let mut text: [u8; 8] in Utf8 = "abc";
        text[index] = 65;
    }
"#;

fn local_byte_stores(
    checked: &checked_trees::CheckedTrees,
) -> Option<Vec<checked_trees::CheckedUnitEffectOperationPlan>> {
    let program = &checked.typed;
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "store")?;
    let state = program.machine_states(machine).first()?;
    let mut shapes = ShapeCollector::new(program);
    let (structural, scalar) =
        super::super::super::free_structural_scalar_signature(program, &mut shapes, state, &[])?;
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
}

#[test]
fn mutable_bounded_byte_local_retains_its_exact_store_root() {
    let checked =
        checked(MUTABLE_BOUNDED_LOCAL).unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "store")
        .unwrap();
    let state = &checked.machine_states(machine)[0];
    let typed_trees::statement::StatementNode::LocalData(local) =
        &checked.statement_table.statements(state.statement_nodes)[0]
    else {
        panic!("the literal establishes the first local")
    };
    assert!(local.is_mutable);
    assert!(matches!(
        crate::execution::terminal_unit::types::byte_sequence_carrier(
            &checked,
            local.type_reference,
            &[]
        ),
        Some(checked_trees::CheckedByteSequenceCarrier::BoundedOwned { capacity: 8 })
    ));
    let stores = local_byte_stores(&checked).expect("mutable bounded local plans an indexed write");
    let [checked_trees::CheckedUnitEffectOperationPlan::ByteSequenceWrite(store)] =
        stores.as_slice()
    else {
        panic!("one whole-byte local write: {stores:?}")
    };
    assert_eq!(store.statement_index, 1);
    assert_eq!(
        store.destination,
        checked_trees::CheckedStoreRoot::Local {
            symbol: local.symbol
        }
    );
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine.symbol)
        .expect("literal establishment and write compose in one Unit plan");
    assert!(plan.operations.iter().any(|operation| matches!(operation,
        checked_trees::CheckedUnitEffectOperationPlan::EstablishByteSequence { result, value, qualifications }
            if result.statement_index == 0 && value.literal_bytes() == Some(b"abc".as_slice()) && qualifications.len() == 1)));
}

#[test]
fn bounded_byte_store_rejects_forged_local_mutability_and_initializer() {
    let checked =
        checked(MUTABLE_BOUNDED_LOCAL).unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "store")
        .unwrap();
    let statements = checked.machine_states(machine)[0].statement_nodes;
    let typed_trees::statement::StatementNode::LocalData(original) =
        &checked.statement_table.statements(statements)[0]
    else {
        unreachable!()
    };
    let typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } = checked
        .type_reference_table
        .type_reference(original.type_reference)
    else {
        panic!("the bounded byte carrier retains its declared domain")
    };
    for mutation in 0..3 {
        let mut forged = checked.clone();
        let typed_trees::statement::StatementNode::LocalData(local) =
            &mut forged.typed.statement_table.statements_mut(statements)[0]
        else {
            unreachable!()
        };
        match mutation {
            0 => local.is_mutable = false,
            1 => local.initial_value = Default::default(),
            _ => local.type_reference = *base_type,
        }
        // Removing the qualification can select the preexisting fixed-array
        // store lane. It must never retain the bounded byte owner's authority.
        assert!(
            local_byte_stores(&forged).is_none_or(|stores| stores.iter().all(
                |operation| !matches!(
                    operation,
                    checked_trees::CheckedUnitEffectOperationPlan::ByteSequenceWrite(_)
                )
            )),
            "mutation {mutation} must not retain the bounded byte write plan"
        );
    }
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
/// outside it. A byte field's structural type carries its domain predicates,
/// so the predicate-free domain already has no Terminal field type: its
/// signature declines before any store is planned rather than erasing the
/// qualification.
#[test]
fn byte_field_stores_decline_without_a_provable_byte_domain() {
    for (source, has_signature) in [
        (
            "domain [u8;3]::Writable;
             data Record { out: [u8;3] in Writable; }
             machine store(record: &mut Record) { record.out = \"abc\"; }",
            false,
        ),
        (
            "data Record { out: [u8;3]; }
             machine store(record: &mut Record) { record.out = \"abc\"; }",
            true,
        ),
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
        let signature =
            super::super::super::free_structural_scalar_signature(program, &mut shapes, state, &[]);
        assert_eq!(
            signature.is_some(),
            has_signature,
            "only a representable byte field keeps its signature: {source:?}"
        );
        let Some((structural, scalar)) = signature else {
            continue;
        };
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
