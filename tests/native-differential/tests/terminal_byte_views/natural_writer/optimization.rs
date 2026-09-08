//! Authored grouped ranking remains authoritative through current-IR replay.
use super::*;

fn verified_writer() -> terminal_psi_to_abstract_operations::VerifiedPsiOptimizationUnit {
    let lowered = writer();
    let input = terminal_psi_to_abstract_operations::lower_artifact_sections_for_optimization(
        &terminal_codec::encode_module(&lowered.semantic_module).unwrap(),
        &terminal_codec::encode_proof_bundle(&lowered.proof_bundle).unwrap(),
        &AdmissionProfile::default(),
    )
    .unwrap();
    terminal_psi_to_abstract_operations::build_verified_psi_optimization_unit(
        input,
        terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .unwrap()
}

#[test]
fn natural_writer_optimizer_replays_verified_component_without_countdown_certificate() {
    let verified = verified_writer();
    let custody = abstract_operations_to_abstract_operations::validation::validate_verified_psi_cycle_components(&verified)
        .expect("verified natural writer must retain its actual cycle in optimizer admission");
    assert_eq!(custody.components().len(), 1);
    assert!(
        custody.ranking_certificates().certificates().is_empty(),
        "natural proof must not become a countdown certificate"
    );
    assert_eq!(
        abstract_operations_to_abstract_operations::validation::validate_transformed_psi_cycle_components(verified.input(), verified.unit()).unwrap(),
        custody,
    );
}

#[test]
fn natural_writer_optimizer_rejects_changed_prefix_cycle_and_exit_bodies() {
    let verified = verified_writer();
    let source = verified
        .input()
        .context()
        .module()
        .machines
        .iter()
        .find(|machine| machine.ranked_scc.is_some())
        .unwrap();
    for mutation in ["prefix", "cycle", "exit"] {
        let mut changed = verified.unit().clone();
        let function = changed
            .functions
            .iter_mut()
            .find(|function| function.machine == source.id)
            .unwrap();
        let node = match mutation {
            "prefix" => {
                &mut function
                    .blocks
                    .iter_mut()
                    .find(|block| block.id == function.entry)
                    .unwrap()
                    .nodes[0]
            }
            "cycle" => {
                let Some(TerminalRankedScc::Natural(components)) = &source.ranked_scc else {
                    panic!("natural");
                };
                let block = components[0].ranks[0].block;
                function
                    .blocks
                    .iter_mut()
                    .find(|candidate| candidate.id == block)
                    .unwrap()
                    .nodes
                    .last_mut()
                    .unwrap()
            }
            "exit" => function
                .blocks
                .iter_mut()
                .flat_map(|block| &mut block.nodes)
                .find(|node| {
                    matches!(
                        node.operation,
                        abstract_operations::AbstractOperation::ReturnUnit { .. }
                    )
                })
                .unwrap(),
            _ => unreachable!(),
        };
        node.operation = abstract_operations::AbstractOperation::ReturnUnit {
            psi_edge: semantic_vocabulary::EdgeId::new(u64::MAX).unwrap(),
            cleanup_actions: Vec::new(),
        };
        changed.identity = optimization_unit::recompute_psi_optimization_unit_identity(&changed);
        assert!(abstract_operations_to_abstract_operations::validation::validate_transformed_psi_cycle_components(
            verified.input(), &changed).is_err(), "changed {mutation} body");
    }
}

#[test]
fn writer_unit_calls_reject_unavailable_or_owned_literal_arguments() {
    let verified = verified_writer();
    for mutation in ["future", "owned", "missing"] {
        let mut changed = verified.unit().clone();
        let caller = changed
            .functions
            .iter_mut()
            .find(|function| function.machine == changed.entry)
            .unwrap();
        let nodes = &mut caller.blocks[0].nodes;
        let call = nodes
            .iter()
            .position(|node| {
                matches!(
                    node.operation,
                    abstract_operations::AbstractOperation::CallUnit { .. }
                )
            })
            .unwrap();
        if mutation == "future" {
            let producer = nodes
                .iter()
                .position(|node| {
                    matches!(
                        node.operation,
                        abstract_operations::AbstractOperation::EstablishByteSequenceLiteral { .. }
                    )
                })
                .unwrap();
            let literal = nodes.remove(producer);
            nodes.insert(call, literal);
        } else {
            let abstract_operations::AbstractOperation::CallUnit {
                structural_arguments,
                ..
            } = &mut nodes[call].operation
            else {
                panic!("Unit call");
            };
            if mutation == "owned" {
                structural_arguments[0].access = terminal_psi::StructuralAccess::Owned;
            } else {
                structural_arguments[0].place =
                    semantic_vocabulary::PlaceId::new(u64::MAX).unwrap();
            }
        }
        changed.identity = optimization_unit::recompute_psi_optimization_unit_identity(&changed);
        assert!(abstract_operations_to_abstract_operations::validation::validate_transformed_psi_cycle_components(
            verified.input(), &changed).is_err(), "literal mutation {mutation}");
    }
}

#[test]
fn natural_writer_legalization_requires_verified_custody_and_exact_payloads() {
    let lowered = writer();
    let selections = optimization_core::OptimizationSelections::new([]).unwrap();
    let optimized = native_realization::optimize_artifact_sections(
        &terminal_codec::encode_module(&lowered.semantic_module).unwrap(),
        &terminal_codec::encode_proof_bundle(&lowered.proof_bundle).unwrap(),
        &AdmissionProfile::default(),
        native_realization::compiler_baseline_request_v1(&selections),
    )
    .unwrap();
    let target = abstract_operations_to_target_operations::lower_optimized_to_target_operations_with_provider_executions(
        optimized, NativeTarget::macos_arm64(), &[AdmittedBoundarySettlement {
            boundary: lowered.semantic_module.boundary_machines[0].id,
            execution: AdmittedBoundaryExecution::CompilerBuiltin(
                target_operations::CompilerBuiltinExecution::HostedWriteByteI32),
            realization: target_operations::HostedWriteByteI32Realization.into(),
        }],
    ).unwrap();
    let abstracted = target.optimized();
    let legalized = target_operations_to_selected_instructions::legalize_target_operations(
        target.target_operations(),
        abstracted.plan(),
        abstracted,
    )
    .unwrap();
    assert!(
        target_operations_to_selected_instructions::legalize_target_operations(
            target.target_operations(),
            abstracted.plan(),
            abstracted.unit(),
        )
        .is_err(),
        "raw cyclic IR cannot substitute for the verified artifact"
    );
    assert!(
        target_operations_to_selected_instructions::validate_legalized_operations(
            target.target_operations(),
            abstracted.plan(),
            abstracted.unit(),
            legalized.plan().clone(),
        )
        .is_err(),
        "replay also requires proof custody"
    );
    for mutation in ["literal", "edges", "effects"] {
        let mut changed = legalized.plan().clone();
        if mutation == "literal" {
            let bytes = changed.scalar_functions.iter_mut().flat_map(|function| &mut function.blocks)
                .flat_map(|block| &mut block.instructions).find_map(|row| match &mut row.kind {
                    legalized_operations::LegalizedScalarInstructionKind::EstablishByteSequenceLiteral { bytes, .. }
                        if !bytes.is_empty() => Some(bytes),
                    _ => None,
                }).unwrap();
            bytes[0] ^= 1;
        } else {
            let function = changed
                .scalar_functions
                .iter_mut()
                .find(|function| function.machine != changed.entry)
                .unwrap();
            if mutation == "edges" {
                function.provenance.edges.clear();
            } else {
                function
                    .structural
                    .as_mut()
                    .unwrap()
                    .published_service_ceiling
                    .clear();
            }
        }
        assert!(
            target_operations_to_selected_instructions::validate_legalized_operations(
                target.target_operations(),
                abstracted.plan(),
                abstracted,
                changed,
            )
            .is_err(),
            "changed {mutation} must not survive replay"
        );
    }
}
