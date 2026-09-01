use super::common::scalar_terminal_artifact;
use super::*;

pub(crate) fn integer_literal_return_artifact(
    integer_type: IntegerType,
    value: IntegerValue,
) -> (Vec<u8>, Vec<u8>) {
    scalar_terminal_artifact(
        ScalarType::Integer(integer_type),
        Vec::new(),
        Some(OperationKind::IntegerConstant { value }),
        None,
        None,
    )
}

pub(crate) fn boolean_literal_return_artifact(value: bool) -> (Vec<u8>, Vec<u8>) {
    scalar_terminal_artifact(
        ScalarType::Boolean,
        Vec::new(),
        Some(OperationKind::BooleanConstant { value }),
        None,
        None,
    )
}

pub(crate) fn boolean_not_immediate_return_artifact(source_value: bool) -> (Vec<u8>, Vec<u8>) {
    let machine = MachineId::new(68_001).unwrap();
    let entry = BlockId::new(68_002).unwrap();
    let constant = ValueId::new(68_004).unwrap();
    let boolean_not = ValueId::new(68_006).unwrap();
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
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(ValueDeclaration {
                id: ValueId::new(68_008).unwrap(),
                scalar_type: ScalarType::Boolean,
            }),
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry,
            blocks: vec![Block {
                id: entry,
                parameters: Vec::new(),
                operations: vec![
                    Operation {
                        id: OperationId::new(68_003).unwrap(),
                        result: OperationResult::Scalar(ValueDeclaration {
                            id: constant,
                            scalar_type: ScalarType::Boolean,
                        }),
                        kind: OperationKind::BooleanConstant {
                            value: source_value,
                        },
                    },
                    Operation {
                        id: OperationId::new(68_005).unwrap(),
                        result: OperationResult::Scalar(ValueDeclaration {
                            id: boolean_not,
                            scalar_type: ScalarType::Boolean,
                        }),
                        kind: OperationKind::BooleanNot { operand: constant },
                    },
                ],
                terminator: Terminator::Return {
                    edge: EdgeId::new(68_007).unwrap(),
                    value: boolean_not,
                    cleanup_actions: Vec::new(),
                },
            }],
            contract: MachineContract {
                id: ContractId::new(68_009).unwrap(),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    (
        psi_terminal_codec::encode_module(&module).unwrap(),
        psi_terminal_codec::encode_proof_bundle(&ProofBundle::default()).unwrap(),
    )
}

pub(crate) fn integer_bitwise_not_immediate_return_artifact(
    scalar_type: IntegerType,
    source_value: IntegerValue,
) -> (Vec<u8>, Vec<u8>) {
    let machine = MachineId::new(67_001).unwrap();
    let entry = BlockId::new(67_002).unwrap();
    let constant = ValueId::new(67_004).unwrap();
    let bitwise_not = ValueId::new(67_006).unwrap();
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
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(ValueDeclaration {
                id: ValueId::new(67_008).unwrap(),
                scalar_type: ScalarType::Integer(scalar_type),
            }),
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry,
            blocks: vec![Block {
                id: entry,
                parameters: Vec::new(),
                operations: vec![
                    Operation {
                        id: OperationId::new(67_003).unwrap(),
                        result: OperationResult::Scalar(ValueDeclaration {
                            id: constant,
                            scalar_type: ScalarType::Integer(scalar_type),
                        }),
                        kind: OperationKind::IntegerConstant {
                            value: source_value,
                        },
                    },
                    Operation {
                        id: OperationId::new(67_005).unwrap(),
                        result: OperationResult::Scalar(ValueDeclaration {
                            id: bitwise_not,
                            scalar_type: ScalarType::Integer(scalar_type),
                        }),
                        kind: OperationKind::IntegerBitwiseNot { operand: constant },
                    },
                ],
                terminator: Terminator::Return {
                    edge: EdgeId::new(67_007).unwrap(),
                    value: bitwise_not,
                    cleanup_actions: Vec::new(),
                },
            }],
            contract: MachineContract {
                id: ContractId::new(67_009).unwrap(),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    (
        psi_terminal_codec::encode_module(&module).unwrap(),
        psi_terminal_codec::encode_proof_bundle(&ProofBundle::default()).unwrap(),
    )
}

pub(crate) fn integer_widen_immediate_return_artifact() -> (Vec<u8>, Vec<u8>) {
    let machine = MachineId::new(65_001).unwrap();
    let entry = BlockId::new(65_002).unwrap();
    let source_type = IntegerType::new(IntegerSign::Unsigned, 16).unwrap();
    let target_type = IntegerType::new(IntegerSign::Signed, 64).unwrap();
    let constant = ValueId::new(65_004).unwrap();
    let widened = ValueId::new(65_006).unwrap();
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
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(ValueDeclaration {
                id: ValueId::new(65_008).unwrap(),
                scalar_type: ScalarType::Integer(target_type),
            }),
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry,
            blocks: vec![Block {
                id: entry,
                parameters: Vec::new(),
                operations: vec![
                    Operation {
                        id: OperationId::new(65_003).unwrap(),
                        result: OperationResult::Scalar(ValueDeclaration {
                            id: constant,
                            scalar_type: ScalarType::Integer(source_type),
                        }),
                        kind: OperationKind::IntegerConstant {
                            value: IntegerValue::Unsigned(65_535),
                        },
                    },
                    Operation {
                        id: OperationId::new(65_005).unwrap(),
                        result: OperationResult::Scalar(ValueDeclaration {
                            id: widened,
                            scalar_type: ScalarType::Integer(target_type),
                        }),
                        kind: OperationKind::IntegerWiden { operand: constant },
                    },
                ],
                terminator: Terminator::Return {
                    edge: EdgeId::new(65_007).unwrap(),
                    value: widened,
                    cleanup_actions: Vec::new(),
                },
            }],
            contract: MachineContract {
                id: ContractId::new(65_009).unwrap(),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    (
        psi_terminal_codec::encode_module(&module).unwrap(),
        psi_terminal_codec::encode_proof_bundle(&ProofBundle::default()).unwrap(),
    )
}

pub(crate) fn integer_exact_cast_immediate_operand_return_artifact() -> (Vec<u8>, Vec<u8>) {
    let machine = MachineId::new(66_001).unwrap();
    let entry = BlockId::new(66_002).unwrap();
    let source_type = IntegerType::new(IntegerSign::Unsigned, 16).unwrap();
    let target_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let constant = ValueId::new(66_004).unwrap();
    let cast = ValueId::new(66_006).unwrap();
    let obligation = ObligationId::new(66_009).unwrap();
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
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(ValueDeclaration {
                id: ValueId::new(66_008).unwrap(),
                scalar_type: ScalarType::Integer(target_type),
            }),
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry,
            blocks: vec![Block {
                id: entry,
                parameters: Vec::new(),
                operations: vec![
                    Operation {
                        id: OperationId::new(66_003).unwrap(),
                        result: OperationResult::Scalar(ValueDeclaration {
                            id: constant,
                            scalar_type: ScalarType::Integer(source_type),
                        }),
                        kind: OperationKind::IntegerConstant {
                            value: IntegerValue::Unsigned(255),
                        },
                    },
                    Operation {
                        id: OperationId::new(66_005).unwrap(),
                        result: OperationResult::Scalar(ValueDeclaration {
                            id: cast,
                            scalar_type: ScalarType::Integer(target_type),
                        }),
                        kind: OperationKind::IntegerExactCast {
                            operand: constant,
                            obligation,
                        },
                    },
                ],
                terminator: Terminator::Return {
                    edge: EdgeId::new(66_007).unwrap(),
                    value: cast,
                    cleanup_actions: Vec::new(),
                },
            }],
            contract: MachineContract {
                id: ContractId::new(66_010).unwrap(),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    let reconstructed = reconstruct_operation_obligations(&module).unwrap();
    assert_eq!(reconstructed.len(), 1);
    let goal = reconstructed[0].obligation.proposition.clone();
    let psi_core::Proposition::IntegerMathLessOrEqual(_, upper_bound) = &goal else {
        panic!("u16-to-u8 constant exact cast must reconstruct one upper bound")
    };
    let closed_relation =
        psi_core::Proposition::IntegerMathLessOrEqual(upper_bound.clone(), upper_bound.clone());
    let constant_axiom =
        psi_proof_admission::lift_fixed_integer_relation(&reconstructed[0].semantic_axioms[0])
            .unwrap();
    let proof = ProofBundle {
        recursive_components: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(66_011).unwrap(),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: goal,
                    rule: ProofRule::IntegerLessOrEqualSubstitution {
                        relation: Box::new(ProofNode {
                            conclusion: closed_relation,
                            rule: ProofRule::Primitive(
                                psi_proof_admission::PrimitiveJudgment::ClosedIntegerRelation,
                            ),
                        }),
                        equality: Box::new(ProofNode {
                            conclusion: constant_axiom,
                            rule: ProofRule::SemanticAxiom { index: 0 },
                        }),
                        endpoint: 0,
                    },
                },
            }),
        }],
    };
    (
        psi_terminal_codec::encode_module(&module).unwrap(),
        psi_terminal_codec::encode_proof_bundle(&proof).unwrap(),
    )
}
