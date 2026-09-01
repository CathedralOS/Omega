//! Exact division and remainder fixtures with their proof custody.

use super::super::VerifiedPsiOptimizationUnit;

use super::proof_certificates::{
    remainder_by_one_certificate, signed_remainder_by_negative_one_certificate,
};

fn verified_exact_self_division_or_remainder_unit(divide: bool) -> VerifiedPsiOptimizationUnit {
    use psi_core::{
        BlockId, ContractId, EdgeId, EvidenceIdentity, IntegerSign, IntegerType, IntegerValue,
        MachineId, ObligationId, OperationId, Proposition, ScalarTerm, ScalarType, ValueId,
    };
    use psi_proof_admission::{
        CertificateEnvelope, EvidenceRoute, ProofNode, ProofRule, ProofSystemMarker,
    };
    use psi_terminal::{
        Block, MachineContract, Operation, OperationKind, OperationResult, TerminalMachine,
        TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration, VocabularyMarker,
    };
    use psi_terminal_verifier::{ObligationEvidence, ProofBundle};

    let machine = MachineId::new(431).unwrap();
    let block = BlockId::new(432).unwrap();
    let operand = ValueId::new(433).unwrap();
    let remainder = ValueId::new(434).unwrap();
    let result = ValueId::new(435).unwrap();
    let obligation = ObligationId::new(436).unwrap();
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let scalar_type = ScalarType::Integer(integer);
    let declaration = |id| ValueDeclaration { id, scalar_type };
    let one = ScalarTerm::integer(integer, IntegerValue::Unsigned(1)).unwrap();
    let goal = Proposition::LessOrEqual(one, ScalarTerm::value(operand, scalar_type));
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
            parameters: vec![declaration(operand)],
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
                operations: vec![Operation {
                    id: OperationId::new(437).unwrap(),
                    result: OperationResult::Scalar(declaration(remainder)),
                    kind: if divide {
                        OperationKind::ExactIntegerDivide {
                            left: operand,
                            right: operand,
                            obligation,
                        }
                    } else {
                        OperationKind::ExactIntegerRemainder {
                            left: operand,
                            right: operand,
                            obligation,
                        }
                    },
                }],
                terminator: Terminator::Return {
                    cleanup_actions: Vec::new(),
                    edge: EdgeId::new(438).unwrap(),
                    value: remainder,
                },
            }],
            contract: MachineContract {
                id: ContractId::new(439).unwrap(),
                crash_routes: Vec::new(),
                requires: vec![goal.clone()],
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
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(440).unwrap(),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: goal,
                    rule: ProofRule::Assumption { index: 0 },
                },
            }),
        }],
    };
    let semantic = psi_terminal_codec::encode_module(&module).unwrap();
    let proof = psi_terminal_codec::encode_proof_bundle(&proof).unwrap();
    let input = omega_psi_to_abstract_operations::lower_artifact_sections_for_optimization(
        &semantic,
        &proof,
        &psi_proof_admission::AdmissionProfile::default(),
    )
    .unwrap();
    omega_psi_to_abstract_operations::build_verified_psi_optimization_unit(
        input,
        psi_terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .unwrap()
}

pub(in crate::pass_manager::tests) fn verified_exact_remainder_by_one_unit()
-> VerifiedPsiOptimizationUnit {
    use psi_core::{
        BlockId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId,
        ObligationId, OperationId, ScalarType, ValueId,
    };
    use psi_terminal::{
        Block, MachineContract, Operation, OperationKind, OperationResult, TerminalMachine,
        TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration, VocabularyMarker,
    };
    use psi_terminal_verifier::{ObligationEvidence, ProofBundle};

    let machine = MachineId::new(451).unwrap();
    let block = BlockId::new(452).unwrap();
    let operand = ValueId::new(453).unwrap();
    let one = ValueId::new(454).unwrap();
    let remainder = ValueId::new(455).unwrap();
    let result = ValueId::new(456).unwrap();
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
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            id: machine,
            attachment: None,
            parameters: vec![declaration(operand)],
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
                        id: OperationId::new(458).unwrap(),
                        result: OperationResult::Scalar(declaration(one)),
                        kind: OperationKind::IntegerConstant {
                            value: IntegerValue::Unsigned(1),
                        },
                    },
                    Operation {
                        id: OperationId::new(459).unwrap(),
                        result: OperationResult::Scalar(declaration(remainder)),
                        kind: OperationKind::ExactIntegerRemainder {
                            left: operand,
                            right: one,
                            obligation,
                        },
                    },
                ],
                terminator: Terminator::Return {
                    cleanup_actions: Vec::new(),
                    edge: EdgeId::new(460).unwrap(),
                    value: remainder,
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
            route: remainder_by_one_certificate(integer, one),
        }],
    };
    let semantic = psi_terminal_codec::encode_module(&module).unwrap();
    let proof = psi_terminal_codec::encode_proof_bundle(&proof).unwrap();
    let input = omega_psi_to_abstract_operations::lower_artifact_sections_for_optimization(
        &semantic,
        &proof,
        &psi_proof_admission::AdmissionProfile::default(),
    )
    .unwrap();
    omega_psi_to_abstract_operations::build_verified_psi_optimization_unit(
        input,
        psi_terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .unwrap()
}

pub(in crate::pass_manager::tests) fn verified_exact_signed_remainder_by_negative_one_unit()
-> VerifiedPsiOptimizationUnit {
    use psi_core::{
        BlockId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId,
        ObligationId, OperationId, ScalarType, ValueId,
    };
    use psi_terminal::{
        Block, MachineContract, Operation, OperationKind, OperationResult, TerminalMachine,
        TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration, VocabularyMarker,
    };
    use psi_terminal_verifier::{ObligationEvidence, ProofBundle};

    let machine = MachineId::new(471).unwrap();
    let block = BlockId::new(472).unwrap();
    let operand = ValueId::new(473).unwrap();
    let negative_one = ValueId::new(474).unwrap();
    let remainder = ValueId::new(475).unwrap();
    let result = ValueId::new(476).unwrap();
    let obligation = ObligationId::new(477).unwrap();
    let integer = IntegerType::new(IntegerSign::Signed, 8).unwrap();
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
                        id: OperationId::new(478).unwrap(),
                        result: OperationResult::Scalar(declaration(operand)),
                        kind: OperationKind::IntegerConstant {
                            value: IntegerValue::Signed(7),
                        },
                    },
                    Operation {
                        id: OperationId::new(479).unwrap(),
                        result: OperationResult::Scalar(declaration(negative_one)),
                        kind: OperationKind::IntegerConstant {
                            value: IntegerValue::Signed(-1),
                        },
                    },
                    Operation {
                        id: OperationId::new(480).unwrap(),
                        result: OperationResult::Scalar(declaration(remainder)),
                        kind: OperationKind::ExactIntegerRemainder {
                            left: operand,
                            right: negative_one,
                            obligation,
                        },
                    },
                ],
                terminator: Terminator::Return {
                    cleanup_actions: Vec::new(),
                    edge: EdgeId::new(481).unwrap(),
                    value: remainder,
                },
            }],
            contract: MachineContract {
                id: ContractId::new(482).unwrap(),
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
            route: signed_remainder_by_negative_one_certificate(integer, operand, negative_one),
        }],
    };
    let semantic = psi_terminal_codec::encode_module(&module).unwrap();
    let proof = psi_terminal_codec::encode_proof_bundle(&proof).unwrap();
    let input = omega_psi_to_abstract_operations::lower_artifact_sections_for_optimization(
        &semantic,
        &proof,
        &psi_proof_admission::AdmissionProfile::default(),
    )
    .unwrap();
    omega_psi_to_abstract_operations::build_verified_psi_optimization_unit(
        input,
        psi_terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .unwrap()
}

pub(in crate::pass_manager::tests) fn verified_exact_self_remainder_unit()
-> VerifiedPsiOptimizationUnit {
    verified_exact_self_division_or_remainder_unit(false)
}

pub(in crate::pass_manager::tests) fn verified_exact_self_divide_unit()
-> VerifiedPsiOptimizationUnit {
    verified_exact_self_division_or_remainder_unit(true)
}
