//! Compatible-policy local and phi-translated value-numbering fixtures.

use super::super::VerifiedPsiOptimizationUnit;

use super::proof_certificates::{
    exact_unsigned_add_certificate, exact_unsigned_shift_count_certificate,
};

pub(in crate::pass_manager::tests) fn verified_compatible_policy_cse_unit()
-> VerifiedPsiOptimizationUnit {
    use semantic_vocabulary::{
        BlockId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId,
        ObligationId, OperationId, ScalarType, ValueId,
    };
    use terminal_psi::{
        Block, MachineContract, Operation, OperationKind, OperationResult, TerminalMachine,
        TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration, VocabularyMarker,
    };
    use terminal_verifier::{ObligationEvidence, ProofBundle};

    let machine = MachineId::new(451).unwrap();
    let block = BlockId::new(452).unwrap();
    let left = ValueId::new(453).unwrap();
    let right = ValueId::new(454).unwrap();
    let leader = ValueId::new(455).unwrap();
    let redundant = ValueId::new(456).unwrap();
    let result = ValueId::new(462).unwrap();
    let obligation = ObligationId::new(457).unwrap();
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let scalar_type = ScalarType::Integer(integer);
    let declaration = |id| ValueDeclaration { id, scalar_type };
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
            result: TerminalMachineResult::Scalar(declaration(result)),
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: block,
            blocks: vec![Block {
                id: block,
                parameters: Vec::new(),
                operations: vec![
                    Operation {
                        id: OperationId::new(463).unwrap(),
                        result: OperationResult::Scalar(declaration(left)),
                        kind: OperationKind::IntegerConstant {
                            value: IntegerValue::Unsigned(7),
                        },
                    },
                    Operation {
                        id: OperationId::new(464).unwrap(),
                        result: OperationResult::Scalar(declaration(right)),
                        kind: OperationKind::IntegerConstant {
                            value: IntegerValue::Unsigned(8),
                        },
                    },
                    Operation {
                        id: OperationId::new(458).unwrap(),
                        result: OperationResult::Scalar(declaration(leader)),
                        kind: OperationKind::WrappingIntegerAdd { left, right },
                    },
                    Operation {
                        id: OperationId::new(459).unwrap(),
                        result: OperationResult::Scalar(declaration(redundant)),
                        kind: OperationKind::ExactIntegerAdd {
                            left: right,
                            right: left,
                            obligation,
                        },
                    },
                ],
                terminator: Terminator::Return {
                    cleanup_actions: Vec::new(),
                    edge: EdgeId::new(460).unwrap(),
                    value: redundant,
                },
            }],
            contract: MachineContract {
                id: ContractId::new(461).unwrap(),
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
        evidence: vec![ObligationEvidence {
            obligation,
            route: exact_unsigned_add_certificate(
                integer,
                right,
                left,
                IntegerValue::Unsigned(8),
                IntegerValue::Unsigned(7),
                1,
                0,
                457,
            ),
        }],
    };
    let semantic = terminal_codec::encode_module(&module).unwrap();
    let proof = terminal_codec::encode_proof_bundle(&proof).unwrap();
    let input = terminal_psi_to_abstract_operations::lower_artifact_sections_for_optimization(
        &semantic,
        &proof,
        &proof_admission::AdmissionProfile::default(),
    )
    .unwrap();
    terminal_psi_to_abstract_operations::build_verified_psi_optimization_unit(
        input,
        terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .unwrap()
}

pub(in crate::pass_manager::tests) fn verified_compatible_policy_phi_gvn_unit()
-> VerifiedPsiOptimizationUnit {
    use semantic_vocabulary::{
        BlockId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId,
        ObligationId, OperationId, ScalarType, ValueId,
    };
    use terminal_psi::{
        Block, MachineContract, Operation, OperationKind, OperationResult, SuccessorEdge,
        TerminalMachine, TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration,
        VocabularyMarker,
    };
    use terminal_verifier::{ObligationEvidence, ProofBundle};

    let machine = MachineId::new(501).unwrap();
    let join = BlockId::new(502).unwrap();
    let left_block = BlockId::new(503).unwrap();
    let entry = BlockId::new(504).unwrap();
    let right_block = BlockId::new(505).unwrap();
    let condition = ValueId::new(506).unwrap();
    let left_a = ValueId::new(507).unwrap();
    let left_b = ValueId::new(508).unwrap();
    let right_a = ValueId::new(509).unwrap();
    let right_b = ValueId::new(510).unwrap();
    let join_a = ValueId::new(511).unwrap();
    let join_b = ValueId::new(512).unwrap();
    let left_leader = ValueId::new(513).unwrap();
    let right_leader = ValueId::new(514).unwrap();
    let redundant = ValueId::new(515).unwrap();
    let result = ValueId::new(516).unwrap();
    let obligation = ObligationId::new(517).unwrap();
    let zero = ValueId::new(527).unwrap();
    let value_type = IntegerType::new(IntegerSign::Signed, 32).unwrap();
    let count_type = IntegerType::new(IntegerSign::Unsigned, 32).unwrap();
    let scalar_type = ScalarType::Integer(value_type);
    let count_scalar_type = ScalarType::Integer(count_type);
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
                declaration(left_a, scalar_type),
                declaration(left_b, scalar_type),
                declaration(right_a, scalar_type),
                declaration(right_b, scalar_type),
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
                    id: join,
                    parameters: vec![
                        declaration(join_a, scalar_type),
                        declaration(join_b, count_scalar_type),
                    ],
                    operations: vec![Operation {
                        id: OperationId::new(518).unwrap(),
                        result: OperationResult::Scalar(declaration(redundant, scalar_type)),
                        kind: OperationKind::ExactIntegerShiftRight {
                            value: join_a,
                            count: join_b,
                            obligation,
                        },
                    }],
                    terminator: Terminator::Return {
                        cleanup_actions: Vec::new(),
                        edge: EdgeId::new(519).unwrap(),
                        value: redundant,
                    },
                },
                Block {
                    id: left_block,
                    parameters: Vec::new(),
                    operations: vec![Operation {
                        id: OperationId::new(520).unwrap(),
                        result: OperationResult::Scalar(declaration(left_leader, scalar_type)),
                        kind: OperationKind::WrappingIntegerShiftRight {
                            value: left_a,
                            count: zero,
                        },
                    }],
                    terminator: Terminator::Jump {
                        edge: EdgeId::new(521).unwrap(),
                        target: join,
                        arguments: vec![left_a, zero],
                        residual_affine_discards: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                },
                Block {
                    id: entry,
                    parameters: Vec::new(),
                    operations: vec![Operation {
                        id: OperationId::new(528).unwrap(),
                        result: OperationResult::Scalar(declaration(zero, count_scalar_type)),
                        kind: OperationKind::IntegerConstant {
                            value: IntegerValue::Unsigned(0),
                        },
                    }],
                    terminator: Terminator::Conditional {
                        condition,
                        when_true: SuccessorEdge {
                            edge: EdgeId::new(522).unwrap(),
                            target: left_block,
                            arguments: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                        when_false: SuccessorEdge {
                            edge: EdgeId::new(523).unwrap(),
                            target: right_block,
                            arguments: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                    },
                },
                Block {
                    id: right_block,
                    parameters: Vec::new(),
                    operations: vec![Operation {
                        id: OperationId::new(524).unwrap(),
                        result: OperationResult::Scalar(declaration(right_leader, scalar_type)),
                        kind: OperationKind::WrappingIntegerShiftRight {
                            value: right_a,
                            count: zero,
                        },
                    }],
                    terminator: Terminator::Jump {
                        edge: EdgeId::new(525).unwrap(),
                        target: join,
                        arguments: vec![right_a, zero],
                        residual_affine_discards: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                },
            ],
            contract: MachineContract {
                id: ContractId::new(526).unwrap(),
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
        evidence: vec![ObligationEvidence {
            obligation,
            route: exact_unsigned_shift_count_certificate(value_type, count_type, join_b, 2, 517),
        }],
    };
    let semantic = terminal_codec::encode_module(&module).unwrap();
    let proof = terminal_codec::encode_proof_bundle(&proof).unwrap();
    let input = terminal_psi_to_abstract_operations::lower_artifact_sections_for_optimization(
        &semantic,
        &proof,
        &proof_admission::AdmissionProfile::default(),
    )
    .unwrap();
    terminal_psi_to_abstract_operations::build_verified_psi_optimization_unit(
        input,
        terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .unwrap()
}
