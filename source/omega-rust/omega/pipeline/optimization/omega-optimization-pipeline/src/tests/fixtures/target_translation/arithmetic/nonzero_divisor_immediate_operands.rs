//! Shared real Terminal fixture for proof-bearing nonzero-divisor operations over constants.

use super::super::*;

#[derive(Clone, Copy)]
pub(super) enum NonzeroDivisorIntegerOperation {
    WrappingDivide,
    WrappingRemainder,
    SaturatingDivide,
}

pub(super) fn nonzero_divisor_integer_immediate_operands_return_artifact(
    scalar_type: IntegerType,
    left: IntegerValue,
    right: IntegerValue,
    operation: NonzeroDivisorIntegerOperation,
) -> (Vec<u8>, Vec<u8>) {
    let machine = MachineId::new(84_001).unwrap();
    let entry = BlockId::new(84_002).unwrap();
    let left_result = ValueId::new(84_004).unwrap();
    let right_result = ValueId::new(84_006).unwrap();
    let operation_result = ValueId::new(84_008).unwrap();
    let obligation = ObligationId::new(84_011).unwrap();
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
            result: TerminalMachineResult::Scalar(ValueDeclaration {
                id: ValueId::new(84_010).unwrap(),
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
                        id: OperationId::new(84_003).unwrap(),
                        result: OperationResult::Scalar(ValueDeclaration {
                            id: left_result,
                            scalar_type: ScalarType::Integer(scalar_type),
                        }),
                        kind: OperationKind::IntegerConstant { value: left },
                    },
                    Operation {
                        id: OperationId::new(84_005).unwrap(),
                        result: OperationResult::Scalar(ValueDeclaration {
                            id: right_result,
                            scalar_type: ScalarType::Integer(scalar_type),
                        }),
                        kind: OperationKind::IntegerConstant { value: right },
                    },
                    Operation {
                        id: OperationId::new(84_007).unwrap(),
                        result: OperationResult::Scalar(ValueDeclaration {
                            id: operation_result,
                            scalar_type: ScalarType::Integer(scalar_type),
                        }),
                        kind: match operation {
                            NonzeroDivisorIntegerOperation::WrappingDivide => {
                                OperationKind::WrappingIntegerDivide {
                                    left: left_result,
                                    right: right_result,
                                    obligation,
                                }
                            }
                            NonzeroDivisorIntegerOperation::WrappingRemainder => {
                                OperationKind::WrappingIntegerRemainder {
                                    left: left_result,
                                    right: right_result,
                                    obligation,
                                }
                            }
                            NonzeroDivisorIntegerOperation::SaturatingDivide => {
                                OperationKind::SaturatingIntegerDivide {
                                    left: left_result,
                                    right: right_result,
                                    obligation,
                                }
                            }
                        },
                    },
                ],
                terminator: Terminator::Return {
                    edge: EdgeId::new(84_009).unwrap(),
                    value: operation_result,
                    cleanup_actions: Vec::new(),
                },
            }],
            contract: MachineContract {
                id: ContractId::new(84_012).unwrap(),
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
    let one = psi_core::ScalarTerm::integer(
        scalar_type,
        match scalar_type.sign() {
            IntegerSign::Signed => IntegerValue::Signed(1),
            IntegerSign::Unsigned => IntegerValue::Unsigned(1),
        },
    )
    .unwrap();
    let literal = psi_core::ScalarTerm::integer(scalar_type, right).unwrap();
    let (selected, closed_relation, endpoint, disjunction_index) = match scalar_type.sign() {
        IntegerSign::Signed if matches!(right, IntegerValue::Signed(value) if value < 0) => {
            let negative_one =
                psi_core::ScalarTerm::integer(scalar_type, IntegerValue::Signed(-1)).unwrap();
            let psi_core::Proposition::Disjunction(disjuncts) = &goal else {
                panic!("signed nonzero-divisor goal must be a disjunction")
            };
            (
                disjuncts[0].clone(),
                psi_core::Proposition::LessOrEqual(literal, negative_one),
                0,
                Some(0),
            )
        }
        IntegerSign::Signed => {
            let psi_core::Proposition::Disjunction(disjuncts) = &goal else {
                panic!("signed nonzero-divisor goal must be a disjunction")
            };
            (
                disjuncts[1].clone(),
                psi_core::Proposition::LessOrEqual(one, literal),
                1,
                Some(1),
            )
        }
        IntegerSign::Unsigned => (
            goal.clone(),
            psi_core::Proposition::LessOrEqual(one, literal),
            1,
            None,
        ),
    };
    let selected_proof = ProofNode {
        conclusion: selected.clone(),
        rule: ProofRule::IntegerLessOrEqualSubstitution {
            relation: Box::new(ProofNode {
                conclusion: closed_relation,
                rule: ProofRule::Primitive(
                    psi_proof_admission::PrimitiveJudgment::ClosedIntegerRelation,
                ),
            }),
            equality: Box::new(ProofNode {
                conclusion: reconstructed[0].semantic_axioms[1].clone(),
                rule: ProofRule::SemanticAxiom { index: 1 },
            }),
            endpoint,
        },
    };
    let proof_node = match disjunction_index {
        Some(index) => ProofNode {
            conclusion: goal,
            rule: ProofRule::DisjunctionIntroduction {
                disjunct: Box::new(selected_proof),
                index,
            },
        },
        None => selected_proof,
    };
    let proof = ProofBundle {
        recursive_components: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(84_013).unwrap(),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: proof_node,
            }),
        }],
    };
    (
        psi_terminal_codec::encode_module(&module).unwrap(),
        psi_terminal_codec::encode_proof_bundle(&proof).unwrap(),
    )
}
