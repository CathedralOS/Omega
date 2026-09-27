use super::{ScalarType, lower_machine};
use crate::TerminalMachineSelection;
use crate::proofs::operation_proofs::finalize_operation_proofs;
use crate::terminal_identities::obligation_id;
use lowered_psi::LoweredPsi;
use proof_admission::AdmissionProfile;
use semantic_vocabulary::{IntegerValue, Proposition, ScalarTerm};
use terminal_psi::OperationKind;
use terminal_verifier::ProofBundle;

fn fixture() -> LoweredPsi {
    let checked = crate::front_end::checked_program(
        "machine count(current: u64 [0..=5]) -> u64 {
            transition current < 5 {
                true -> count(current + 1)
                false -> current
            }
        }",
    );
    let mut lowered = lower_machine(&checked, TerminalMachineSelection::Name("count")).unwrap();
    assert_eq!(lowered.semantic_module.scalar_block_invariants.len(), 1);
    lowered.semantic_module.scalar_block_invariants.clear();
    lowered.proof_bundle = ProofBundle::default();
    lowered
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
    let checked = crate::front_end::checked_program(
        "machine m(x: u64 [0..=8], s: u64 [0..=8]) -> u64 {
            transition { _ -> m(s, x) }
         }",
    );
    let mut lowered = lower_machine(&checked, TerminalMachineSelection::Name("m")).unwrap();
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
    // The rewritten contract narrows `x` to `<= 4`, so its retained entry
    // range must publish the same `[0, 4]` interval; `s` still spans `[0, 8]`.
    for range in &mut lowered
        .semantic_module
        .scalar_qualifications
        .integer_entry_ranges
    {
        if range.parameter == x.id {
            range.maximum = IntegerValue::Unsigned(4);
        }
    }
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
