//! Exact arithmetic and selected-lowering fixtures.

use crate::tests::*;

pub(crate) fn conditional_exact_binary_artifact(subtract: bool) -> (Vec<u8>, Vec<u8>) {
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
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        proof_output_calls: Vec::new(),
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
                                value: IntegerValue::Unsigned(if subtract { 13 } else { 7 }),
                            },
                        ),
                        integer_operation(
                            true_right_operation,
                            true_right,
                            OperationKind::IntegerConstant {
                                value: IntegerValue::Unsigned(if subtract { 5 } else { 8 }),
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
                                value: IntegerValue::Unsigned(if subtract { 21 } else { 11 }),
                            },
                        ),
                        integer_operation(
                            false_right_operation,
                            false_right,
                            OperationKind::IntegerConstant {
                                value: IntegerValue::Unsigned(if subtract { 8 } else { 13 }),
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
    let proof = ProofBundle {
        evidence_producers: Vec::new(),
        evidence: [true_obligation, false_obligation]
            .into_iter()
            .map(|obligation| ObligationEvidence {
                obligation,
                route: EvidenceRoute::KernelDerived(PrimitiveJudgment::Truth),
            })
            .collect(),
    };
    (
        psi_terminal_codec::encode_module(&module).unwrap(),
        psi_terminal_codec::encode_proof_bundle(&proof).unwrap(),
    )
}

pub(crate) fn conditional_active_resident_exact_add_chain_artifact() -> (Vec<u8>, Vec<u8>) {
    conditional_active_resident_exact_add_chain_artifact_with_false_literal(IntegerValue::Unsigned(
        11,
    ))
}

pub(crate) fn conditional_active_resident_exact_add_chain_artifact_with_false_literal(
    false_literal: IntegerValue,
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
    let resident_operation = OperationId::new(5_221).unwrap();
    let left_operation = OperationId::new(5_222).unwrap();
    let right_operation = OperationId::new(5_223).unwrap();
    let inner_operation = OperationId::new(5_224).unwrap();
    let middle_operation = OperationId::new(5_225).unwrap();
    let result_operation = OperationId::new(5_226).unwrap();
    let false_operation = OperationId::new(5_227).unwrap();
    let inner_obligation = ObligationId::new(5_231).unwrap();
    let middle_obligation = ObligationId::new(5_232).unwrap();
    let result_obligation = ObligationId::new(5_233).unwrap();
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
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        proof_output_calls: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        closed_conformance_applications: Vec::new(),
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
                    operations: vec![
                        operation(
                            resident_operation,
                            resident,
                            OperationKind::IntegerConstant {
                                value: IntegerValue::Unsigned(3),
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
                        operation(
                            result_operation,
                            result_value,
                            OperationKind::ExactIntegerAdd {
                                left: resident,
                                right: middle,
                                obligation: result_obligation,
                            },
                        ),
                    ],
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
    let proof = ProofBundle {
        evidence_producers: Vec::new(),
        evidence: [inner_obligation, middle_obligation, result_obligation]
            .into_iter()
            .map(|obligation| ObligationEvidence {
                obligation,
                route: EvidenceRoute::KernelDerived(PrimitiveJudgment::Truth),
            })
            .collect(),
    };
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
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        proof_output_calls: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        closed_conformance_applications: Vec::new(),
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
    let proof = ProofBundle {
        evidence_producers: Vec::new(),
        evidence: [true_obligation, false_obligation]
            .into_iter()
            .map(|obligation| ObligationEvidence {
                obligation,
                route: EvidenceRoute::KernelDerived(PrimitiveJudgment::Truth),
            })
            .collect(),
    };
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
