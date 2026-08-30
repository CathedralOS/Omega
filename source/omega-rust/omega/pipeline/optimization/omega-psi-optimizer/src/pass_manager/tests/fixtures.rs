//! Verified optimizer inputs shared by execution and replay tests.

use super::*;

fn exact_unsigned_add_certificate(
    integer: psi_core::IntegerType,
    left: psi_core::ValueId,
    right: psi_core::ValueId,
    left_value: psi_core::IntegerValue,
    right_value: psi_core::IntegerValue,
    left_axiom: usize,
    right_axiom: usize,
    identity: u64,
) -> psi_proof_admission::EvidenceRoute {
    use psi_core::{EvidenceIdentity, IntegerMathTerm, Proposition, ScalarTerm, ScalarType};
    use psi_proof_admission::{
        CertificateEnvelope, IntegerAffineWitness, PrimitiveJudgment, ProofNode, ProofRule,
        ProofSystemMarker,
    };

    let scalar_type = ScalarType::Integer(integer);
    let left_id = left;
    let right_id = right;
    let left = ScalarTerm::value(left, scalar_type);
    let right = ScalarTerm::value(right, scalar_type);
    let left_literal = ScalarTerm::integer(integer, left_value).unwrap();
    let right_literal = ScalarTerm::integer(integer, right_value).unwrap();
    let target = ScalarTerm::exact_integer_add(integer, left.clone(), right.clone()).unwrap();
    let sum = IntegerMathTerm::Add(
        Box::new(IntegerMathTerm::MathValue {
            source_type: integer,
            value: left_id,
        }),
        Box::new(IntegerMathTerm::MathValue {
            source_type: integer,
            value: right_id,
        }),
    );
    let exact_sum = integer.exact_add(left_value, right_value).unwrap();
    let tight = IntegerMathTerm::literal(exact_sum);
    let goal = Proposition::IntegerMathLessOrEqual(
        sum.clone(),
        IntegerMathTerm::literal(integer.maximum_value()),
    );
    let left_equality = Proposition::Equal(left.clone(), left_literal);
    let right_equality = Proposition::Equal(right.clone(), right_literal.clone());
    let right_bound = Proposition::LessOrEqual(right, right_literal.clone());
    let tight_bound = ProofNode {
        conclusion: Proposition::IntegerMathLessOrEqual(sum, tight.clone()),
        rule: ProofRule::IntegerAffineBound {
            root_bound: Box::new(ProofNode {
                conclusion: Proposition::Conjunction(vec![
                    left_equality.clone(),
                    right_bound.clone(),
                ]),
                rule: ProofRule::ConjunctionIntroduction(vec![
                    ProofNode {
                        conclusion: left_equality,
                        rule: ProofRule::SemanticAxiom { index: left_axiom },
                    },
                    ProofNode {
                        conclusion: right_bound,
                        rule: ProofRule::IntegerLessOrEqualSubstitution {
                            relation: Box::new(ProofNode {
                                conclusion: Proposition::LessOrEqual(
                                    right_literal.clone(),
                                    right_literal,
                                ),
                                rule: ProofRule::Primitive(
                                    PrimitiveJudgment::ClosedIntegerRelation,
                                ),
                            }),
                            equality: Box::new(ProofNode {
                                conclusion: right_equality,
                                rule: ProofRule::SemanticAxiom { index: right_axiom },
                            }),
                            endpoint: 0,
                        },
                    },
                ]),
            }),
            witness: IntegerAffineWitness {
                root: left,
                target,
                definition_axioms: Vec::new(),
                literal_axioms: Vec::new(),
            },
        },
    };
    psi_proof_admission::EvidenceRoute::CertificateDerived(CertificateEnvelope {
        identity: EvidenceIdentity::new(identity).unwrap(),
        proof_system_marker: ProofSystemMarker::CURRENT,
        proof: ProofNode {
            conclusion: goal,
            rule: ProofRule::IntegerLessOrEqualTransitivity {
                left_less_or_equal_middle: Box::new(tight_bound),
                middle_less_or_equal_right: Box::new(ProofNode {
                    conclusion: Proposition::IntegerMathLessOrEqual(
                        tight,
                        IntegerMathTerm::literal(integer.maximum_value()),
                    ),
                    rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
                }),
            },
        },
    })
}

fn exact_unsigned_shift_count_certificate(
    value_type: psi_core::IntegerType,
    count_type: psi_core::IntegerType,
    count: psi_core::ValueId,
    count_axiom: usize,
    identity: u64,
) -> psi_proof_admission::EvidenceRoute {
    use psi_core::{EvidenceIdentity, IntegerValue, Proposition, ScalarTerm, ScalarType};
    use psi_proof_admission::{
        CertificateEnvelope, PrimitiveJudgment, ProofNode, ProofRule, ProofSystemMarker,
    };

    let count = ScalarTerm::value(count, ScalarType::Integer(count_type));
    let zero = ScalarTerm::integer(count_type, IntegerValue::Unsigned(0)).unwrap();
    let maximum = ScalarTerm::integer(
        count_type,
        IntegerValue::Unsigned(u128::from(value_type.bits() - 1)),
    )
    .unwrap();
    let goal = Proposition::LessOrEqual(count.clone(), maximum.clone());
    psi_proof_admission::EvidenceRoute::CertificateDerived(CertificateEnvelope {
        identity: EvidenceIdentity::new(identity).unwrap(),
        proof_system_marker: ProofSystemMarker::CURRENT,
        proof: ProofNode {
            conclusion: goal,
            rule: ProofRule::IntegerLessOrEqualSubstitution {
                relation: Box::new(ProofNode {
                    conclusion: Proposition::LessOrEqual(zero.clone(), maximum),
                    rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
                }),
                equality: Box::new(ProofNode {
                    conclusion: Proposition::Equal(count, zero),
                    rule: ProofRule::SemanticAxiom { index: count_axiom },
                }),
                endpoint: 0,
            },
        },
    })
}

fn remainder_by_one_certificate(
    integer: psi_core::IntegerType,
    divisor: psi_core::ValueId,
) -> psi_proof_admission::EvidenceRoute {
    use psi_core::{EvidenceIdentity, IntegerValue, Proposition, ScalarTerm, ScalarType};
    use psi_proof_admission::{
        CertificateEnvelope, PrimitiveJudgment, ProofNode, ProofRule, ProofSystemMarker,
    };

    let scalar_type = ScalarType::Integer(integer);
    let literal_one = ScalarTerm::integer(integer, IntegerValue::Unsigned(1)).unwrap();
    let divisor_term = ScalarTerm::value(divisor, scalar_type);
    psi_proof_admission::EvidenceRoute::CertificateDerived(CertificateEnvelope {
        identity: EvidenceIdentity::new(462).unwrap(),
        proof_system_marker: ProofSystemMarker::CURRENT,
        proof: ProofNode {
            conclusion: Proposition::LessOrEqual(literal_one.clone(), divisor_term.clone()),
            rule: ProofRule::IntegerLessOrEqualSubstitution {
                relation: Box::new(ProofNode {
                    conclusion: Proposition::LessOrEqual(literal_one.clone(), literal_one.clone()),
                    rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
                }),
                equality: Box::new(ProofNode {
                    conclusion: Proposition::Equal(divisor_term, literal_one),
                    rule: ProofRule::SemanticAxiom { index: 0 },
                }),
                endpoint: 1,
            },
        },
    })
}

fn signed_remainder_by_negative_one_certificate(
    integer: psi_core::IntegerType,
    dividend: psi_core::ValueId,
    divisor: psi_core::ValueId,
) -> psi_proof_admission::EvidenceRoute {
    use psi_core::{EvidenceIdentity, IntegerValue, Proposition, ScalarTerm, ScalarType};
    use psi_proof_admission::{
        CertificateEnvelope, PrimitiveJudgment, ProofNode, ProofRule, ProofSystemMarker,
    };

    let scalar_type = ScalarType::Integer(integer);
    let literal = |value| ScalarTerm::integer(integer, IntegerValue::Signed(value)).unwrap();
    let dividend_term = ScalarTerm::value(dividend, scalar_type);
    let divisor_term = ScalarTerm::value(divisor, scalar_type);
    let minimum_plus_one = match integer.minimum_value() {
        IntegerValue::Signed(minimum) => minimum.checked_add(1).unwrap(),
        IntegerValue::Unsigned(_) => unreachable!("negative-one fixture requires a signed type"),
    };
    let negative_case = Proposition::LessOrEqual(divisor_term.clone(), literal(-1));
    let dividend_case = Proposition::LessOrEqual(literal(minimum_plus_one), dividend_term.clone());
    let defined_case = Proposition::Conjunction(vec![negative_case.clone(), dividend_case.clone()]);
    let goal = Proposition::Disjunction(vec![
        Proposition::LessOrEqual(divisor_term.clone(), literal(-2)),
        Proposition::LessOrEqual(literal(1), divisor_term.clone()),
        defined_case.clone(),
    ]);
    let prove_bound = |conclusion: Proposition,
                       relation: Proposition,
                       equality: Proposition,
                       endpoint: usize,
                       axiom: usize| ProofNode {
        conclusion,
        rule: ProofRule::IntegerLessOrEqualSubstitution {
            relation: Box::new(ProofNode {
                conclusion: relation,
                rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
            }),
            equality: Box::new(ProofNode {
                conclusion: equality,
                rule: ProofRule::SemanticAxiom { index: axiom },
            }),
            endpoint,
        },
    };
    psi_proof_admission::EvidenceRoute::CertificateDerived(CertificateEnvelope {
        identity: EvidenceIdentity::new(483).unwrap(),
        proof_system_marker: ProofSystemMarker::CURRENT,
        proof: ProofNode {
            conclusion: goal,
            rule: ProofRule::DisjunctionIntroduction {
                disjunct: Box::new(ProofNode {
                    conclusion: defined_case,
                    rule: ProofRule::ConjunctionIntroduction(vec![
                        prove_bound(
                            negative_case,
                            Proposition::LessOrEqual(literal(-1), literal(-1)),
                            Proposition::Equal(divisor_term, literal(-1)),
                            0,
                            1,
                        ),
                        prove_bound(
                            dividend_case,
                            Proposition::LessOrEqual(literal(minimum_plus_one), literal(7)),
                            Proposition::Equal(dividend_term, literal(7)),
                            1,
                            0,
                        ),
                    ]),
                }),
                index: 2,
            },
        },
    })
}

pub(super) fn verified_empty_unit() -> VerifiedPsiOptimizationUnit {
    use psi_core::{BlockId, ContractId, EdgeId, MachineId};
    use psi_terminal::{
        Block, MachineContract, TerminalMachine, TerminalMachineResult, TerminalModule, Terminator,
        VocabularyMarker,
    };

    let module = TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: MachineId::new(401).unwrap(),
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
            id: MachineId::new(401).unwrap(),
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
            entry: BlockId::new(402).unwrap(),
            blocks: vec![Block {
                id: BlockId::new(402).unwrap(),
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: Terminator::ReturnUnit {
                    edge: EdgeId::new(403).unwrap(),
                    trivial_affine_discards: Vec::new(),
                },
            }],
            contract: MachineContract {
                id: ContractId::new(404).unwrap(),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    let semantic = psi_terminal_codec::encode_module(&module).unwrap();
    let proof =
        psi_terminal_codec::encode_proof_bundle(&psi_terminal_verifier::ProofBundle::default())
            .unwrap();
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

pub(super) fn verified_exact_add_unit() -> VerifiedPsiOptimizationUnit {
    verified_exact_add_unit_with_right(psi_core::IntegerValue::Unsigned(8))
}

pub(super) fn verified_exact_add_zero_unit() -> VerifiedPsiOptimizationUnit {
    verified_exact_add_unit_with_right(psi_core::IntegerValue::Unsigned(0))
}

pub(super) fn verified_compatible_policy_cse_unit() -> VerifiedPsiOptimizationUnit {
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

pub(super) fn verified_compatible_policy_phi_gvn_unit() -> VerifiedPsiOptimizationUnit {
    use psi_core::{
        BlockId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId,
        ObligationId, OperationId, ScalarType, ValueId,
    };
    use psi_terminal::{
        Block, MachineContract, Operation, OperationKind, OperationResult, SuccessorEdge,
        TerminalMachine, TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration,
        VocabularyMarker,
    };
    use psi_terminal_verifier::{ObligationEvidence, ProofBundle};

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
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation,
            route: exact_unsigned_shift_count_certificate(
                value_type,
                count_type,
                join_b,
                2,
                517,
            ),
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

pub(super) fn verified_exact_add_unit_with_right(
    right_constant: psi_core::IntegerValue,
) -> VerifiedPsiOptimizationUnit {
    use psi_core::{
        BlockId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId,
        ObligationId, OperationId, ScalarType, ValueId,
    };
    use psi_terminal::{
        Block, MachineContract, Operation, OperationKind, OperationResult, TerminalMachine,
        TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration, VocabularyMarker,
    };
    use psi_terminal_verifier::{ObligationEvidence, ProofBundle};

    let machine = MachineId::new(411).unwrap();
    let block = BlockId::new(412).unwrap();
    let left = ValueId::new(413).unwrap();
    let right = ValueId::new(414).unwrap();
    let computed = ValueId::new(415).unwrap();
    let result = ValueId::new(422).unwrap();
    let obligation = ObligationId::new(419).unwrap();
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
                        id: OperationId::new(416).unwrap(),
                        result: OperationResult::Scalar(declaration(left)),
                        kind: OperationKind::IntegerConstant {
                            value: IntegerValue::Unsigned(7),
                        },
                    },
                    Operation {
                        id: OperationId::new(417).unwrap(),
                        result: OperationResult::Scalar(declaration(right)),
                        kind: OperationKind::IntegerConstant {
                            value: right_constant,
                        },
                    },
                    Operation {
                        id: OperationId::new(418).unwrap(),
                        result: OperationResult::Scalar(declaration(computed)),
                        kind: OperationKind::ExactIntegerAdd {
                            left,
                            right,
                            obligation,
                        },
                    },
                ],
                terminator: Terminator::Return {
                    cleanup_actions: Vec::new(),
                    edge: EdgeId::new(420).unwrap(),
                    value: computed,
                },
            }],
            contract: MachineContract {
                id: ContractId::new(421).unwrap(),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    let proof = ProofBundle {
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation,
            route: exact_unsigned_add_certificate(
                integer,
                left,
                right,
                IntegerValue::Unsigned(7),
                right_constant,
                0,
                1,
                419,
            ),
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

pub(super) fn verified_exact_remainder_by_one_unit() -> VerifiedPsiOptimizationUnit {
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

pub(super) fn verified_exact_signed_remainder_by_negative_one_unit() -> VerifiedPsiOptimizationUnit
{
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

pub(super) fn verified_exact_self_remainder_unit() -> VerifiedPsiOptimizationUnit {
    verified_exact_self_division_or_remainder_unit(false)
}

pub(super) fn verified_exact_self_divide_unit() -> VerifiedPsiOptimizationUnit {
    verified_exact_self_division_or_remainder_unit(true)
}
