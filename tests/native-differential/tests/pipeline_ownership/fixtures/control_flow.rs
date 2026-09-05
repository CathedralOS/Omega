//! Conditional and control-flow-cleanup artifact fixtures.

use crate::tests::*;

pub(crate) fn conditional_forwarded_parameter_artifact() -> (Vec<u8>, Vec<u8>) {
    let machine = MachineId::new(4_001).unwrap();
    let entry = BlockId::new(4_002).unwrap();
    let when_true = BlockId::new(4_003).unwrap();
    let when_false = BlockId::new(4_004).unwrap();
    let condition = ValueId::new(4_005).unwrap();
    let forwarded = ValueId::new(4_006).unwrap();
    let result = ValueId::new(4_007).unwrap();
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let declaration = |id, scalar_type| ValueDeclaration { id, scalar_type };
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
            parameters: vec![
                declaration(condition, ScalarType::Boolean),
                declaration(forwarded, scalar_type),
            ],
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
                            edge: EdgeId::new(4_011).unwrap(),
                            target: when_true,
                            arguments: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                        when_false: SuccessorEdge {
                            edge: EdgeId::new(4_012).unwrap(),
                            target: when_false,
                            arguments: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                    },
                },
                Block {
                    id: when_true,
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::Return {
                        edge: EdgeId::new(4_013).unwrap(),
                        value: forwarded,
                        cleanup_actions: Vec::new(),
                    },
                },
                Block {
                    id: when_false,
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::Return {
                        edge: EdgeId::new(4_014).unwrap(),
                        value: forwarded,
                        cleanup_actions: Vec::new(),
                    },
                },
            ],
            contract: MachineContract {
                id: ContractId::new(4_015).unwrap(),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    let proof = ProofBundle {
        recursive_components: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: Vec::new(),
    };
    (
        terminal_codec::encode_module(&module).unwrap(),
        terminal_codec::encode_proof_bundle(&proof).unwrap(),
    )
}

pub(crate) fn constant_conditional_prune_artifact() -> (Vec<u8>, Vec<u8>) {
    let machine = MachineId::new(4_101).unwrap();
    let entry = BlockId::new(4_102).unwrap();
    let when_true = BlockId::new(4_103).unwrap();
    let when_false = BlockId::new(4_104).unwrap();
    let condition = ValueId::new(4_105).unwrap();
    let forwarded = ValueId::new(4_106).unwrap();
    let result = ValueId::new(4_107).unwrap();
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let declaration = |id, scalar_type| ValueDeclaration { id, scalar_type };
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
            parameters: vec![declaration(forwarded, scalar_type)],
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
                    operations: vec![Operation {
                        id: OperationId::new(4_108).unwrap(),
                        result: OperationResult::Scalar(declaration(
                            condition,
                            ScalarType::Boolean,
                        )),
                        kind: OperationKind::BooleanConstant { value: true },
                    }],
                    terminator: Terminator::Conditional {
                        condition,
                        when_true: SuccessorEdge {
                            edge: EdgeId::new(4_111).unwrap(),
                            target: when_true,
                            arguments: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                        when_false: SuccessorEdge {
                            edge: EdgeId::new(4_112).unwrap(),
                            target: when_false,
                            arguments: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                    },
                },
                Block {
                    id: when_true,
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::Return {
                        edge: EdgeId::new(4_113).unwrap(),
                        value: forwarded,
                        cleanup_actions: Vec::new(),
                    },
                },
                Block {
                    id: when_false,
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::Return {
                        edge: EdgeId::new(4_114).unwrap(),
                        value: forwarded,
                        cleanup_actions: Vec::new(),
                    },
                },
            ],
            contract: MachineContract {
                id: ContractId::new(4_115).unwrap(),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    let proof = ProofBundle {
        recursive_components: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: Vec::new(),
    };
    (
        terminal_codec::encode_module(&module).unwrap(),
        terminal_codec::encode_proof_bundle(&proof).unwrap(),
    )
}

pub(crate) fn linear_empty_block_artifact() -> (Vec<u8>, Vec<u8>) {
    let machine = MachineId::new(4_201).unwrap();
    let entry = BlockId::new(4_202).unwrap();
    let empty = BlockId::new(4_203).unwrap();
    let target = BlockId::new(4_204).unwrap();
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
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Unit,
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
                    terminator: Terminator::Jump {
                        edge: EdgeId::new(4_211).unwrap(),
                        target: empty,
                        arguments: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                },
                Block {
                    id: empty,
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::Jump {
                        edge: EdgeId::new(4_212).unwrap(),
                        target,
                        arguments: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                },
                Block {
                    id: target,
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::ReturnUnit {
                        edge: EdgeId::new(4_213).unwrap(),
                        trivial_affine_discards: Vec::new(),
                    },
                },
            ],
            contract: MachineContract {
                id: ContractId::new(4_215).unwrap(),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    let proof = ProofBundle {
        recursive_components: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: Vec::new(),
    };
    (
        terminal_codec::encode_module(&module).unwrap(),
        terminal_codec::encode_proof_bundle(&proof).unwrap(),
    )
}

pub(crate) fn adjacent_block_merge_artifact() -> (Vec<u8>, Vec<u8>) {
    let machine = MachineId::new(4_251).unwrap();
    let entry = BlockId::new(4_252).unwrap();
    let target = BlockId::new(4_253).unwrap();
    let input = ValueId::new(4_254).unwrap();
    let forwarded = ValueId::new(4_255).unwrap();
    let result = ValueId::new(4_256).unwrap();
    let computed = ValueId::new(4_261).unwrap();
    let boolean = |id| ValueDeclaration {
        id,
        scalar_type: ScalarType::Boolean,
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
            parameters: vec![boolean(input)],
            structural_parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(boolean(result)),
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
                    terminator: Terminator::Jump {
                        edge: EdgeId::new(4_257).unwrap(),
                        target,
                        arguments: vec![input],
                        trivial_affine_discards: Vec::new(),
                    },
                },
                Block {
                    id: target,
                    parameters: vec![boolean(forwarded)],
                    operations: vec![Operation {
                        id: OperationId::new(4_258).unwrap(),
                        result: OperationResult::Scalar(boolean(computed)),
                        kind: OperationKind::BooleanNot { operand: forwarded },
                    }],
                    terminator: Terminator::Return {
                        edge: EdgeId::new(4_259).unwrap(),
                        value: computed,
                        cleanup_actions: Vec::new(),
                    },
                },
            ],
            contract: MachineContract {
                id: ContractId::new(4_260).unwrap(),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    let proof = ProofBundle {
        recursive_components: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: Vec::new(),
    };
    (
        terminal_codec::encode_module(&module).unwrap(),
        terminal_codec::encode_proof_bundle(&proof).unwrap(),
    )
}

pub(crate) fn adjacent_conditional_merge_artifact() -> (Vec<u8>, Vec<u8>) {
    let machine = MachineId::new(4_271).unwrap();
    let entry = BlockId::new(4_272).unwrap();
    let decision = BlockId::new(4_273).unwrap();
    let left = BlockId::new(4_274).unwrap();
    let right = BlockId::new(4_275).unwrap();
    let condition = ValueId::new(4_276).unwrap();
    let forwarded = ValueId::new(4_277).unwrap();
    let boolean = |id| ValueDeclaration {
        id,
        scalar_type: ScalarType::Boolean,
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
            parameters: vec![boolean(condition)],
            structural_parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Unit,
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
                    terminator: Terminator::Jump {
                        edge: EdgeId::new(4_278).unwrap(),
                        target: decision,
                        arguments: vec![condition],
                        trivial_affine_discards: Vec::new(),
                    },
                },
                Block {
                    id: decision,
                    parameters: vec![boolean(forwarded)],
                    operations: Vec::new(),
                    terminator: Terminator::Conditional {
                        condition: forwarded,
                        when_true: SuccessorEdge {
                            edge: EdgeId::new(4_279).unwrap(),
                            target: left,
                            arguments: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                        when_false: SuccessorEdge {
                            edge: EdgeId::new(4_280).unwrap(),
                            target: right,
                            arguments: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                    },
                },
                Block {
                    id: left,
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::ReturnUnit {
                        edge: EdgeId::new(4_281).unwrap(),
                        trivial_affine_discards: Vec::new(),
                    },
                },
                Block {
                    id: right,
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::ReturnUnit {
                        edge: EdgeId::new(4_282).unwrap(),
                        trivial_affine_discards: Vec::new(),
                    },
                },
            ],
            contract: MachineContract {
                id: ContractId::new(4_283).unwrap(),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    let proof = ProofBundle {
        recursive_components: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: Vec::new(),
    };
    (
        terminal_codec::encode_module(&module).unwrap(),
        terminal_codec::encode_proof_bundle(&proof).unwrap(),
    )
}

pub(crate) fn path_qualified_empty_block_artifact() -> (Vec<u8>, Vec<u8>) {
    let machine = MachineId::new(4_301).unwrap();
    let entry = BlockId::new(4_302).unwrap();
    let left = BlockId::new(4_303).unwrap();
    let right = BlockId::new(4_304).unwrap();
    let empty = BlockId::new(4_305).unwrap();
    let target = BlockId::new(4_306).unwrap();
    let condition = ValueId::new(4_307).unwrap();
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
            parameters: vec![ValueDeclaration {
                id: condition,
                scalar_type: ScalarType::Boolean,
            }],
            structural_parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Unit,
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
                            edge: EdgeId::new(4_311).unwrap(),
                            target: left,
                            arguments: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                        when_false: SuccessorEdge {
                            edge: EdgeId::new(4_312).unwrap(),
                            target: right,
                            arguments: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                    },
                },
                Block {
                    id: left,
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::Jump {
                        edge: EdgeId::new(4_313).unwrap(),
                        target: empty,
                        arguments: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                },
                Block {
                    id: right,
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::Jump {
                        edge: EdgeId::new(4_314).unwrap(),
                        target: empty,
                        arguments: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                },
                Block {
                    id: empty,
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::Jump {
                        edge: EdgeId::new(4_315).unwrap(),
                        target,
                        arguments: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                },
                Block {
                    id: target,
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::ReturnUnit {
                        edge: EdgeId::new(4_316).unwrap(),
                        trivial_affine_discards: Vec::new(),
                    },
                },
            ],
            contract: MachineContract {
                id: ContractId::new(4_317).unwrap(),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    let proof = ProofBundle {
        recursive_components: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: Vec::new(),
    };
    (
        terminal_codec::encode_module(&module).unwrap(),
        terminal_codec::encode_proof_bundle(&proof).unwrap(),
    )
}

pub(crate) fn staged_forwarded_conditional(
    target: NativeTarget,
) -> StagedOptimizedSelectedInstructions {
    let (semantic, proof) = conditional_forwarded_parameter_artifact();
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
