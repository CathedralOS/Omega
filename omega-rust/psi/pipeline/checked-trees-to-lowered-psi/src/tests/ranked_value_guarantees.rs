//! A value-returning ranked machine's guarantee is checked against its loop:
//! the producer proposes the header invariant the guarantee needs, proves
//! every arrival, and the replayed bundle refuses a wrong accumulator arrival
//! while the unchanged `Natural` certificate still answers the cycle question.

use super::{LoweringError, checked_source, lower_machine};
use crate::TerminalMachineSelection;
use crate::proofs::operation_proofs::finalize_operation_proofs;
use crate::proofs::scalar_block_invariants::retain_provable;
use lowered_psi::LoweredPsi;
use proof_admission::{AdmissionProfile, EvidenceError, ProofError};
use semantic_vocabulary::{IntegerValue, ObligationId, Proposition, ScalarTerm};
use terminal_psi::{
    ContractClause, OperationKind, TerminalRankedScc, Terminator, ValueDeclaration,
};
use terminal_verifier::{
    VerificationError, reconstruct_control_cycle_obligations, reconstruct_terminal_obligations,
    validate_module, verify_module,
};

const COUNTDOWN: &str = "machine descend(n: u64, previous: u64) -> u64
terminates by n -> Nat::Descending;
{
    transition n > 0 {
        true -> descend(n - 1, n)
        false -> previous
    }
}";

fn term(value: &ValueDeclaration) -> ScalarTerm {
    ScalarTerm::value(value.id, value.scalar_type)
}

/// The ranked countdown with `requires n <= previous` and the accumulator
/// guarantee `ensures result <= previous`. The source exit prover has no exact
/// origin for a parameter the loop re-enters, so the contract is stated at the
/// Terminal level: this exercises producer inference and independent
/// verification, not source acceptance. The retained cycle certificate stays;
/// every obligation certificate is produced again once the contract is known.
fn guaranteed_countdown(
    guarantee: impl FnOnce(&ValueDeclaration, &[ValueDeclaration]) -> Proposition,
) -> LoweredPsi {
    let checked = checked_source(COUNTDOWN);
    let mut lowered = lower_machine(&checked, TerminalMachineSelection::Name("descend")).unwrap();
    assert!(lowered.semantic_module.scalar_block_invariants.is_empty());
    assert!(matches!(
        lowered.semantic_module.machines[0].ranked_scc,
        Some(TerminalRankedScc::Natural(_))
    ));
    let next = terminal_verifier::maximum_registered_obligation_id(&lowered.semantic_module)
        .unwrap()
        .checked_add(1)
        .unwrap();
    let machine = &mut lowered.semantic_module.machines[0];
    let result = machine.result.scalar().unwrap();
    let [n, previous] = machine.parameters.as_slice() else {
        panic!("countdown formals")
    };
    machine.contract.requires = vec![Proposition::LessOrEqual(term(n), term(previous))];
    machine.contract.ensures = vec![ContractClause {
        obligation: ObligationId::new(next).unwrap(),
        proposition: guarantee(&result, &machine.parameters),
    }];
    lowered.proof_bundle.evidence.clear();
    lowered
}

fn accumulator_guarantee(result: &ValueDeclaration, formals: &[ValueDeclaration]) -> Proposition {
    Proposition::LessOrEqual(term(result), term(&formals[1]))
}

/// The latch block, its backedge arrival obligation, and its `1` constant.
fn latch(lowered: &LoweredPsi) -> (usize, ObligationId, ValueDeclaration) {
    let machine = &lowered.semantic_module.machines[0];
    let [invariant] = lowered.semantic_module.scalar_block_invariants.as_slice() else {
        panic!("one strengthened header")
    };
    let (block_index, backedge) = machine
        .blocks
        .iter()
        .enumerate()
        .find_map(|(index, block)| match &block.terminator {
            Terminator::Jump { edge, target, .. }
                if *target == invariant.header && block.id != machine.entry =>
            {
                Some((index, *edge))
            }
            _ => None,
        })
        .expect("the countdown latch jumps back to its header");
    let arrival = invariant
        .arrivals
        .iter()
        .find(|arrival| arrival.edge == backedge)
        .expect("the backedge is an invariant arrival");
    let one = machine.blocks[block_index]
        .operations
        .iter()
        .find_map(|operation| match operation.kind {
            OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(1),
            } => operation.result.scalar(),
            _ => None,
        })
        .expect("the latch materializes its decrement constant");
    (block_index, arrival.obligation, one)
}

#[test]
fn ranked_countdown_guarantee_retains_an_accumulator_invariant_and_verifies() {
    let mut lowered = guaranteed_countdown(accumulator_guarantee);
    retain_provable(&mut lowered).unwrap();
    let module = &lowered.semantic_module;
    let machine = &module.machines[0];
    let [invariant] = module.scalar_block_invariants.as_slice() else {
        panic!("the guarantee strengthens exactly one cyclic header")
    };
    let header = machine
        .blocks
        .iter()
        .find(|block| block.id == invariant.header)
        .unwrap();
    let [rank, accumulator] = header.parameters.as_slice() else {
        panic!("the header carries the rank and the accumulator")
    };
    let Proposition::Conjunction(members) = &invariant.predicate else {
        panic!("two proposals join into one header conjunction")
    };
    assert_eq!(members.len(), 2, "{members:?}");
    assert!(
        members.contains(&Proposition::LessOrEqual(term(rank), term(accumulator))),
        "the entry requirement is generalized over the header parameters: {members:?}"
    );
    assert!(
        members.contains(&Proposition::LessOrEqual(
            term(accumulator),
            term(&machine.parameters[1])
        )),
        "the guarantee is transported to the header accumulator: {members:?}"
    );
    assert_eq!(invariant.arrivals.len(), 2, "entry arrival and backedge");

    finalize_operation_proofs(&mut lowered).unwrap();
    let module = &lowered.semantic_module;
    let proofs = &lowered.proof_bundle;
    let verified = verify_module(module, proofs, &AdmissionProfile::default()).unwrap();
    assert_eq!(verified.accepted_control_cycles().len(), 1);
    assert_eq!(
        proofs.control_cycles.len(),
        1,
        "the Natural certificate is retained"
    );
    let guarantee = module.machines[0].contract.ensures[0].obligation;
    assert!(
        proofs
            .evidence
            .iter()
            .any(|evidence| evidence.obligation == guarantee),
        "the guarantee has its own certificate"
    );
    // The functional claim is not inherited from termination: dropping any
    // arrival certificate leaves the module unverified with its cycle intact.
    for arrival in &module.scalar_block_invariants[0].arrivals {
        let mut omitted = proofs.clone();
        omitted
            .evidence
            .retain(|evidence| evidence.obligation != arrival.obligation);
        assert!(matches!(
            verify_module(module, &omitted, &AdmissionProfile::default()),
            Err(VerificationError::MissingEvidence(obligation)) if obligation == arrival.obligation
        ));
    }
}

#[test]
fn wrong_accumulator_arrival_rejects_the_replayed_guarantee_certificate() {
    let mut lowered = guaranteed_countdown(accumulator_guarantee);
    retain_provable(&mut lowered).unwrap();
    finalize_operation_proofs(&mut lowered).unwrap();
    let (latch_index, preservation, one) = latch(&lowered);
    let module = lowered.semantic_module.clone();
    let proofs = lowered.proof_bundle.clone();
    verify_module(&module, &proofs, &AdmissionProfile::default()).unwrap();

    // The latch forwards its `1` constant as the next accumulator instead of
    // the current rank; the rank argument and the certified descent are untouched.
    let mut wrong = module.clone();
    let Terminator::Jump { arguments, .. } = &mut wrong.machines[0].blocks[latch_index].terminator
    else {
        panic!("latch jump")
    };
    arguments[1] = one.id;
    let validated = validate_module(&wrong).expect("a wrong accumulator arrival is well formed");
    let cycle_questions = reconstruct_control_cycle_obligations(&wrong).unwrap();
    assert_eq!(
        cycle_questions,
        reconstruct_control_cycle_obligations(&module).unwrap(),
        "the wrong update asks the identical cycle question"
    );
    let [cycle_question] = cycle_questions.as_slice() else {
        panic!("one ranked component")
    };
    let context = validated.value_context(&wrong.machines[0]).unwrap();
    let parameters = wrong.machines[0]
        .parameters
        .iter()
        .map(|parameter| parameter.id)
        .collect();
    proof_admission::verify_recursive_component_with_machine_parameters(
        &context,
        &cycle_question.obligation,
        &parameters,
        proofs.control_cycles[0].certificate.clone(),
        &AdmissionProfile::default(),
    )
    .expect("the unchanged Natural certificate still answers the cycle question");
    // The preservation goal now names the forwarded constant, and the retained
    // certificate for the actual update refuses the changed conclusion.
    let questions = reconstruct_terminal_obligations(&wrong).unwrap();
    let goal = &questions
        .obligations()
        .iter()
        .find(|question| question.obligation.id == preservation)
        .unwrap()
        .obligation
        .proposition;
    assert!(goal.any_value_id(|value| value == one.id), "{goal:?}");
    assert_eq!(
        verify_module(&wrong, &proofs, &AdmissionProfile::default()).err(),
        Some(VerificationError::RejectedEvidence {
            obligation: preservation,
            error: EvidenceError::Certificate(ProofError::CertificateConclusionMismatch),
        })
    );
    // Fresh production refuses the wrong arrival as well: the accumulator
    // invariant is not inductive, so the strengthening is discarded and the
    // guarantee stays unprovable.
    let mut reproduced = lowered.clone();
    reproduced.semantic_module = wrong;
    reproduced.semantic_module.scalar_block_invariants.clear();
    reproduced.proof_bundle.evidence.clear();
    retain_provable(&mut reproduced).unwrap();
    assert!(
        reproduced
            .semantic_module
            .scalar_block_invariants
            .is_empty()
    );
    let guarantee = reproduced.semantic_module.machines[0].contract.ensures[0].obligation;
    assert!(matches!(
        finalize_operation_proofs(&mut reproduced),
        Err(LoweringError::OperationProofUnavailable(obligation)) if obligation == guarantee
    ));
}

const CLIMB: &str = "machine climb(remaining: u64[0..=1000], acc: u64[0..=1000]) -> u64
terminates by remaining -> Nat::Descending;
ensures result == acc + remaining
{
    transition remaining > 0 && acc < 1000 {
        true -> climb(remaining - 1, acc + 1)
        false -> (acc + remaining)
    }
}";

/// The strengthened cyclic header and its backedge preservation obligation.
fn conserved_sum_header(lowered: &LoweredPsi) -> (&Proposition, ObligationId) {
    let machine = &lowered.semantic_module.machines[0];
    let invariant = lowered
        .semantic_module
        .scalar_block_invariants
        .iter()
        .find(|invariant| {
            machine
                .blocks
                .iter()
                .any(|block| block.id == invariant.header && block.id != machine.entry)
        })
        .expect("the latch header is strengthened");
    let backedge = machine
        .blocks
        .iter()
        .find_map(|block| match &block.terminator {
            Terminator::Jump { edge, target, .. }
                if *target == invariant.header && block.id != machine.entry =>
            {
                Some(*edge)
            }
            _ => None,
        })
        .expect("the latch jumps back to its header");
    let arrival = invariant
        .arrivals
        .iter()
        .find(|arrival| arrival.edge == backedge)
        .expect("the backedge is an invariant arrival");
    (&invariant.predicate, arrival.obligation)
}

/// Whether `predicate` carries an exact `sum == sum` conjunct.
fn conserves_a_sum(predicate: &Proposition) -> bool {
    match predicate {
        Proposition::Conjunction(members) => members.iter().any(conserves_a_sum),
        Proposition::Equal(left, right) => {
            matches!(left, ScalarTerm::ExactIntegerAdd { .. })
                && matches!(right, ScalarTerm::ExactIntegerAdd { .. })
        }
        _ => false,
    }
}

#[test]
fn arithmetic_accumulator_guarantee_retains_the_conserved_sum_invariant() {
    let checked = checked_source(CLIMB);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("climb"))
        .expect("the conserved sum makes the guarantee provable");
    let module = &lowered.semantic_module;
    let (invariant, preservation) = conserved_sum_header(&lowered);
    assert!(
        conserves_a_sum(invariant),
        "the header invariant carries the transported guarantee: {invariant:?}"
    );
    let verified = verify_module(module, &lowered.proof_bundle, &AdmissionProfile::default())
        .expect("the emitted certificates verify independently");
    assert_eq!(verified.accepted_control_cycles().len(), 1);
    let guarantee = module.machines[0].contract.ensures[0].obligation;
    assert!(
        lowered
            .proof_bundle
            .evidence
            .iter()
            .any(|evidence| evidence.obligation == guarantee),
        "the ensures clause has its own certificate"
    );

    // The certified backedge is the update's own claim: forwarding the
    // accumulator's predecessor unchanged asks a different preservation
    // question, and the retained certificate refuses it rather than silently
    // covering the wrong arrival.
    let mut wrong = module.clone();
    let latch_index = wrong.machines[0]
        .blocks
        .iter()
        .position(|block| match &block.terminator {
            Terminator::Jump { target, .. } => {
                *target
                    == module
                        .scalar_block_invariants
                        .iter()
                        .find(|invariant| {
                            invariant
                                .arrivals
                                .iter()
                                .any(|arrival| arrival.obligation == preservation)
                        })
                        .map(|invariant| invariant.header)
                        .expect("strengthened header")
                    && block.id != wrong.machines[0].entry
            }
            _ => false,
        })
        .expect("the latch block");
    let unchanged_acc = wrong.machines[0].blocks[latch_index]
        .operations
        .iter()
        .find_map(|operation| match operation.kind {
            OperationKind::ExactIntegerAdd { left, .. } => Some(left),
            _ => None,
        })
        .expect("the latch adds one to the accumulator");
    let Terminator::Jump { arguments, .. } = &mut wrong.machines[0].blocks[latch_index].terminator
    else {
        panic!("latch jump")
    };
    arguments[1] = unchanged_acc;
    let questions = reconstruct_terminal_obligations(&wrong).unwrap();
    let goal = &questions
        .obligations()
        .iter()
        .find(|question| question.obligation.id == preservation)
        .unwrap()
        .obligation
        .proposition;
    assert!(
        goal.any_value_id(|value| value == unchanged_acc),
        "{goal:?}"
    );
    assert_eq!(
        verify_module(&wrong, &lowered.proof_bundle, &AdmissionProfile::default()).err(),
        Some(VerificationError::RejectedEvidence {
            obligation: preservation,
            error: EvidenceError::Certificate(ProofError::CertificateConclusionMismatch),
        })
    );
}

#[test]
fn a_guarantee_the_loop_does_not_establish_discards_the_whole_strengthening() {
    // `result <= n` claims the accumulator never exceeds the initial rank; the
    // transported header candidate `accumulator <= n` fails establishment
    // because `requires n <= previous` bounds the rank, not the accumulator.
    let mut lowered = guaranteed_countdown(|result, formals| {
        Proposition::LessOrEqual(term(result), term(&formals[0]))
    });
    retain_provable(&mut lowered).unwrap();
    assert!(
        lowered.semantic_module.scalar_block_invariants.is_empty(),
        "a strengthening that cannot prove every arrival leaves the roster unchanged"
    );
    let guarantee = lowered.semantic_module.machines[0].contract.ensures[0].obligation;
    assert!(matches!(
        finalize_operation_proofs(&mut lowered),
        Err(LoweringError::OperationProofUnavailable(obligation)) if obligation == guarantee
    ));
}
