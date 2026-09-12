use super::*;
use proof_admission::AdmissionProfile;

fn fixture() -> LoweredPsi {
    let checked = checked_source(
        "machine count(current: u64 [0..=5]) -> u64 {
            transition current < 5 {
                true -> count(current + 1)
                false -> current
            }
        }",
    );
    let mut lowered = lower_machine(&checked, "count").unwrap();
    assert_eq!(lowered.semantic_module.scalar_block_invariants.len(), 1);
    lowered.semantic_module.scalar_block_invariants.clear();
    lowered.proof_bundle = ProofBundle::default();
    lowered
}

#[test]
fn optional_inference_drops_an_entry_range_that_does_not_survive_the_backedge() {
    let mut lowered = fixture();
    let machine = &mut lowered.semantic_module.machines[0];
    let parameter = machine.parameters[0];
    let ScalarType::Integer(integer) = parameter.scalar_type else {
        panic!("integer")
    };
    // This is a Terminal inference test, not a source type change. The narrower
    // invocation contract is valid for this safe loop, but is not an invariant:
    // an input of four reaches five. Source state types have separate store
    // obligations and must not be weakened to manufacture this counterexample.
    machine.contract.requires = vec![Proposition::LessOrEqual(
        ScalarTerm::value(parameter.id, parameter.scalar_type),
        ScalarTerm::integer(integer, IntegerValue::Unsigned(4)).unwrap(),
    )];
    let original = lowered.semantic_module.clone();
    crate::scalar_block_invariants::retain_provable(&mut lowered).unwrap();
    assert_eq!(lowered.semantic_module, original);
    finalize_operation_proofs(&mut lowered).unwrap();
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .unwrap();
}

#[test]
fn exhausted_optional_obligation_ids_do_not_reject_an_existing_program() {
    let mut lowered = fixture();
    let operation = lowered.semantic_module.machines[0]
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.operations)
        .find(|operation| matches!(operation.kind, OperationKind::ExactIntegerAdd { .. }))
        .unwrap();
    let OperationKind::ExactIntegerAdd { obligation, .. } = &mut operation.kind else {
        panic!("exact addition")
    };
    *obligation = obligation_id(u64::MAX);
    let original = lowered.semantic_module.clone();
    crate::scalar_block_invariants::retain_provable(&mut lowered).unwrap();
    assert_eq!(lowered.semantic_module, original);
    finalize_operation_proofs(&mut lowered).unwrap();
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .unwrap();
}

#[test]
fn optional_inference_preserves_retained_source_certificates() {
    let mut lowered = fixture();
    finalize_operation_proofs(&mut lowered).unwrap();
    let original_proofs = lowered.proof_bundle.clone();
    crate::scalar_block_invariants::retain_provable(&mut lowered).unwrap();
    assert_eq!(lowered.proof_bundle, original_proofs);
    // If inference retained a compatible roster, its new arrivals still need
    // ordinary finalization; existing source certificates are never discarded.
    finalize_operation_proofs(&mut lowered).unwrap();
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .unwrap();
}
