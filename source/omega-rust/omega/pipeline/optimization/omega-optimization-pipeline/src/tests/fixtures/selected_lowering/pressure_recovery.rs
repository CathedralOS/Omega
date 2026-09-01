//! Active-resident pressure fixtures and their allocation/realization compositions.

use super::baseline::request;
use crate::stage_optimized_active_resident_rematerialization_function_relative_realization;
use crate::tests::{
    AdmissionProfile, Block, BlockId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue,
    MachineContract, MachineId, NativeTarget, ObligationId, Operation, OperationId, OperationKind,
    OperationResult, Optimization, OptimizationSelections, PressureRematerializationPolicy,
    RecoveryClassificationPolicy, ScalarType, SpillChoicePolicy,
    StagedAllocationRecoveryFunctionRelativeRealization,
    StagedAllocationRecoveryFunctionRelativeSource, StagedOptimizedActiveResidentRematerialization,
    StagedOptimizedActiveResidentRematerializationFunctionRelativeRealization,
    StagedOptimizedActiveResidentRematerializationResolvedSelectedFormLayout,
    StagedOptimizedAllocationLegality, StagedOptimizedPostAllocationMachinePlan,
    StagedOptimizedSelectedInstructions, SuccessorEdge, TerminalMachine, TerminalMachineResult,
    TerminalModule, Terminator, ValueDeclaration, ValueId, VocabularyMarker,
    lower_optimized_to_target_operations, operation_proof_bundle, optimize_artifact_sections,
    selected_lowering_budget, stage_allocation_recovery_function_relative_realization,
    stage_optimized_active_resident_rematerialization,
    stage_optimized_active_resident_rematerialization_resolved_selected_form_layout,
    stage_optimized_active_resident_rematerialization_selected_form_encoding,
    stage_optimized_allocation_legality_for_active_resident_immediate_u64_multi_use_rematerialization_v1,
    stage_optimized_instruction_selection, stage_optimized_live_ranges, stage_optimized_liveness,
    stage_optimized_post_allocation_machine_plan_after_active_resident_rematerialization,
};

pub(crate) fn conditional_active_resident_exact_add_chain_artifact() -> (Vec<u8>, Vec<u8>) {
    conditional_active_resident_exact_add_chain_artifact_with_literals(
        IntegerValue::Unsigned(3),
        IntegerValue::Unsigned(11),
    )
}

pub(crate) fn conditional_active_resident_exact_add_bridge_chain_artifact() -> (Vec<u8>, Vec<u8>) {
    conditional_active_resident_exact_add_chain_artifact_with_literals_and_bridge(
        IntegerValue::Unsigned(3),
        IntegerValue::Unsigned(11),
        true,
    )
}

pub(crate) fn conditional_active_resident_exact_add_chain_artifact_with_false_literal(
    false_literal: IntegerValue,
) -> (Vec<u8>, Vec<u8>) {
    conditional_active_resident_exact_add_chain_artifact_with_literals(
        IntegerValue::Unsigned(3),
        false_literal,
    )
}

pub(crate) fn conditional_active_resident_exact_add_chain_artifact_with_resident_literal(
    resident_literal: IntegerValue,
) -> (Vec<u8>, Vec<u8>) {
    conditional_active_resident_exact_add_chain_artifact_with_literals(
        resident_literal,
        IntegerValue::Unsigned(11),
    )
}

fn conditional_active_resident_exact_add_chain_artifact_with_literals(
    resident_literal: IntegerValue,
    false_literal: IntegerValue,
) -> (Vec<u8>, Vec<u8>) {
    conditional_active_resident_exact_add_chain_artifact_with_literals_and_bridge(
        resident_literal,
        false_literal,
        false,
    )
}

fn conditional_active_resident_exact_add_chain_artifact_with_literals_and_bridge(
    resident_literal: IntegerValue,
    false_literal: IntegerValue,
    retain_right_across_first_resident_use: bool,
) -> (Vec<u8>, Vec<u8>) {
    let machine = MachineId::new(5_201).unwrap();
    let entry = BlockId::new(5_202).unwrap();
    let when_true = BlockId::new(5_203).unwrap();
    let when_false = BlockId::new(5_204).unwrap();
    let condition = ValueId::new(5_205).unwrap();
    let resident = ValueId::new(5_206).unwrap();
    let left = ValueId::new(5_207).unwrap();
    let right = ValueId::new(5_208).unwrap();
    let inner = ValueId::new(5_209).unwrap();
    let middle = ValueId::new(5_210).unwrap();
    let result_value = ValueId::new(5_211).unwrap();
    let false_value = ValueId::new(5_212).unwrap();
    let machine_result = ValueId::new(5_213).unwrap();
    let bridge = ValueId::new(5_214).unwrap();
    let resident_operation = OperationId::new(5_221).unwrap();
    let left_operation = OperationId::new(5_222).unwrap();
    let right_operation = OperationId::new(5_223).unwrap();
    let inner_operation = OperationId::new(5_224).unwrap();
    let middle_operation = OperationId::new(5_225).unwrap();
    let result_operation = OperationId::new(5_226).unwrap();
    let false_operation = OperationId::new(5_227).unwrap();
    let bridge_operation = OperationId::new(5_228).unwrap();
    let inner_obligation = ObligationId::new(5_231).unwrap();
    let middle_obligation = ObligationId::new(5_232).unwrap();
    let result_obligation = ObligationId::new(5_233).unwrap();
    let bridge_obligation = ObligationId::new(5_234).unwrap();
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let declaration = |id, scalar_type| ValueDeclaration { id, scalar_type };
    let operation = |id, result, kind| Operation {
        id,
        result: OperationResult::Scalar(declaration(result, scalar_type)),
        kind,
    };
    let module = TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine,
        structural_types: Vec::new(),
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        placed_view_inputs: Vec::new(),
        reborrow_root_handoffs: Vec::new(),
        reborrow_restored_call_uses: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            id: machine,
            attachment: None,
            parameters: vec![declaration(condition, ScalarType::Boolean)],
            structural_parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(declaration(machine_result, scalar_type)),
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry,
            blocks: vec![
                Block {
                    id: entry,
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::Conditional {
                        condition,
                        when_true: SuccessorEdge {
                            edge: EdgeId::new(5_241).unwrap(),
                            target: when_true,
                            arguments: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                        when_false: SuccessorEdge {
                            edge: EdgeId::new(5_242).unwrap(),
                            target: when_false,
                            arguments: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                    },
                },
                Block {
                    id: when_true,
                    parameters: Vec::new(),
                    operations: {
                        let mut operations = vec![
                            operation(
                                resident_operation,
                                resident,
                                OperationKind::IntegerConstant {
                                    value: resident_literal,
                                },
                            ),
                            operation(
                                left_operation,
                                left,
                                OperationKind::IntegerConstant {
                                    value: IntegerValue::Unsigned(5),
                                },
                            ),
                            operation(
                                right_operation,
                                right,
                                OperationKind::IntegerConstant {
                                    value: IntegerValue::Unsigned(7),
                                },
                            ),
                            operation(
                                inner_operation,
                                inner,
                                OperationKind::ExactIntegerAdd {
                                    left,
                                    right,
                                    obligation: inner_obligation,
                                },
                            ),
                            operation(
                                middle_operation,
                                middle,
                                OperationKind::ExactIntegerAdd {
                                    left: resident,
                                    right: inner,
                                    obligation: middle_obligation,
                                },
                            ),
                        ];
                        let result_right = if retain_right_across_first_resident_use {
                            operations.push(operation(
                                bridge_operation,
                                bridge,
                                OperationKind::ExactIntegerAdd {
                                    left: right,
                                    right: middle,
                                    obligation: bridge_obligation,
                                },
                            ));
                            bridge
                        } else {
                            middle
                        };
                        operations.push(operation(
                            result_operation,
                            result_value,
                            OperationKind::ExactIntegerAdd {
                                left: resident,
                                right: result_right,
                                obligation: result_obligation,
                            },
                        ));
                        operations
                    },
                    terminator: Terminator::Return {
                        edge: EdgeId::new(5_243).unwrap(),
                        value: result_value,
                        cleanup_actions: Vec::new(),
                    },
                },
                Block {
                    id: when_false,
                    parameters: Vec::new(),
                    operations: vec![operation(
                        false_operation,
                        false_value,
                        OperationKind::IntegerConstant {
                            value: false_literal,
                        },
                    )],
                    terminator: Terminator::Return {
                        edge: EdgeId::new(5_244).unwrap(),
                        value: false_value,
                        cleanup_actions: Vec::new(),
                    },
                },
            ],
            contract: MachineContract {
                id: ContractId::new(5_251).unwrap(),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    let proof = operation_proof_bundle(&module);
    (
        psi_terminal_codec::encode_module(&module).unwrap(),
        psi_terminal_codec::encode_proof_bundle(&proof).unwrap(),
    )
}

pub(crate) fn staged_active_resident_exact_add_chain(
    target: NativeTarget,
) -> StagedOptimizedSelectedInstructions {
    staged_active_resident_exact_add_chain_with_selections(
        target,
        OptimizationSelections::new([
            Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1,
        ])
        .unwrap(),
    )
}

pub(crate) fn staged_active_resident_exact_add_bridge_chain(
    target: NativeTarget,
) -> StagedOptimizedSelectedInstructions {
    let (semantic, proof) = conditional_active_resident_exact_add_bridge_chain_artifact();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        request(
            OptimizationSelections::new([
                Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1,
            ])
            .unwrap(),
        ),
    )
    .unwrap();
    let target = lower_optimized_to_target_operations(optimized, target).unwrap();
    stage_optimized_instruction_selection(target).unwrap()
}

pub(crate) fn staged_active_resident_exact_add_chain_with_selections(
    target: NativeTarget,
    selections: OptimizationSelections,
) -> StagedOptimizedSelectedInstructions {
    let (semantic, proof) = conditional_active_resident_exact_add_chain_artifact();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        request(selections),
    )
    .unwrap();
    let target = lower_optimized_to_target_operations(optimized, target).unwrap();
    stage_optimized_instruction_selection(target).unwrap()
}

pub(crate) fn staged_active_resident_two_view_legality(
    target: NativeTarget,
) -> StagedOptimizedAllocationLegality {
    staged_active_resident_two_view_legality_with_selections(
        target,
        OptimizationSelections::new([
            Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1,
        ])
        .unwrap(),
    )
}

pub(crate) fn staged_active_resident_bridge_chain_two_view_legality(
    target: NativeTarget,
) -> StagedOptimizedAllocationLegality {
    let ranges = stage_optimized_live_ranges(
        stage_optimized_liveness(staged_active_resident_exact_add_bridge_chain(target)).unwrap(),
    )
    .unwrap();
    stage_optimized_allocation_legality_for_active_resident_immediate_u64_multi_use_rematerialization_v1(ranges)
        .unwrap()
}

pub(crate) fn staged_active_resident_two_view_legality_with_selections(
    target: NativeTarget,
    selections: OptimizationSelections,
) -> StagedOptimizedAllocationLegality {
    let ranges = stage_optimized_live_ranges(
        stage_optimized_liveness(staged_active_resident_exact_add_chain_with_selections(
            target, selections,
        ))
        .unwrap(),
    )
    .unwrap();
    stage_optimized_allocation_legality_for_active_resident_immediate_u64_multi_use_rematerialization_v1(ranges)
        .unwrap()
}

pub(crate) fn staged_active_resident_rematerialization_and_machine(
    target: NativeTarget,
) -> (
    StagedOptimizedActiveResidentRematerialization,
    StagedOptimizedPostAllocationMachinePlan,
) {
    let source = stage_optimized_active_resident_rematerialization(
        staged_active_resident_two_view_legality(target),
        SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
        RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
        PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1,
        selected_lowering_budget(),
    )
    .unwrap();
    let machine =
        stage_optimized_post_allocation_machine_plan_after_active_resident_rematerialization(
            &source,
        )
        .unwrap();
    (source, machine)
}

pub(crate) fn staged_active_resident_resolved_layout(
    target: NativeTarget,
) -> StagedOptimizedActiveResidentRematerializationResolvedSelectedFormLayout {
    let (source, machine) = staged_active_resident_rematerialization_and_machine(target);
    let pre_layout =
        stage_optimized_active_resident_rematerialization_selected_form_encoding(source, machine)
            .unwrap();
    stage_optimized_active_resident_rematerialization_resolved_selected_form_layout(pre_layout)
        .unwrap()
}

pub(crate) fn staged_active_resident_resolved_layout_with_selections(
    target: NativeTarget,
    selections: OptimizationSelections,
) -> StagedOptimizedActiveResidentRematerializationResolvedSelectedFormLayout {
    let source = stage_optimized_active_resident_rematerialization(
        staged_active_resident_two_view_legality_with_selections(target, selections),
        SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
        RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
        PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1,
        selected_lowering_budget(),
    )
    .unwrap();
    let machine =
        stage_optimized_post_allocation_machine_plan_after_active_resident_rematerialization(
            &source,
        )
        .unwrap();
    let pre_layout =
        stage_optimized_active_resident_rematerialization_selected_form_encoding(source, machine)
            .unwrap();
    stage_optimized_active_resident_rematerialization_resolved_selected_form_layout(pre_layout)
        .unwrap()
}

pub(crate) fn staged_active_resident_function_relative_realization(
    target: NativeTarget,
) -> StagedOptimizedActiveResidentRematerializationFunctionRelativeRealization {
    stage_optimized_active_resident_rematerialization_function_relative_realization(
        staged_active_resident_resolved_layout(target),
    )
    .unwrap()
}

pub(crate) fn staged_active_resident_allocation_recovery_realization(
    target: NativeTarget,
) -> StagedAllocationRecoveryFunctionRelativeRealization {
    let (source, machine) = staged_active_resident_rematerialization_and_machine(target);
    stage_allocation_recovery_function_relative_realization(
        StagedAllocationRecoveryFunctionRelativeSource::ActiveResidentRematerialization(Box::new(
            source,
        )),
        machine,
    )
    .unwrap()
}
