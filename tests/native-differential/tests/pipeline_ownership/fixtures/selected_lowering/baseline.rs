//! Baseline exact-binary artifacts and their selected-stage carriers.

use crate::tests::{
    AdmissionProfile, AllocatorAvailabilityPolicy, Block, BlockId, ContractId, EdgeId,
    ExplicitOptimizationRequest, IntegerSign, IntegerType, IntegerValue, MachineContract,
    MachineId, NativeTarget, ObligationId, Operation, OperationId, OperationKind, OperationResult,
    Optimization, OptimizationSelections, OptimizationWorkBudget, OptimizedTargetLoweringRequest,
    ScalarType, StagedOptimizedAllocationLegality, StagedOptimizedSelectedInstructions,
    SuccessorEdge, TerminalMachine, TerminalMachineResult, TerminalModule, Terminator,
    ValueDeclaration, ValueId, VocabularyMarker, budget, lower_optimized_to_target_operations,
    materialize_allocator_availability, operation_proof_bundle, optimize_artifact_sections,
    stage_optimized_allocation_legality_with_availability, stage_optimized_instruction_selection,
    stage_optimized_live_ranges, stage_optimized_liveness,
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
    let declaration = |id, scalar_type| ValueDeclaration {
        qualifications: Default::default(),
        id,
        scalar_type,
    };
    let integer_operation = |id, result, kind| Operation {
        static_reach_binding: None,
        id,
        result: OperationResult::Scalar(declaration(result, scalar_type)),
        kind,
    };
    let module = TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
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
            closed_reach_application: None,
            declared_service_reach: Vec::new(),
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
                    erased_scalar_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: entry,
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::Conditional {
                        condition,
                        when_true: SuccessorEdge {
                            erased_arguments: Vec::new(),
                            structural_arguments: Vec::new(),
                            edge: EdgeId::new(5_041).unwrap(),
                            target: when_true,
                            arguments: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                        when_false: SuccessorEdge {
                            erased_arguments: Vec::new(),
                            structural_arguments: Vec::new(),
                            edge: EdgeId::new(5_042).unwrap(),
                            target: when_false,
                            arguments: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                    },
                },
                Block {
                    erased_scalar_formals: Vec::new(),
                    structural_parameters: Vec::new(),
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
                    erased_scalar_formals: Vec::new(),
                    structural_parameters: Vec::new(),
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
                erased_scalar_formals: Vec::new(),
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
        terminal_codec::encode_module(&module).unwrap(),
        terminal_codec::encode_proof_section(&module, &proof).unwrap(),
    )
}

/// Single-block `literal + literal` then `register + literal` chain. Each
/// `MaterializeI64` is defined immediately before its sole `ExactAddI64`
/// consumer, so under a one-view unconstrained allowlist each stranded
/// literal is the spill choice's own incoming victim (`selected_victim ==
/// incoming`, role `Incoming`) and the adjacent add admits the
/// `EXACT_ADD_IMMEDIATE_U12` pair — the literal-fold staging path has a real
/// step to apply.
pub(crate) fn single_block_exact_add_fold_artifact() -> (Vec<u8>, Vec<u8>) {
    let machine = MachineId::new(5_301).unwrap();
    let entry = BlockId::new(5_302).unwrap();
    let left = ValueId::new(5_305).unwrap();
    let right = ValueId::new(5_306).unwrap();
    let inner = ValueId::new(5_307).unwrap();
    let literal = ValueId::new(5_308).unwrap();
    let result = ValueId::new(5_309).unwrap();
    let sum = ValueId::new(5_310).unwrap();
    let left_operation = OperationId::new(5_321).unwrap();
    let right_operation = OperationId::new(5_322).unwrap();
    let inner_operation = OperationId::new(5_323).unwrap();
    let literal_operation = OperationId::new(5_324).unwrap();
    let result_operation = OperationId::new(5_325).unwrap();
    let inner_obligation = ObligationId::new(5_331).unwrap();
    let result_obligation = ObligationId::new(5_332).unwrap();
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let declaration = |id, scalar_type| ValueDeclaration {
        qualifications: Default::default(),
        id,
        scalar_type,
    };
    let integer_operation = |id, result, kind| Operation {
        static_reach_binding: None,
        id,
        result: OperationResult::Scalar(declaration(result, scalar_type)),
        kind,
    };
    let module = TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
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
            closed_reach_application: None,
            declared_service_reach: Vec::new(),
            id: machine,
            attachment: None,
            parameters: Vec::new(),
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
            blocks: vec![Block {
                erased_scalar_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: entry,
                parameters: Vec::new(),
                operations: vec![
                    integer_operation(
                        left_operation,
                        left,
                        OperationKind::IntegerConstant {
                            value: IntegerValue::Unsigned(5),
                        },
                    ),
                    integer_operation(
                        right_operation,
                        right,
                        OperationKind::IntegerConstant {
                            value: IntegerValue::Unsigned(7),
                        },
                    ),
                    integer_operation(
                        inner_operation,
                        inner,
                        OperationKind::ExactIntegerAdd {
                            left,
                            right,
                            obligation: inner_obligation,
                        },
                    ),
                    integer_operation(
                        literal_operation,
                        literal,
                        OperationKind::IntegerConstant {
                            value: IntegerValue::Unsigned(3),
                        },
                    ),
                    integer_operation(
                        result_operation,
                        sum,
                        OperationKind::ExactIntegerAdd {
                            left: inner,
                            right: literal,
                            obligation: result_obligation,
                        },
                    ),
                ],
                terminator: Terminator::Return {
                    edge: EdgeId::new(5_341).unwrap(),
                    value: sum,
                    cleanup_actions: Vec::new(),
                },
            }],
            contract: MachineContract {
                erased_scalar_formals: Vec::new(),
                id: ContractId::new(5_351).unwrap(),
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
        terminal_codec::encode_proof_section(&module, &proof).unwrap(),
    )
}

pub(crate) fn staged_single_block_exact_add_fold(
    target: NativeTarget,
) -> StagedOptimizedSelectedInstructions {
    let (semantic, proof) = single_block_exact_add_fold_artifact();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        ExplicitOptimizationRequest::new(
            OptimizationSelections::new([Optimization::CopyPropagation]).unwrap(),
            budget(),
        )
        .unwrap(),
    )
    .unwrap();
    let target = lower_optimized_to_target_operations(
        optimized,
        OptimizedTargetLoweringRequest::new(target),
    )
    .unwrap();
    stage_optimized_instruction_selection(target).unwrap()
}

/// Stage allocation legality for the single-block exact-add fold program with
/// allocator availability pinned to the result view. One unconstrained view
/// strands each `MaterializeI64`: its range ends at the adjacent `ExactAddI64`
/// it feeds, tying the seated contender's end, and its higher virtual register
/// makes it the choice's own victim (`selected_victim == incoming`, role
/// `Incoming`). The adjacent add admits `EXACT_ADD_IMMEDIATE_U12`, so each
/// fold step exists.
pub(crate) fn staged_single_block_exact_add_fold_legality(
    target: NativeTarget,
) -> StagedOptimizedAllocationLegality {
    let ranges = stage_optimized_live_ranges(
        stage_optimized_liveness(staged_single_block_exact_add_fold(target)).unwrap(),
    )
    .unwrap();
    let environment = ranges
        .liveness_stage()
        .selected_stage()
        .register_environment();
    // The single allowlisted view is the pinned result view itself: the
    // returned sum's pinned point still intersects it, while each literal's
    // incoming point finds the view already occupied.
    let view_name = match target.architecture {
        target::Architecture::X86_64 => "rax",
        _ => "x0",
    };
    let availability = materialize_allocator_availability(
        environment.identity(),
        environment.target(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        &environment.allocation_constraint_keys(),
        AllocatorAvailabilityPolicy::ExplicitUnconstrainedViewAllowlistV1 {
            views: vec![
                environment
                    .physical()
                    .model()
                    .view_named(view_name)
                    .unwrap()
                    .id,
            ],
        },
    )
    .unwrap();
    stage_optimized_allocation_legality_with_availability(ranges, availability).unwrap()
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
    let target = lower_optimized_to_target_operations(
        optimized,
        OptimizedTargetLoweringRequest::new(target),
    )
    .unwrap();
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
    let target = lower_optimized_to_target_operations(
        optimized,
        OptimizedTargetLoweringRequest::new(target),
    )
    .unwrap();
    stage_optimized_instruction_selection(target).unwrap()
}

pub(crate) fn request(selections: OptimizationSelections) -> ExplicitOptimizationRequest {
    ExplicitOptimizationRequest::new(selections, budget()).unwrap()
}
