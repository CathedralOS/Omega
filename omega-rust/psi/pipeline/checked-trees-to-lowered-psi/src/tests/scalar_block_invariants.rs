use super::{ScalarType, checked_source, lower_machine};
use crate::proofs::nonzero_divisor_certificate::{
    produce_checked_canonical_integer_proof, produce_relaxed_integer_proof,
};
use crate::proofs::operation_proofs::finalize_operation_proofs;
use crate::terminal_identities::obligation_id;
use lowered_psi::LoweredPsi;
use proof_admission::AdmissionProfile;
use semantic_vocabulary::{IntegerValue, Proposition, ScalarTerm};
use terminal_psi::OperationKind;
use terminal_verifier::{ProofBundle, ReconstructedTerminalObligationOwner};

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
    crate::proofs::scalar_block_invariants::retain_provable(&mut lowered).unwrap();
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
    crate::proofs::scalar_block_invariants::retain_provable(&mut lowered).unwrap();
    assert_eq!(lowered.semantic_module, original);
    finalize_operation_proofs(&mut lowered).unwrap();
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .unwrap();
}

/// The unconditional self-loop hands the arrival obligation `x + s <= 4`
/// under `x <= 4` and `s <= 0`: the bound only closes once `s <= 0` is read
/// as `s == 0` and pushed through the addition — the bounded derived closure,
/// outside the canonical custody envelope. The candidate must be retained and
/// the kernel must still verify the finalized module.
#[test]
fn relaxed_arrival_retains_bound_canonical_cannot_close() {
    let checked = checked_source(
        "machine m(x: u64 [0..=4], s: u64 [0..=0]) -> u64 {
            transition { _ -> m(x + s, s) }
         }",
    );
    let mut lowered = lower_machine(&checked, "m").unwrap();
    lowered.semantic_module.scalar_block_invariants.clear();
    lowered.proof_bundle = ProofBundle::default();
    crate::proofs::scalar_block_invariants::retain_provable(&mut lowered).unwrap();
    assert_eq!(lowered.semantic_module.scalar_block_invariants.len(), 1);

    // Pin the fallback: on the cycle arrival the canonical search must fail
    // where the relaxed derived closure succeeds.
    let module = &lowered.semantic_module;
    let validated = terminal_verifier::validate_module(module).unwrap();
    let questions = terminal_verifier::reconstruct_terminal_obligations(module).unwrap();
    let mut relaxed_arrivals = 0usize;
    for site in questions.obligations() {
        let ReconstructedTerminalObligationOwner::ScalarBlockInvariant { machine, .. } = site.owner
        else {
            continue;
        };
        let owner = module.machines.iter().find(|m| m.id == machine).unwrap();
        let context = validated.value_context(owner).unwrap();
        let params = owner.parameters.iter().map(|v| v.id).collect();
        if produce_checked_canonical_integer_proof(
            &context,
            &site.obligation.proposition,
            &site.requirements,
            &site.semantic_axioms,
            &params,
        )
        .is_none()
        {
            relaxed_arrivals += 1;
            assert!(
                produce_relaxed_integer_proof(
                    &context,
                    &site.obligation.proposition,
                    &site.requirements,
                    &site.semantic_axioms,
                    &params,
                )
                .is_some(),
                "the canonical gap must be closed by the relaxed derived closure"
            );
        }
    }
    assert_eq!(
        relaxed_arrivals, 1,
        "exactly one arrival needs the fallback"
    );

    // Finalization must produce evidence for every retained arrival and the
    // kernel must verify the whole module — no certificate is admitted on the
    // relaxed producer's word alone.
    finalize_operation_proofs(&mut lowered).unwrap();
    let retained = &lowered.semantic_module.scalar_block_invariants[0];
    for arrival in &retained.arrivals {
        assert!(
            lowered
                .proof_bundle
                .evidence
                .iter()
                .any(|e| e.obligation == arrival.obligation),
            "finalization must produce evidence for retained arrival {:?}",
            arrival.obligation
        );
    }
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .unwrap();
}

/// The swap recursion `m(s, x)` transports the x-slot bound onto `s` at the
/// back-edge. This is a Terminal inference test, not a source type change:
/// both parameters are declared `[0..=8]` so the swap checks, then the
/// machine contract is narrowed to `x <= 4` and `s <= 0` is offered only
/// behind an undischarged implication. Hoisting that premise's conclusion
/// into ambient facts is the only way `s <= 4` could close, so custody
/// requires the merged header candidate to drop — and the kernel-verified
/// module must still stand without it.
#[test]
fn relaxed_arrival_rejects_when_only_a_guarded_premise_could_close_it() {
    let checked = checked_source(
        "machine m(x: u64 [0..=8], s: u64 [0..=8]) -> u64 {
            transition { _ -> m(s, x) }
         }",
    );
    let mut lowered = lower_machine(&checked, "m").unwrap();
    lowered.semantic_module.scalar_block_invariants.clear();
    lowered.proof_bundle = ProofBundle::default();
    let machine = &mut lowered.semantic_module.machines[0];
    let x = machine.parameters[0];
    let s = machine.parameters[1];
    let ScalarType::Integer(integer) = s.scalar_type else {
        panic!("scalar fixture parameters stay integer");
    };
    let integer_bound =
        |bound: u128| ScalarTerm::integer(integer, IntegerValue::Unsigned(bound)).unwrap();
    let s_term = || ScalarTerm::value(s.id, s.scalar_type);
    machine.contract.requires = vec![
        Proposition::LessOrEqual(integer_bound(0), ScalarTerm::value(x.id, x.scalar_type)),
        Proposition::LessOrEqual(ScalarTerm::value(x.id, x.scalar_type), integer_bound(4)),
        Proposition::LessOrEqual(integer_bound(0), s_term()),
        Proposition::LessOrEqual(s_term(), integer_bound(8)),
        Proposition::Implication {
            premise: Box::new(Proposition::LessOrEqual(s_term(), integer_bound(0))),
            conclusion: Box::new(Proposition::LessOrEqual(s_term(), integer_bound(0))),
        },
    ];
    crate::proofs::scalar_block_invariants::retain_provable(&mut lowered).unwrap();
    assert!(
        lowered.semantic_module.scalar_block_invariants.is_empty(),
        "an undischarged implication premise cannot supply the missing bound"
    );
    // The drop must come from candidate rejection, not module invalidation:
    // the module still validates, finalization still succeeds, and the kernel
    // still verifies.
    terminal_verifier::validate_module(&lowered.semantic_module).unwrap();
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
    crate::proofs::scalar_block_invariants::retain_provable(&mut lowered).unwrap();
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
