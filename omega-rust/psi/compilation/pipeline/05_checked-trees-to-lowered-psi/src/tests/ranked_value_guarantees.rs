//! A value-returning ranked machine's guarantee is checked against its loop:
//! the producer proposes the header invariant the guarantee needs, proves
//! every arrival, and the replayed bundle refuses a wrong accumulator arrival
//! while the unchanged `Natural` certificate still answers the cycle question.

use super::{LoweringError, lower_machine};
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
    let checked = crate::front_end::checked_program(COUNTDOWN);
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

/// The actual customer's explicit subject and bounded-distance contract, with
/// host effects removed only from the harness. No guarantee is injected after
/// source checking: source custody, invariant production and proof publication
/// must all preserve the authored mathematical relationship.
const CLIMBING_SUM: &str = r#"
    data Main {}
    machine Main::climb(main: &mut Main, limit: u64, index: u64, acc: u64) -> u64
    requires limit <= 1000, acc <= index, acc <= 1000
    terminates by (index, limit) -> Nat::BoundedDistance;
    ensures embed(result) >= embed(acc) + embed(limit) - embed(index)
    {
        transition index < limit {
            true -> climb(main, limit, index + 1, acc + 1)
            false -> acc
        }
    }
"#;

#[test]
fn mathematical_climbing_guarantee_survives_codec_and_rejects_wrong_accumulator() {
    use terminal_interpreter::{
        AcceptTerminalEffects, TerminalExecutionResult, TerminalScalarValue,
        TerminalStructuralInputs, TerminalStructuralValue,
    };
    let checked = crate::front_end::checked_program(CLIMBING_SUM);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Main::climb"))
        .expect("the authored mathematical guarantee lowers with its inductive invariant");
    let semantic_bytes = terminal_codec::encode_module(&lowered.semantic_module).unwrap();
    let proof_bytes =
        terminal_codec::encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
            .unwrap();
    let module = terminal_codec::decode_module(&semantic_bytes).unwrap();
    let proofs = terminal_codec::decode_proof_section_for(&module, &proof_bytes).unwrap();
    let profile = AdmissionProfile::default();
    verify_module(&module, &proofs, &profile).expect("source-free proof replay");
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let unsigned = |value| TerminalScalarValue::Integer {
        scalar_type: semantic_vocabulary::IntegerType::new(
            semantic_vocabulary::IntegerSign::Unsigned,
            64,
        )
        .unwrap(),
        value: IntegerValue::Unsigned(value),
    };
    let subject = TerminalStructuralValue {
        opaque_identity: 1,
        structural_type: entry.structural_parameters[0].structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    for (limit, index, accumulator, expected) in [(4, 0, 0, 4), (4, 9, 5, 5)] {
        // The second call returns immediately: the mathematical remaining
        // distance is negative, rather than underflowing an unsigned carrier.
        let result = terminal_interpreter::interpret_terminal_artifact_measured(
            &semantic_bytes,
            &proof_bytes,
            &profile,
            &[unsigned(limit), unsigned(index), unsigned(accumulator)],
            TerminalStructuralInputs {
                arguments: std::slice::from_ref(&subject),
                ..Default::default()
            },
            &mut AcceptTerminalEffects,
        )
        .expect("verified climbing artifact executes without checked trees");
        assert_eq!(
            result.value(),
            TerminalExecutionResult::Scalar(unsigned(expected))
        );
    }

    // Retain the exact arithmetic operations and rank arrival. Only the
    // accumulator carried on the backedge changes from acc + 1 to acc.
    let mut wrong = module.clone();
    let machine_index = wrong
        .machines
        .iter()
        .position(|machine| machine.id == module.entry)
        .unwrap();
    let machine = &wrong.machines[machine_index];
    let (block_index, prior_accumulator) = machine
        .blocks
        .iter()
        .enumerate()
        .find_map(|(index, block)| {
            let Terminator::Jump {
                target, arguments, ..
            } = &block.terminator
            else {
                return None;
            };
            if block.id == machine.entry
                || !wrong
                    .scalar_block_invariants
                    .iter()
                    .any(|invariant| invariant.header == *target)
            {
                return None;
            }
            let updated = *arguments.get(2)?;
            let prior = machine
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .find_map(|operation| {
                    if operation
                        .result
                        .scalar()
                        .is_none_or(|result| result.id != updated)
                    {
                        return None;
                    }
                    match operation.kind {
                        OperationKind::ExactIntegerAdd { left, .. } => Some(left),
                        _ => None,
                    }
                })?;
            Some((index, prior))
        })
        .expect("the loop's accumulator update reaches its strengthened header");
    let Terminator::Jump { arguments, .. } =
        &mut wrong.machines[machine_index].blocks[block_index].terminator
    else {
        panic!("climbing backedge");
    };
    assert_ne!(arguments[2], prior_accumulator);
    arguments[2] = prior_accumulator;
    let validated =
        validate_module(&wrong).expect("the stale accumulator arrival is structurally well formed");
    let cycle_questions = reconstruct_control_cycle_obligations(&wrong).unwrap();
    assert_eq!(
        cycle_questions,
        reconstruct_control_cycle_obligations(&module).unwrap(),
        "the mutation leaves bounded-distance termination unchanged"
    );
    let [cycle_question] = cycle_questions.as_slice() else {
        panic!("one ranked component");
    };
    let context = validated
        .value_context(&wrong.machines[machine_index])
        .unwrap();
    let parameters = wrong.machines[machine_index]
        .parameters
        .iter()
        .map(|parameter| parameter.id)
        .collect();
    proof_admission::verify_recursive_component_with_machine_parameters(
        &context,
        &cycle_question.obligation,
        &parameters,
        proofs.control_cycles[0].certificate.clone(),
        &profile,
    )
    .expect("the retained cycle certificate still proves termination");
    assert!(
        matches!(
            verify_module(&wrong, &proofs, &profile),
            Err(VerificationError::RejectedEvidence { .. })
        ),
        "termination cannot license a false accumulator guarantee"
    );

    let mut reproduced = lowered;
    reproduced.semantic_module = wrong;
    reproduced.semantic_module.scalar_block_invariants.clear();
    reproduced.proof_bundle.evidence.clear();
    retain_provable(&mut reproduced).unwrap();
    assert!(
        matches!(
            finalize_operation_proofs(&mut reproduced),
            Err(LoweringError::OperationProofUnavailable(_))
        ),
        "fresh evidence production must also refuse the wrong update"
    );
}

/// The source guarantee is an identity in the u64 residue ring. It must not
/// acquire exact-arithmetic no-overflow obligations during publication.
const WRAPPING_GAUSS: &str = r#"
    data Main {}
    machine Main::gauss_sum(main: &mut Main, n: u64 in Wrapping, acc: u64 in Wrapping) -> u64 in Wrapping
    terminates by n;
    ensures result * 2 == acc * 2 + n * (n + 1)
    {
        transition n > 0 {
            true -> gauss_sum(main, n - 1, acc + n)
            false -> acc
        }
    }
"#;

#[test]
fn wrapping_gauss_guarantee_survives_codec_and_rejects_wrong_accumulator() {
    use terminal_interpreter::{
        AcceptTerminalEffects, TerminalExecutionResult, TerminalScalarValue,
        TerminalStructuralInputs, TerminalStructuralValue,
    };
    let checked = crate::front_end::checked_program(WRAPPING_GAUSS);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Main::gauss_sum"))
        .expect("the authored residue-ring guarantee lowers with its inductive invariant");
    let semantic_bytes = terminal_codec::encode_module(&lowered.semantic_module).unwrap();
    let proof_bytes =
        terminal_codec::encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
            .unwrap();
    let module = terminal_codec::decode_module(&semantic_bytes).unwrap();
    let proofs = terminal_codec::decode_proof_section_for(&module, &proof_bytes).unwrap();
    let profile = AdmissionProfile::default();
    verify_module(&module, &proofs, &profile).expect("source-free proof replay");
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let unsigned = |value| TerminalScalarValue::Integer {
        scalar_type: semantic_vocabulary::IntegerType::new(
            semantic_vocabulary::IntegerSign::Unsigned,
            64,
        )
        .unwrap(),
        value: IntegerValue::Unsigned(value),
    };
    let subject = TerminalStructuralValue {
        opaque_identity: 1,
        structural_type: entry.structural_parameters[0].structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    for (rank, accumulator, expected) in [(4, 0, 10), (2, u64::MAX as u128, 2), (0, 17, 17)] {
        // The second invocation crosses the u64 modulus in acc + n; the
        // third returns its nonzero accumulator without entering the loop.
        // All cases execute the same decoded artifact with runtime inputs.
        let result = terminal_interpreter::interpret_terminal_artifact_measured(
            &semantic_bytes,
            &proof_bytes,
            &profile,
            &[unsigned(rank), unsigned(accumulator)],
            TerminalStructuralInputs {
                arguments: std::slice::from_ref(&subject),
                ..Default::default()
            },
            &mut AcceptTerminalEffects,
        )
        .expect("verified Gauss artifact executes without checked trees");
        assert_eq!(
            result.value(),
            TerminalExecutionResult::Scalar(unsigned(expected))
        );
    }

    // Weakening 0 < n to 0 <= n admits the wrapping decrement at zero.
    // The module remains well formed, but its retained ranking evidence must
    // fail independently of accumulator guarantees or their certificates.
    let mut unguarded = module.clone();
    let machine_index = unguarded
        .machines
        .iter()
        .position(|machine| machine.id == module.entry)
        .unwrap();
    let machine = &unguarded.machines[machine_index];
    let (block_index, operation_index, left, right) = machine
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block_index, block)| {
            let Terminator::Conditional { condition, .. } = block.terminator else {
                return None;
            };
            let (operation_index, operation) =
                block.operations.iter().enumerate().find(|(_, operation)| {
                    operation
                        .result
                        .scalar()
                        .is_some_and(|result| result.id == condition)
                })?;
            let OperationKind::IntegerLessThan { left, right } = operation.kind else {
                return None;
            };
            machine
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .find(|operation| {
                    operation
                        .result
                        .scalar()
                        .is_some_and(|result| result.id == left)
                        && matches!(
                            operation.kind,
                            OperationKind::IntegerConstant {
                                value: IntegerValue::Unsigned(0)
                            }
                        )
                })?;
            Some((block_index, operation_index, left, right))
        })
        .expect("the authored positive-rank branch compares zero with the current rank");
    unguarded.machines[machine_index].blocks[block_index].operations[operation_index].kind =
        OperationKind::IntegerLessOrEqual { left, right };
    let validated = validate_module(&unguarded)
        .expect("weakening the branch changes no type, custody, or graph shape");
    let cycle_questions = reconstruct_control_cycle_obligations(&unguarded).unwrap();
    assert_ne!(
        cycle_questions,
        reconstruct_control_cycle_obligations(&module).unwrap()
    );
    let [cycle_question] = cycle_questions.as_slice() else {
        panic!("one ranked component");
    };
    let context = validated
        .value_context(&unguarded.machines[machine_index])
        .unwrap();
    let parameters = unguarded.machines[machine_index]
        .parameters
        .iter()
        .map(|parameter| parameter.id)
        .collect();
    let rejected = proof_admission::verify_recursive_component_with_machine_parameters(
        &context,
        &cycle_question.obligation,
        &parameters,
        proofs.control_cycles[0].certificate.clone(),
        &profile,
    );
    assert!(
        matches!(
            rejected,
            Err(proof_admission::RecursiveComponentError::Decrease { .. })
        ),
        "the retained cycle certificate must reject the missing strict guard: {rejected:?}"
    );

    // Retain the exact arithmetic operations and rank arrival. Only the
    // accumulator carried on the backedge changes from acc + n to acc.
    let mut wrong = module.clone();
    let machine_index = wrong
        .machines
        .iter()
        .position(|machine| machine.id == module.entry)
        .unwrap();
    let machine = &wrong.machines[machine_index];
    let questions = reconstruct_terminal_obligations(&module).unwrap();
    let (block_index, prior_accumulator) = machine
        .blocks
        .iter()
        .enumerate()
        .find_map(|(index, block)| {
            let Terminator::Jump {
                edge,
                target,
                arguments,
                ..
            } = &block.terminator
            else {
                return None;
            };
            if block.id == machine.entry
                || !wrong
                    .scalar_block_invariants
                    .iter()
                    .any(|invariant| invariant.header == *target)
            {
                return None;
            }
            let updated = *arguments.get(1)?;
            // The arrival can carry an SSA alias of the update rather than
            // its operation result. Use the independently reconstructed facts
            // at this exact arrival, which transport both the update and its
            // operands into the latch's current namespace.
            let question = questions.obligations().iter().find(|question| {
                matches!(question.owner,
                    terminal_verifier::ReconstructedTerminalObligationOwner::ScalarBlockInvariant {
                        machine: owner, header, edge: incoming
                    } if owner == machine.id && header == *target && incoming == *edge)
            })?;
            let prior = question.semantic_axioms.iter().find_map(|fact| {
                let Proposition::Equal(result, expression) = fact else {
                    return None;
                };
                let ScalarTerm::Value { id, scalar_type } = result else {
                    return None;
                };
                let ScalarTerm::WrappingIntegerAdd {
                    scalar_type: carrier,
                    left,
                    ..
                } = expression
                else {
                    return None;
                };
                let ScalarTerm::Value {
                    id: prior,
                    scalar_type: prior_type,
                } = left.as_ref()
                else {
                    return None;
                };
                (*id == updated
                    && *scalar_type == semantic_vocabulary::ScalarType::Integer(*carrier)
                    && prior_type == scalar_type)
                    .then_some(*prior)
            })?;
            Some((index, prior))
        })
        .expect("the loop's accumulator update reaches its strengthened header");
    let Terminator::Jump { arguments, .. } =
        &mut wrong.machines[machine_index].blocks[block_index].terminator
    else {
        panic!("Gauss backedge");
    };
    assert_ne!(arguments[1], prior_accumulator);
    arguments[1] = prior_accumulator;
    let validated =
        validate_module(&wrong).expect("the stale accumulator arrival is structurally well formed");
    let cycle_questions = reconstruct_control_cycle_obligations(&wrong).unwrap();
    assert_eq!(
        cycle_questions,
        reconstruct_control_cycle_obligations(&module).unwrap(),
        "the mutation leaves natural-rank termination unchanged"
    );
    let [cycle_question] = cycle_questions.as_slice() else {
        panic!("one ranked component");
    };
    let context = validated
        .value_context(&wrong.machines[machine_index])
        .unwrap();
    let parameters = wrong.machines[machine_index]
        .parameters
        .iter()
        .map(|parameter| parameter.id)
        .collect();
    proof_admission::verify_recursive_component_with_machine_parameters(
        &context,
        &cycle_question.obligation,
        &parameters,
        proofs.control_cycles[0].certificate.clone(),
        &profile,
    )
    .expect("the retained cycle certificate still proves termination");
    assert!(
        matches!(
            verify_module(&wrong, &proofs, &profile),
            Err(VerificationError::RejectedEvidence { .. })
        ),
        "termination cannot license a false accumulator guarantee"
    );

    let mut reproduced = lowered;
    reproduced.semantic_module = wrong;
    reproduced.semantic_module.scalar_block_invariants.clear();
    reproduced.proof_bundle.evidence.clear();
    retain_provable(&mut reproduced).unwrap();
    assert!(
        matches!(
            finalize_operation_proofs(&mut reproduced),
            Err(LoweringError::OperationProofUnavailable(_))
        ),
        "fresh evidence production must also refuse the wrong update"
    );
}
