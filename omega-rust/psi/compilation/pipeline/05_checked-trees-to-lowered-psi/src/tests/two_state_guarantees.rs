//! Replay both authored arrivals of a ranked accumulator cycle. Ranking and
//! the invocation-relative result guarantee remain separate obligations.

use super::{LoweringError, lower_machine};
use crate::TerminalMachineSelection;
use crate::proofs::operation_proofs::finalize_operation_proofs;
use crate::proofs::scalar_block_invariants::retain_provable;
use proof_admission::AdmissionProfile;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, OperationId, ValueId};
use terminal_psi::{
    Operation, OperationKind, OperationResult, TerminalNaturalRankComparison, TerminalRankedScc,
    Terminator, ValueDeclaration,
};
use terminal_verifier::{
    VerificationError, reconstruct_control_cycle_obligations, reconstruct_terminal_obligations,
    validate_module, verify_module,
};

const CUSTOMER: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../tests/omega/pass/proofs/runtime_two_state_accumulator_exit/main.omg"
));

#[test]
fn two_state_accumulator_survives_serialization_and_rejects_each_wrong_arrival() {
    use terminal_interpreter::{
        AcceptTerminalEffects, TerminalExecutionResult, TerminalScalarValue,
        TerminalStructuralInputs,
    };
    // The crate helper has no package loader. Select the exact maintained
    // machine; the native corpus also checks its Console caller and exit70.
    let machine_start = CUSTOMER.find("machine descend(").unwrap();
    let checked = crate::front_end::checked_program(&CUSTOMER[machine_start..]);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("descend"))
        .expect("both state arrivals preserve the invocation-relative guarantee");
    let semantic_bytes = terminal_codec::encode_module(&lowered.semantic_module).unwrap();
    let proof_bytes =
        terminal_codec::encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
            .unwrap();
    let module = terminal_codec::decode_module(&semantic_bytes).unwrap();
    let proofs = terminal_codec::decode_proof_section_for(&module, &proof_bytes).unwrap();
    let profile = AdmissionProfile::default();
    verify_module(&module, &proofs, &profile).expect("independent decoded artifact replay");
    let carrier = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let unsigned = |value| TerminalScalarValue::Integer {
        scalar_type: carrier,
        value: IntegerValue::Unsigned(value),
    };
    // Both zero exits, initial zero, and the maintained native invocation.
    for (rank, previous, expected) in [(0, 9, 9), (1, 9, 1), (2, 9, 1), (3, 9, 1)] {
        let result = terminal_interpreter::interpret_terminal_artifact_measured(
            &semantic_bytes,
            &proof_bytes,
            &profile,
            &[unsigned(rank), unsigned(previous)],
            TerminalStructuralInputs::default(),
            &mut AcceptTerminalEffects,
        )
        .expect("serialized two-state machine executes without checked trees");
        assert_eq!(
            result.value(),
            TerminalExecutionResult::Scalar(unsigned(expected))
        );
    }

    let machine_index = module
        .machines
        .iter()
        .position(|machine| machine.id == module.entry)
        .unwrap();
    let machine = &module.machines[machine_index];
    let Some(TerminalRankedScc::Natural(cycles)) = &machine.ranked_scc else {
        panic!("the authored natural ranking must be retained");
    };
    let [cycle] = cycles.as_slice() else {
        panic!("one two-state component");
    };
    let strict_edges = cycle
        .edges
        .iter()
        .filter(|edge| edge.comparison == TerminalNaturalRankComparison::Strict)
        .collect::<Vec<_>>();
    assert_eq!(
        strict_edges.len(),
        2,
        "each authored cyclic transition strictly decreases"
    );
    let original_questions = reconstruct_control_cycle_obligations(&module).unwrap();
    let [original_question] = original_questions.as_slice() else {
        panic!("one cycle question");
    };
    let parameters = machine
        .parameters
        .iter()
        .map(|parameter| parameter.id)
        .collect();
    let original_context = validate_module(&module)
        .unwrap()
        .value_context(machine)
        .unwrap();
    let original_acceptance = proof_admission::verify_recursive_component_with_machine_parameters(
        &original_context,
        &original_question.obligation,
        &parameters,
        proofs.control_cycles[0].certificate.clone(),
        &profile,
    )
    .unwrap();
    let fresh_value = ValueId::new(
        machine
            .parameters
            .iter()
            .copied()
            .chain(machine.result.scalar())
            .chain(
                machine
                    .blocks
                    .iter()
                    .flat_map(|block| block.parameters.iter().copied()),
            )
            .chain(
                machine
                    .blocks
                    .iter()
                    .flat_map(|block| &block.operations)
                    .filter_map(|operation| operation.result.scalar()),
            )
            .map(|value| value.id.get())
            .max()
            .unwrap()
            .checked_add(1)
            .unwrap(),
    )
    .unwrap();
    let fresh_operation = OperationId::new(
        module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .map(|operation| operation.id.get())
            .max()
            .unwrap()
            .checked_add(1)
            .unwrap(),
    )
    .unwrap();
    for edge in strict_edges {
        let mut wrong = module.clone();
        let source = wrong.machines[machine_index]
            .blocks
            .iter_mut()
            .find(|block| block.id == edge.source)
            .unwrap();
        let Terminator::Jump {
            edge: actual_edge,
            arguments,
            ..
        } = &mut source.terminator
        else {
            panic!("the authored transition has a final simultaneous jump");
        };
        assert_eq!(*actual_edge, edge.edge);
        assert_eq!(
            arguments.len(),
            2,
            "rank and accumulator are the complete state telescope"
        );
        assert_ne!(arguments[1], fresh_value);
        arguments[1] = fresh_value;
        source.operations.push(Operation {
            id: fresh_operation,
            static_reach_binding: None,
            suspension_crossing: None,
            result: OperationResult::Scalar(ValueDeclaration {
                id: fresh_value,
                scalar_type: semantic_vocabulary::ScalarType::Integer(carrier),
                qualifications: Default::default(),
            }),
            kind: OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(1000),
            },
        });
        let validated =
            validate_module(&wrong).expect("wrong accumulator preserves scalar and graph shape");
        let questions = reconstruct_control_cycle_obligations(&wrong).unwrap();
        let [question] = questions.as_slice() else {
            panic!("one unchanged cycle");
        };
        assert_eq!(
            question.obligation.ranking_relation,
            original_question.obligation.ranking_relation
        );
        let context = validated
            .value_context(&wrong.machines[machine_index])
            .unwrap();
        // The wrong accumulator changes incidental alias facts and their
        // ordering. Replay the same certificate under a permutation/weakening
        // of CURRENT facts: every original cited proposition must still be
        // present, and no old, unavailable fact can enter the replay context.
        let mut replay = question.obligation.clone();
        replay.well_foundedness = current_facts_in_original_citation_order(
            &original_question.obligation.well_foundedness,
            &question.obligation.well_foundedness,
            &original_acceptance.well_foundedness,
        );
        assert_eq!(replay.members, original_question.obligation.members);
        assert_eq!(replay.edges.len(), original_question.obligation.edges.len());
        for (current, original) in replay
            .edges
            .iter_mut()
            .zip(&original_question.obligation.edges)
        {
            assert_eq!(
                (current.caller, current.callee),
                (original.caller, original.callee)
            );
            let accepted = original_acceptance
                .decreases
                .iter()
                .find(|accepted| accepted.obligation == original.decrease.obligation.id)
                .unwrap();
            current.decrease = current_facts_in_original_citation_order(
                &original.decrease,
                &current.decrease,
                accepted,
            );
        }
        proof_admission::verify_recursive_component_with_machine_parameters(
            &context,
            &replay,
            &parameters,
            proofs.control_cycles[0].certificate.clone(),
            &profile,
        )
        .expect(
            "the same logical rank certificate replays after exact current-premise position normalization",
        );
        assert!(
            matches!(
                verify_module(&wrong, &proofs, &profile),
                Err(VerificationError::RejectedEvidence { .. })
                    | Err(VerificationError::RejectedControlCycle {
                        error: proof_admission::RecursiveComponentError::Decrease { .. },
                        ..
                    })
            ),
            "the unmodified artifact evidence must reject the altered arrival on edge {:?}",
            edge.edge
        );
        // Reconstruct all guarantee evidence as well: a stale citation alone
        // must not account for the rejection of this false update. The same
        // control-cycle certificates remain available throughout.
        let mut reproduced = lowered.clone();
        reproduced.semantic_module = wrong;
        reproduced.semantic_module.scalar_block_invariants.clear();
        reproduced.proof_bundle.evidence.clear();
        retain_provable(&mut reproduced).unwrap();
        let result = finalize_operation_proofs(&mut reproduced);
        let Err(LoweringError::OperationProofUnavailable(obligation)) = result else {
            panic!(
                "fresh guarantee proof must refuse the wrong arrival on edge {:?}: {result:?}",
                edge.edge
            );
        };
        let questions = reconstruct_terminal_obligations(&reproduced.semantic_module).unwrap();
        let failed = questions
            .obligations()
            .iter()
            .find(|question| question.obligation.id == obligation)
            .expect("the unavailable obligation is reconstructed");
        assert!(matches!(failed.owner,
            terminal_verifier::ReconstructedTerminalObligationOwner::ContractEnsures { .. }
                | terminal_verifier::ReconstructedTerminalObligationOwner::ScalarBlockInvariant { .. }),
            "wrong arrival on edge {:?} must fail guarantee preservation, not another obligation: {failed:?}", edge.edge);
    }
}

/// Preserve proof bytes while placing exact currently available premises at
/// their original citation positions. The checker-owned dependency closure
/// tells us which old positions are material; losing any such fact rejects.
/// An unused missing slot holds a duplicate current fact, never a fabricated
/// proposition. Independent replay checks this restricted current context.
fn current_facts_in_original_citation_order(
    original: &proof_admission::CertificateObligation,
    current: &proof_admission::CertificateObligation,
    accepted: &proof_admission::AcceptedFact,
) -> proof_admission::CertificateObligation {
    assert_eq!(
        original.obligation, current.obligation,
        "the exact rank question is unchanged"
    );
    assert_eq!(original.assumptions, current.assumptions);
    if let proof_admission::AcceptedFactRoute::CertificateDerived { acceptance, .. } =
        &accepted.route
    {
        for cited in &acceptance.semantic_axioms {
            assert_eq!(
                original.semantic_axioms.get(cited.index),
                Some(&cited.proposition)
            );
            assert!(
                current.semantic_axioms.contains(&cited.proposition),
                "accumulator mutation must not remove a material rank premise: {:?}",
                cited.proposition
            );
        }
    }
    let mut replay = current.clone();
    replay.semantic_axioms = original
        .semantic_axioms
        .iter()
        .map(|fact| {
            current
                .semantic_axioms
                .iter()
                .find(|candidate| *candidate == fact)
                .or_else(|| current.semantic_axioms.first())
                .expect("a removed unused citation slot needs an available current fact")
                .clone()
        })
        .collect();
    assert!(
        replay
            .semantic_axioms
            .iter()
            .all(|fact| current.semantic_axioms.contains(fact))
    );
    replay
}

#[test]
fn sequential_ranked_components_survive_serialization_and_execution() {
    use terminal_interpreter::{
        AcceptTerminalEffects, TerminalExecutionResult, TerminalScalarValue,
        TerminalStructuralInputs,
    };
    let customer = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../tests/omega/pass/proofs/runtime_sequential_ranked_components_exit/main.omg"
    ));
    // Keep the maintained machine unchanged; the corpus owns package loading
    // and the Console wrapper, while this test selects its artifact entry.
    let machine_start = customer.find("machine descend(").unwrap();
    let checked = crate::front_end::checked_program(&customer[machine_start..]);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("descend"))
        .expect("the later component retains the established invocation-relative bound");
    let semantic_bytes = terminal_codec::encode_module(&lowered.semantic_module).unwrap();
    let proof_bytes =
        terminal_codec::encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
            .unwrap();
    let module = terminal_codec::decode_module(&semantic_bytes).unwrap();
    let proofs = terminal_codec::decode_proof_section_for(&module, &proof_bytes).unwrap();
    let profile = AdmissionProfile::default();
    verify_module(&module, &proofs, &profile)
        .expect("source-free sequential-component proof replay");
    let machine = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let Some(TerminalRankedScc::Natural(cycles)) = &machine.ranked_scc else {
        panic!("the authored natural rankings must be retained");
    };
    assert_eq!(
        cycles.len(),
        2,
        "each distinct SCC retains its own rank witness"
    );
    assert_eq!(
        reconstruct_control_cycle_obligations(&module)
            .unwrap()
            .len(),
        2
    );
    assert_eq!(proofs.control_cycles.len(), 2);
    let carrier = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let unsigned = |value| TerminalScalarValue::Integer {
        scalar_type: carrier,
        value: IntegerValue::Unsigned(value),
    };
    // Entry's cycle preserves previous. Its zero-rank exit enters the later
    // component, which returns that value. No n <= previous premise exists.
    for (rank, previous, expected) in [(0, 9, 9), (3, 9, 9), (3, 0, 0)] {
        let result = terminal_interpreter::interpret_terminal_artifact_measured(
            &semantic_bytes,
            &proof_bytes,
            &profile,
            &[unsigned(rank), unsigned(previous)],
            TerminalStructuralInputs::default(),
            &mut AcceptTerminalEffects,
        )
        .expect("decoded sequential components execute without checked trees");
        assert_eq!(
            result.value(),
            TerminalExecutionResult::Scalar(unsigned(expected))
        );
    }
}
