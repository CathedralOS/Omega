//! Widened exact arithmetic artifacts that exercise exact-literal folding and selection.

use super::baseline::request;
use crate::tests::{
    AdmissionProfile, Block, BlockId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue,
    MachineContract, MachineId, NativeTarget, ObligationId, Operation, OperationId, OperationKind,
    OperationResult, Optimization, OptimizationSelections, ScalarType,
    StagedOptimizedSelectedInstructions, SuccessorEdge, TerminalMachine, TerminalMachineResult,
    TerminalModule, Terminator, ValueDeclaration, ValueId, VocabularyMarker,
    lower_optimized_to_target_operations, operation_proof_bundle, optimize_artifact_sections,
    stage_optimized_instruction_selection,
};

pub(crate) fn conditional_widened_u8_exact_add_artifact() -> (Vec<u8>, Vec<u8>) {
    conditional_widened_u8_exact_add_artifact_with_values([200, 55], [254, 1])
}

pub(crate) fn conditional_widened_u8_exact_add_artifact_with_values(
    when_true_values: [u128; 2],
    when_false_values: [u128; 2],
) -> (Vec<u8>, Vec<u8>) {
    conditional_widened_u8_exact_binary_artifact_with_values(
        false,
        when_true_values,
        when_false_values,
    )
}

pub(crate) fn conditional_widened_u8_exact_subtract_artifact() -> (Vec<u8>, Vec<u8>) {
    conditional_widened_u8_exact_binary_artifact_with_values(true, [255, 0], [200, 55])
}

pub(crate) fn conditional_widened_u8_exact_subtract_artifact_with_values(
    when_true_values: [u128; 2],
    when_false_values: [u128; 2],
) -> (Vec<u8>, Vec<u8>) {
    conditional_widened_u8_exact_binary_artifact_with_values(
        true,
        when_true_values,
        when_false_values,
    )
}

pub(crate) fn conditional_widened_u8_exact_binary_artifact_with_values(
    subtract: bool,
    when_true_values: [u128; 2],
    when_false_values: [u128; 2],
) -> (Vec<u8>, Vec<u8>) {
    let machine = MachineId::new(5_101).unwrap();
    let entry = BlockId::new(5_102).unwrap();
    let when_true = BlockId::new(5_103).unwrap();
    let when_false = BlockId::new(5_104).unwrap();
    let condition = ValueId::new(5_105).unwrap();
    let true_left = ValueId::new(5_106).unwrap();
    let true_right = ValueId::new(5_107).unwrap();
    let true_narrow_sum = ValueId::new(5_108).unwrap();
    let true_wide_sum = ValueId::new(5_109).unwrap();
    let false_left = ValueId::new(5_110).unwrap();
    let false_right = ValueId::new(5_111).unwrap();
    let false_narrow_sum = ValueId::new(5_112).unwrap();
    let false_wide_sum = ValueId::new(5_113).unwrap();
    let result = ValueId::new(5_114).unwrap();
    let true_left_operation = OperationId::new(5_121).unwrap();
    let true_right_operation = OperationId::new(5_122).unwrap();
    let true_add_operation = OperationId::new(5_123).unwrap();
    let true_widen_operation = OperationId::new(5_124).unwrap();
    let false_left_operation = OperationId::new(5_125).unwrap();
    let false_right_operation = OperationId::new(5_126).unwrap();
    let false_add_operation = OperationId::new(5_127).unwrap();
    let false_widen_operation = OperationId::new(5_128).unwrap();
    let true_obligation = ObligationId::new(5_131).unwrap();
    let false_obligation = ObligationId::new(5_132).unwrap();
    let u8_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap());
    let u64_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let declaration = |id, scalar_type| ValueDeclaration { id, scalar_type };
    let operation = |id, result, scalar_type, kind| Operation {
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
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            id: machine,
            attachment: None,
            parameters: vec![declaration(condition, ScalarType::Boolean)],
            structural_parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(declaration(result, u64_type)),
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
                            edge: EdgeId::new(5_141).unwrap(),
                            target: when_true,
                            arguments: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                        when_false: SuccessorEdge {
                            edge: EdgeId::new(5_142).unwrap(),
                            target: when_false,
                            arguments: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                    },
                },
                Block {
                    id: when_true,
                    parameters: Vec::new(),
                    operations: vec![
                        operation(
                            true_left_operation,
                            true_left,
                            u8_type,
                            OperationKind::IntegerConstant {
                                value: IntegerValue::Unsigned(when_true_values[0]),
                            },
                        ),
                        operation(
                            true_right_operation,
                            true_right,
                            u8_type,
                            OperationKind::IntegerConstant {
                                value: IntegerValue::Unsigned(when_true_values[1]),
                            },
                        ),
                        operation(
                            true_add_operation,
                            true_narrow_sum,
                            u8_type,
                            if subtract {
                                OperationKind::ExactIntegerSubtract {
                                    left: true_left,
                                    right: true_right,
                                    obligation: true_obligation,
                                }
                            } else {
                                OperationKind::ExactIntegerAdd {
                                    left: true_left,
                                    right: true_right,
                                    obligation: true_obligation,
                                }
                            },
                        ),
                        operation(
                            true_widen_operation,
                            true_wide_sum,
                            u64_type,
                            OperationKind::IntegerWiden {
                                operand: true_narrow_sum,
                            },
                        ),
                    ],
                    terminator: Terminator::Return {
                        edge: EdgeId::new(5_143).unwrap(),
                        value: true_wide_sum,
                        cleanup_actions: Vec::new(),
                    },
                },
                Block {
                    id: when_false,
                    parameters: Vec::new(),
                    operations: vec![
                        operation(
                            false_left_operation,
                            false_left,
                            u8_type,
                            OperationKind::IntegerConstant {
                                value: IntegerValue::Unsigned(when_false_values[0]),
                            },
                        ),
                        operation(
                            false_right_operation,
                            false_right,
                            u8_type,
                            OperationKind::IntegerConstant {
                                value: IntegerValue::Unsigned(when_false_values[1]),
                            },
                        ),
                        operation(
                            false_add_operation,
                            false_narrow_sum,
                            u8_type,
                            if subtract {
                                OperationKind::ExactIntegerSubtract {
                                    left: false_left,
                                    right: false_right,
                                    obligation: false_obligation,
                                }
                            } else {
                                OperationKind::ExactIntegerAdd {
                                    left: false_left,
                                    right: false_right,
                                    obligation: false_obligation,
                                }
                            },
                        ),
                        operation(
                            false_widen_operation,
                            false_wide_sum,
                            u64_type,
                            OperationKind::IntegerWiden {
                                operand: false_narrow_sum,
                            },
                        ),
                    ],
                    terminator: Terminator::Return {
                        edge: EdgeId::new(5_144).unwrap(),
                        value: false_wide_sum,
                        cleanup_actions: Vec::new(),
                    },
                },
            ],
            contract: MachineContract {
                id: ContractId::new(5_151).unwrap(),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    let proof = operation_proof_bundle(&module);
    (
        terminal_codec::encode_module(&module).unwrap(),
        terminal_codec::encode_proof_bundle(&proof).unwrap(),
    )
}

pub(crate) fn staged_widened_u8_exact_add_conditional(
    target: NativeTarget,
) -> StagedOptimizedSelectedInstructions {
    let (semantic, proof) = conditional_widened_u8_exact_add_artifact();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        request(OptimizationSelections::new([Optimization::CopyPropagation]).unwrap()),
    )
    .unwrap();
    let target = lower_optimized_to_target_operations(optimized, target).unwrap();
    stage_optimized_instruction_selection(target).unwrap()
}

pub(crate) fn staged_widened_u8_exact_subtract_conditional(
    target: NativeTarget,
) -> StagedOptimizedSelectedInstructions {
    let (semantic, proof) = conditional_widened_u8_exact_subtract_artifact();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        request(OptimizationSelections::new([Optimization::CopyPropagation]).unwrap()),
    )
    .unwrap();
    let target = lower_optimized_to_target_operations(optimized, target).unwrap();
    stage_optimized_instruction_selection(target).unwrap()
}
