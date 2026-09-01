//! Baseline exact-binary artifacts and their selected-stage carriers.

use crate::tests::{
    AdmissionProfile, Block, BlockId, ContractId, EdgeId, ExplicitOptimizationRequest, IntegerSign,
    IntegerType, IntegerValue, MachineContract, MachineId, NativeTarget, ObligationId, Operation,
    OperationId, OperationKind, OperationResult, Optimization, OptimizationSelections,
    OptimizationWorkBudget, ScalarType, StagedOptimizedSelectedInstructions, SuccessorEdge,
    TerminalMachine, TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration, ValueId,
    VocabularyMarker, budget, lower_optimized_to_target_operations, operation_proof_bundle,
    optimize_artifact_sections, stage_optimized_instruction_selection,
};

pub(crate) fn conditional_exact_binary_artifact(subtract: bool) -> (Vec<u8>, Vec<u8>) {
    let (when_true_values, when_false_values) = if subtract {
        ([13, 5], [21, 8])
    } else {
        ([7, 8], [11, 13])
    };
    conditional_exact_binary_artifact_with_values(subtract, when_true_values, when_false_values)
}

pub(crate) fn conditional_exact_binary_artifact_with_values(
    subtract: bool,
    when_true_values: [u128; 2],
    when_false_values: [u128; 2],
) -> (Vec<u8>, Vec<u8>) {
    let machine = MachineId::new(5_001).unwrap();
    let entry = BlockId::new(5_002).unwrap();
    let when_true = BlockId::new(5_003).unwrap();
    let when_false = BlockId::new(5_004).unwrap();
    let condition = ValueId::new(5_005).unwrap();
    let true_left = ValueId::new(5_006).unwrap();
    let true_right = ValueId::new(5_007).unwrap();
    let true_sum = ValueId::new(5_008).unwrap();
    let false_left = ValueId::new(5_009).unwrap();
    let false_right = ValueId::new(5_010).unwrap();
    let false_sum = ValueId::new(5_011).unwrap();
    let result = ValueId::new(5_012).unwrap();
    let true_left_operation = OperationId::new(5_021).unwrap();
    let true_right_operation = OperationId::new(5_022).unwrap();
    let true_add_operation = OperationId::new(5_023).unwrap();
    let false_left_operation = OperationId::new(5_024).unwrap();
    let false_right_operation = OperationId::new(5_025).unwrap();
    let false_add_operation = OperationId::new(5_026).unwrap();
    let true_obligation = ObligationId::new(5_031).unwrap();
    let false_obligation = ObligationId::new(5_032).unwrap();
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let declaration = |id, scalar_type| ValueDeclaration { id, scalar_type };
    let integer_operation = |id, result, kind| Operation {
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
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            id: machine,
            attachment: None,
            parameters: vec![declaration(condition, ScalarType::Boolean)],
            structural_parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(declaration(result, scalar_type)),
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
                            edge: EdgeId::new(5_041).unwrap(),
                            target: when_true,
                            arguments: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                        when_false: SuccessorEdge {
                            edge: EdgeId::new(5_042).unwrap(),
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
                        integer_operation(
                            true_left_operation,
                            true_left,
                            OperationKind::IntegerConstant {
                                value: IntegerValue::Unsigned(when_true_values[0]),
                            },
                        ),
                        integer_operation(
                            true_right_operation,
                            true_right,
                            OperationKind::IntegerConstant {
                                value: IntegerValue::Unsigned(when_true_values[1]),
                            },
                        ),
                        integer_operation(
                            true_add_operation,
                            true_sum,
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
                    ],
                    terminator: Terminator::Return {
                        edge: EdgeId::new(5_043).unwrap(),
                        value: true_sum,
                        cleanup_actions: Vec::new(),
                    },
                },
                Block {
                    id: when_false,
                    parameters: Vec::new(),
                    operations: vec![
                        integer_operation(
                            false_left_operation,
                            false_left,
                            OperationKind::IntegerConstant {
                                value: IntegerValue::Unsigned(when_false_values[0]),
                            },
                        ),
                        integer_operation(
                            false_right_operation,
                            false_right,
                            OperationKind::IntegerConstant {
                                value: IntegerValue::Unsigned(when_false_values[1]),
                            },
                        ),
                        integer_operation(
                            false_add_operation,
                            false_sum,
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
                    ],
                    terminator: Terminator::Return {
                        edge: EdgeId::new(5_044).unwrap(),
                        value: false_sum,
                        cleanup_actions: Vec::new(),
                    },
                },
            ],
            contract: MachineContract {
                id: ContractId::new(5_051).unwrap(),
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

pub(crate) fn staged_exact_add_conditional(
    target: NativeTarget,
) -> StagedOptimizedSelectedInstructions {
    staged_exact_add_conditional_with_selections(
        target,
        OptimizationSelections::new([Optimization::CopyPropagation]).unwrap(),
        budget(),
    )
}

pub(crate) fn staged_exact_add_conditional_with_selections(
    target: NativeTarget,
    selections: OptimizationSelections,
    budget: OptimizationWorkBudget,
) -> StagedOptimizedSelectedInstructions {
    let (semantic, proof) = conditional_exact_binary_artifact(false);
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        ExplicitOptimizationRequest::new(selections, budget).unwrap(),
    )
    .unwrap();
    let target = lower_optimized_to_target_operations(optimized, target).unwrap();
    stage_optimized_instruction_selection(target).unwrap()
}

pub(crate) fn staged_exact_subtract_conditional(
    target: NativeTarget,
) -> StagedOptimizedSelectedInstructions {
    staged_exact_subtract_conditional_with_selections(
        target,
        OptimizationSelections::new([Optimization::CopyPropagation]).unwrap(),
        budget(),
    )
}

pub(crate) fn staged_exact_subtract_conditional_with_selections(
    target: NativeTarget,
    selections: OptimizationSelections,
    budget: OptimizationWorkBudget,
) -> StagedOptimizedSelectedInstructions {
    let (semantic, proof) = conditional_exact_binary_artifact(true);
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        ExplicitOptimizationRequest::new(selections, budget).unwrap(),
    )
    .unwrap();
    let target = lower_optimized_to_target_operations(optimized, target).unwrap();
    stage_optimized_instruction_selection(target).unwrap()
}

pub(crate) fn request(selections: OptimizationSelections) -> ExplicitOptimizationRequest {
    ExplicitOptimizationRequest::new(selections, budget()).unwrap()
}
