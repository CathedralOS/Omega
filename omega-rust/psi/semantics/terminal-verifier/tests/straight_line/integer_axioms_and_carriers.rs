use proof_admission::{
    AdmissionProfile, CertificateEnvelope, EvidenceRoute, ProofNode, ProofRule, ProofSystemMarker,
};
use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, EvidenceIdentity, IntegerMathTerm, IntegerSign, IntegerType,
    IntegerValue, MachineId, ObligationId, OperationId, Proposition, ScalarTerm, ScalarType,
    ValueId,
};
use terminal_psi::{
    Block, ContractClause, MachineContract, Operation, OperationKind, TerminalMachine,
    TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration, VocabularyMarker,
};
use terminal_verifier::{
    ModuleError, ObligationEvidence, ProofBundle, reconstruct_operation_obligations,
    reconstruct_terminal_obligations, validate_module, verify_module,
};

#[test]
fn integer_equality_axiom_proves_the_return_contract() {
    let integer =
        IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 integer-equality operand type");
    let integer_scalar = ScalarType::Integer(integer);
    let left = ValueId::new(40).expect("left parameter");
    let right = ValueId::new(41).expect("right parameter");
    let compared = ValueId::new(42).expect("compared");
    let result = ValueId::new(43).expect("result");
    let obligation = ObligationId::new(40).expect("obligation");
    let value = |id, scalar_type| ScalarTerm::value(id, scalar_type);
    let equality = ScalarTerm::integer_equal(
        integer,
        value(left, integer_scalar),
        value(right, integer_scalar),
    )
    .expect("matching integer operands form equality");
    let goal = Proposition::Equal(value(result, ScalarType::Boolean), equality.clone());
    let module = TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: MachineId::new(40).expect("machine"),
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
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            closed_reach_application: None,
            declared_service_reach: Vec::new(),
            id: MachineId::new(40).expect("machine"),
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: vec![
                ValueDeclaration {
                    qualifications: Default::default(),
                    id: left,
                    scalar_type: integer_scalar,
                },
                ValueDeclaration {
                    qualifications: Default::default(),
                    id: right,
                    scalar_type: integer_scalar,
                },
            ],
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: result,
                scalar_type: ScalarType::Boolean,
            }),
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: BlockId::new(40).expect("block"),
            blocks: vec![Block {
                erased_scalar_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: BlockId::new(40).expect("block"),
                parameters: Vec::new(),
                operations: vec![Operation {
                    static_reach_binding: None,
                    id: OperationId::new(40).expect("operation"),
                    result: terminal_psi::OperationResult::Scalar(ValueDeclaration {
                        qualifications: Default::default(),
                        id: compared,
                        scalar_type: ScalarType::Boolean,
                    }),
                    kind: OperationKind::IntegerEqual { left, right },
                }],
                terminator: Terminator::Return {
                    cleanup_actions: Vec::new(),
                    edge: EdgeId::new(40).expect("edge"),
                    value: compared,
                },
            }],
            contract: MachineContract {
                erased_scalar_formals: Vec::new(),
                id: ContractId::new(40).expect("contract"),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: vec![ContractClause {
                    obligation,
                    proposition: goal.clone(),
                }],
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    let bundle = ProofBundle {
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(40).expect("certificate"),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: goal,
                    rule: ProofRule::EqualityTransitivity {
                        left_equals_middle: Box::new(ProofNode {
                            conclusion: Proposition::Equal(
                                value(result, ScalarType::Boolean),
                                value(compared, ScalarType::Boolean),
                            ),
                            rule: ProofRule::SemanticAxiom { index: 3 },
                        }),
                        middle_equals_right: Box::new(ProofNode {
                            conclusion: Proposition::Equal(
                                value(compared, ScalarType::Boolean),
                                equality,
                            ),
                            rule: ProofRule::SemanticAxiom { index: 0 },
                        }),
                    },
                },
            }),
        }],
    };

    verify_module(&module, &bundle, &AdmissionProfile::default())
        .expect("integer-equality semantics should reconstruct operation and return axioms");
    let reconstructed = reconstruct_terminal_obligations(&module).unwrap();
    let site = reconstructed
        .obligations()
        .iter()
        .find(|site| site.obligation.id == obligation)
        .unwrap();
    assert_eq!(site.semantic_axioms.len(), 4);
    assert!(matches!(
        &site.semantic_axioms[0],
        Proposition::Equal(_, ScalarTerm::IntegerEqual { .. })
    ));
    assert!(matches!(
        &site.semantic_axioms[1],
        Proposition::Implication { .. }
    ));
    assert!(matches!(
        &site.semantic_axioms[2],
        Proposition::Implication { .. }
    ));
    assert_eq!(
        site.semantic_axioms[3],
        Proposition::Equal(
            value(result, ScalarType::Boolean),
            value(compared, ScalarType::Boolean)
        )
    );

    let signed_integer =
        IntegerType::new(IntegerSign::Signed, 8).expect("i8 mismatched operand type");
    let mut wrong_operand = module.clone();
    wrong_operand.machines[0].contract.ensures.clear();
    wrong_operand.machines[0].parameters[1].scalar_type = ScalarType::Integer(signed_integer);
    assert_eq!(
        validate_module(&wrong_operand)
            .expect_err("integer equality requires two operands of one exact integer type"),
        ModuleError::IntegerEqualOperandTypeMismatch {
            operation: OperationId::new(40).expect("operation"),
            left: integer_scalar,
            right: ScalarType::Integer(signed_integer),
        }
    );

    let mut wrong_result = module;
    wrong_result.machines[0].contract.ensures.clear();
    wrong_result.machines[0].blocks[0].operations[0]
        .result
        .scalar_mut()
        .unwrap()
        .scalar_type = integer_scalar;
    assert_eq!(
        validate_module(&wrong_result).expect_err("integer equality requires a Boolean result"),
        ModuleError::IntegerEqualRequiresBooleanResult(OperationId::new(40).expect("operation"))
    );
}

#[test]
fn integer_ordering_axioms_prove_return_contracts() {
    let integer = IntegerType::new(IntegerSign::Signed, 8).expect("i8 ordering type");
    let integer_scalar = ScalarType::Integer(integer);
    let left = ValueId::new(50).expect("left parameter");
    let right = ValueId::new(51).expect("right parameter");
    let compared = ValueId::new(52).expect("compared");
    let result = ValueId::new(53).expect("result");
    let obligation = ObligationId::new(50).expect("obligation");
    let value = |id, scalar_type| ScalarTerm::value(id, scalar_type);

    for inclusive in [false, true] {
        let ordered = if inclusive {
            ScalarTerm::integer_less_or_equal(
                integer,
                value(left, integer_scalar),
                value(right, integer_scalar),
            )
            .expect("matching operands form less-or-equal")
        } else {
            ScalarTerm::integer_less_than(
                integer,
                value(left, integer_scalar),
                value(right, integer_scalar),
            )
            .expect("matching operands form less-than")
        };
        let goal = Proposition::Equal(value(result, ScalarType::Boolean), ordered.clone());
        let operation = if inclusive {
            OperationKind::IntegerLessOrEqual { left, right }
        } else {
            OperationKind::IntegerLessThan { left, right }
        };
        let module = TerminalModule {
            scalar_qualifications: Default::default(),
            scalar_block_invariants: Vec::new(),
            operation_crash_contracts: Vec::new(),
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry: MachineId::new(50).expect("machine"),
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
            evidence_contract_lanes: Vec::new(),
            proof_output_calls: Vec::new(),
            proof_recursive_components: Vec::new(),
            closed_conformance_applications: Vec::new(),
            dynamic_dispatch: Default::default(),
            suspension_call_plan_count: 0,
            suspension_call_sites: Vec::new(),
            suspension_call_plans: Vec::new(),
            quotient_correspondences: Vec::new(),
            machines: vec![TerminalMachine {
                closed_reach_application: None,
                declared_service_reach: Vec::new(),
                id: MachineId::new(50).expect("machine"),
                attachment: None,
                structural_parameters: Vec::new(),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                parameters: vec![
                    ValueDeclaration {
                        qualifications: Default::default(),
                        id: left,
                        scalar_type: integer_scalar,
                    },
                    ValueDeclaration {
                        qualifications: Default::default(),
                        id: right,
                        scalar_type: integer_scalar,
                    },
                ],
                ranked_scc: None,
                result: TerminalMachineResult::Scalar(ValueDeclaration {
                    qualifications: Default::default(),
                    id: result,
                    scalar_type: ScalarType::Boolean,
                }),
                structural_places: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: BlockId::new(50).expect("block"),
                blocks: vec![Block {
                    erased_scalar_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: BlockId::new(50).expect("block"),
                    parameters: Vec::new(),
                    operations: vec![Operation {
                        static_reach_binding: None,
                        id: OperationId::new(50).expect("operation"),
                        result: terminal_psi::OperationResult::Scalar(ValueDeclaration {
                            qualifications: Default::default(),
                            id: compared,
                            scalar_type: ScalarType::Boolean,
                        }),
                        kind: operation,
                    }],
                    terminator: Terminator::Return {
                        cleanup_actions: Vec::new(),
                        edge: EdgeId::new(50).expect("edge"),
                        value: compared,
                    },
                }],
                contract: MachineContract {
                    erased_scalar_formals: Vec::new(),
                    id: ContractId::new(50).expect("contract"),
                    crash_routes: Vec::new(),
                    requires: Vec::new(),
                    ensures: vec![ContractClause {
                        obligation,
                        proposition: goal.clone(),
                    }],
                    outcome_specific_ensures: Vec::new(),
                },
            }],
        };
        let bundle = ProofBundle {
            recursive_components: Vec::new(),
            control_cycles: Vec::new(),
            evidence_producers: Vec::new(),
            evidence: vec![ObligationEvidence {
                obligation,
                route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                    identity: EvidenceIdentity::new(50).expect("certificate"),
                    proof_system_marker: ProofSystemMarker::CURRENT,
                    proof: ProofNode {
                        conclusion: goal,
                        rule: ProofRule::EqualityTransitivity {
                            left_equals_middle: Box::new(ProofNode {
                                conclusion: Proposition::Equal(
                                    value(result, ScalarType::Boolean),
                                    value(compared, ScalarType::Boolean),
                                ),
                                rule: ProofRule::SemanticAxiom { index: 3 },
                            }),
                            middle_equals_right: Box::new(ProofNode {
                                conclusion: Proposition::Equal(
                                    value(compared, ScalarType::Boolean),
                                    ordered,
                                ),
                                rule: ProofRule::SemanticAxiom { index: 0 },
                            }),
                        },
                    },
                }),
            }],
        };
        verify_module(&module, &bundle, &AdmissionProfile::default())
            .expect("ordering reconstructs the exact operation and return axioms");
        let reconstructed = reconstruct_terminal_obligations(&module).unwrap();
        let site = reconstructed
            .obligations()
            .iter()
            .find(|site| site.obligation.id == obligation)
            .unwrap();
        assert_eq!(site.semantic_axioms.len(), 4);
        assert!(matches!(
            &site.semantic_axioms[0],
            Proposition::Equal(
                _,
                ScalarTerm::IntegerLessThan { .. } | ScalarTerm::IntegerLessOrEqual { .. }
            )
        ));
        assert!(matches!(
            &site.semantic_axioms[1],
            Proposition::Implication { .. }
        ));
        assert!(matches!(
            &site.semantic_axioms[2],
            Proposition::Implication { .. }
        ));
        assert_eq!(
            site.semantic_axioms[3],
            Proposition::Equal(
                value(result, ScalarType::Boolean),
                value(compared, ScalarType::Boolean)
            )
        );

        let mut wrong_operand = module.clone();
        wrong_operand.machines[0].contract.ensures.clear();
        wrong_operand.machines[0].parameters[1].scalar_type =
            ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 mismatch"));
        assert!(matches!(
            validate_module(&wrong_operand),
            Err(ModuleError::IntegerOrderingOperandTypeMismatch { .. })
        ));

        let mut wrong_result = module.clone();
        wrong_result.machines[0].contract.ensures.clear();
        wrong_result.machines[0].blocks[0].operations[0]
            .result
            .scalar_mut()
            .unwrap()
            .scalar_type = integer_scalar;
        assert_eq!(
            validate_module(&wrong_result).expect_err("integer ordering requires a Boolean result"),
            ModuleError::IntegerOrderingRequiresBooleanResult(
                OperationId::new(50).expect("operation")
            )
        );
    }
}

#[test]
fn integer_bitwise_axioms_prove_exact_result_contracts() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 bitwise type");
    let scalar_type = ScalarType::Integer(integer);
    let left = ValueId::new(60).expect("left");
    let right = ValueId::new(61).expect("right");
    let computed = ValueId::new(62).expect("computed");
    let result = ValueId::new(63).expect("result");
    let value = |id| ScalarTerm::value(id, scalar_type);

    for kind in 0_u8..3 {
        let operation = match kind {
            0 => OperationKind::IntegerBitwiseAnd { left, right },
            1 => OperationKind::IntegerBitwiseOr { left, right },
            2 => OperationKind::IntegerBitwiseXor { left, right },
            _ => unreachable!(),
        };
        let term = match kind {
            0 => ScalarTerm::integer_bitwise_and(integer, value(left), value(right)),
            1 => ScalarTerm::integer_bitwise_or(integer, value(left), value(right)),
            2 => ScalarTerm::integer_bitwise_xor(integer, value(left), value(right)),
            _ => unreachable!(),
        }
        .expect("matching operands form a bitwise term");
        let goal = Proposition::Equal(value(result), term.clone());
        let obligation = ObligationId::new(60 + u64::from(kind)).expect("obligation");
        let module = TerminalModule {
            scalar_qualifications: Default::default(),
            scalar_block_invariants: Vec::new(),
            operation_crash_contracts: Vec::new(),
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry: MachineId::new(60).expect("machine"),
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
            evidence_contract_lanes: Vec::new(),
            proof_output_calls: Vec::new(),
            proof_recursive_components: Vec::new(),
            closed_conformance_applications: Vec::new(),
            dynamic_dispatch: Default::default(),
            suspension_call_plan_count: 0,
            suspension_call_sites: Vec::new(),
            suspension_call_plans: Vec::new(),
            quotient_correspondences: Vec::new(),
            machines: vec![TerminalMachine {
                closed_reach_application: None,
                declared_service_reach: Vec::new(),
                id: MachineId::new(60).expect("machine"),
                attachment: None,
                structural_parameters: Vec::new(),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                parameters: vec![
                    ValueDeclaration {
                        qualifications: Default::default(),
                        id: left,
                        scalar_type,
                    },
                    ValueDeclaration {
                        qualifications: Default::default(),
                        id: right,
                        scalar_type,
                    },
                ],
                ranked_scc: None,
                result: TerminalMachineResult::Scalar(ValueDeclaration {
                    qualifications: Default::default(),
                    id: result,
                    scalar_type,
                }),
                structural_places: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: BlockId::new(60).expect("block"),
                blocks: vec![Block {
                    erased_scalar_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: BlockId::new(60).expect("block"),
                    parameters: Vec::new(),
                    operations: vec![Operation {
                        static_reach_binding: None,
                        id: OperationId::new(60).expect("operation"),
                        result: terminal_psi::OperationResult::Scalar(ValueDeclaration {
                            qualifications: Default::default(),
                            id: computed,
                            scalar_type,
                        }),
                        kind: operation,
                    }],
                    terminator: Terminator::Return {
                        cleanup_actions: Vec::new(),
                        edge: EdgeId::new(60).expect("edge"),
                        value: computed,
                    },
                }],
                contract: MachineContract {
                    erased_scalar_formals: Vec::new(),
                    id: ContractId::new(60).expect("contract"),
                    crash_routes: Vec::new(),
                    requires: Vec::new(),
                    ensures: vec![ContractClause {
                        obligation,
                        proposition: goal.clone(),
                    }],
                    outcome_specific_ensures: Vec::new(),
                },
            }],
        };
        let bundle = ProofBundle {
            recursive_components: Vec::new(),
            control_cycles: Vec::new(),
            evidence_producers: Vec::new(),
            evidence: vec![ObligationEvidence {
                obligation,
                route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                    identity: EvidenceIdentity::new(60 + u64::from(kind)).expect("certificate"),
                    proof_system_marker: ProofSystemMarker::CURRENT,
                    proof: ProofNode {
                        conclusion: goal,
                        rule: ProofRule::EqualityTransitivity {
                            left_equals_middle: Box::new(ProofNode {
                                conclusion: Proposition::Equal(value(result), value(computed)),
                                rule: ProofRule::SemanticAxiom { index: 1 },
                            }),
                            middle_equals_right: Box::new(ProofNode {
                                conclusion: Proposition::Equal(value(computed), term),
                                rule: ProofRule::SemanticAxiom { index: 0 },
                            }),
                        },
                    },
                }),
            }],
        };
        verify_module(&module, &bundle, &AdmissionProfile::default())
            .expect("reconstructs the exact bitwise result axiom");

        let mut wrong_operand = module.clone();
        wrong_operand.machines[0].contract.ensures.clear();
        wrong_operand.machines[0].parameters[1].scalar_type =
            ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 8).expect("i8 mismatch"));
        assert!(matches!(
            validate_module(&wrong_operand),
            Err(ModuleError::IntegerBitwiseOperandTypeMismatch { .. })
        ));

        let mut wrong_result = module.clone();
        wrong_result.machines[0].contract.ensures.clear();
        wrong_result.machines[0].blocks[0].operations[0]
            .result
            .scalar_mut()
            .unwrap()
            .scalar_type = ScalarType::Boolean;
        assert_eq!(
            validate_module(&wrong_result).expect_err("integer bitwise requires an integer result"),
            ModuleError::IntegerBitwiseRequiresIntegerResult(
                OperationId::new(60).expect("operation")
            )
        );
    }
}

#[test]
fn integer_bitwise_not_reconstructs_its_exact_result_axiom() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 type");
    let scalar_type = ScalarType::Integer(integer);
    let operand = ValueId::new(65).expect("operand");
    let computed = ValueId::new(66).expect("computed");
    let result = ValueId::new(67).expect("result");
    let value = |id| ScalarTerm::value(id, scalar_type);
    let term = ScalarTerm::integer_bitwise_not(integer, value(operand)).unwrap();
    let goal = Proposition::Equal(value(result), term.clone());
    let obligation = ObligationId::new(65).expect("obligation");
    let module = TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: MachineId::new(65).expect("machine"),
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
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            closed_reach_application: None,
            declared_service_reach: Vec::new(),
            id: MachineId::new(65).expect("machine"),
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: vec![ValueDeclaration {
                qualifications: Default::default(),
                id: operand,
                scalar_type,
            }],
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: result,
                scalar_type,
            }),
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: BlockId::new(65).expect("block"),
            blocks: vec![Block {
                erased_scalar_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: BlockId::new(65).expect("block"),
                parameters: Vec::new(),
                operations: vec![Operation {
                    static_reach_binding: None,
                    id: OperationId::new(65).expect("operation"),
                    result: terminal_psi::OperationResult::Scalar(ValueDeclaration {
                        qualifications: Default::default(),
                        id: computed,
                        scalar_type,
                    }),
                    kind: OperationKind::IntegerBitwiseNot { operand },
                }],
                terminator: Terminator::Return {
                    cleanup_actions: Vec::new(),
                    edge: EdgeId::new(65).expect("edge"),
                    value: computed,
                },
            }],
            contract: MachineContract {
                erased_scalar_formals: Vec::new(),
                id: ContractId::new(65).expect("contract"),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: vec![ContractClause {
                    obligation,
                    proposition: goal.clone(),
                }],
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    let bundle = ProofBundle {
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(65).expect("certificate"),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: goal,
                    rule: ProofRule::EqualityTransitivity {
                        left_equals_middle: Box::new(ProofNode {
                            conclusion: Proposition::Equal(value(result), value(computed)),
                            rule: ProofRule::SemanticAxiom { index: 1 },
                        }),
                        middle_equals_right: Box::new(ProofNode {
                            conclusion: Proposition::Equal(value(computed), term),
                            rule: ProofRule::SemanticAxiom { index: 0 },
                        }),
                    },
                },
            }),
        }],
    };
    verify_module(&module, &bundle, &AdmissionProfile::default())
        .expect("reconstructs the exact bitwise-not result axiom");

    let mut wrong_operand = module;
    wrong_operand.machines[0].contract.ensures.clear();
    wrong_operand.machines[0].parameters[0].scalar_type = ScalarType::Boolean;
    assert!(matches!(
        validate_module(&wrong_operand),
        Err(ModuleError::IntegerBitwiseNotOperandTypeMismatch { .. })
    ));
}

#[test]
fn integer_widen_reconstructs_its_exact_result_axiom_and_rejects_partial_casts() {
    let source_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let target_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
    let source_scalar = ScalarType::Integer(source_type);
    let target_scalar = ScalarType::Integer(target_type);
    let operand = ValueId::new(68).expect("operand");
    let computed = ValueId::new(69).expect("computed");
    let result = ValueId::new(70).expect("result");
    let source = ScalarTerm::value(operand, source_scalar);
    let widened = ScalarTerm::integer_widen(source_type, target_type, source).unwrap();
    let goal = Proposition::Equal(ScalarTerm::value(result, target_scalar), widened.clone());
    let obligation = ObligationId::new(68).expect("obligation");
    let module = TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: MachineId::new(68).expect("machine"),
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
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            closed_reach_application: None,
            declared_service_reach: Vec::new(),
            id: MachineId::new(68).expect("machine"),
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: vec![ValueDeclaration {
                qualifications: Default::default(),
                id: operand,
                scalar_type: source_scalar,
            }],
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: result,
                scalar_type: target_scalar,
            }),
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: BlockId::new(68).expect("block"),
            blocks: vec![Block {
                erased_scalar_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: BlockId::new(68).expect("block"),
                parameters: Vec::new(),
                operations: vec![Operation {
                    static_reach_binding: None,
                    id: OperationId::new(68).expect("operation"),
                    result: terminal_psi::OperationResult::Scalar(ValueDeclaration {
                        qualifications: Default::default(),
                        id: computed,
                        scalar_type: target_scalar,
                    }),
                    kind: OperationKind::IntegerWiden { operand },
                }],
                terminator: Terminator::Return {
                    cleanup_actions: Vec::new(),
                    edge: EdgeId::new(68).expect("edge"),
                    value: computed,
                },
            }],
            contract: MachineContract {
                erased_scalar_formals: Vec::new(),
                id: ContractId::new(68).expect("contract"),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: vec![ContractClause {
                    obligation,
                    proposition: goal.clone(),
                }],
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    let bundle = ProofBundle {
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(68).expect("certificate"),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: goal,
                    rule: ProofRule::EqualityTransitivity {
                        left_equals_middle: Box::new(ProofNode {
                            conclusion: Proposition::Equal(
                                ScalarTerm::value(result, target_scalar),
                                ScalarTerm::value(computed, target_scalar),
                            ),
                            rule: ProofRule::SemanticAxiom { index: 1 },
                        }),
                        middle_equals_right: Box::new(ProofNode {
                            conclusion: Proposition::Equal(
                                ScalarTerm::value(computed, target_scalar),
                                widened,
                            ),
                            rule: ProofRule::SemanticAxiom { index: 0 },
                        }),
                    },
                },
            }),
        }],
    };
    verify_module(&module, &bundle, &AdmissionProfile::default())
        .expect("reconstructs the exact widening result axiom");

    let mut narrowing = module.clone();
    narrowing.machines[0].contract.ensures.clear();
    narrowing.machines[0].parameters[0].scalar_type = target_scalar;
    narrowing.machines[0].blocks[0].operations[0]
        .result
        .scalar_mut()
        .unwrap()
        .scalar_type = source_scalar;
    narrowing.machines[0]
        .result
        .scalar_mut()
        .unwrap()
        .scalar_type = source_scalar;
    assert!(matches!(
        validate_module(&narrowing),
        Err(ModuleError::IntegerWidenOperandTypeMismatch { .. })
    ));

    let mut cross_signedness = module;
    cross_signedness.machines[0].contract.ensures.clear();
    let unsigned_target = ScalarType::Integer(
        IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 cross-sign target"),
    );
    cross_signedness.machines[0].blocks[0].operations[0]
        .result
        .scalar_mut()
        .unwrap()
        .scalar_type = unsigned_target;
    cross_signedness.machines[0]
        .result
        .scalar_mut()
        .unwrap()
        .scalar_type = unsigned_target;
    assert!(matches!(
        validate_module(&cross_signedness),
        Err(ModuleError::IntegerWidenOperandTypeMismatch { .. })
    ));
}

#[test]
fn preserves_address_carrier_identity() {
    let address = ScalarType::Integer(IntegerType::address(64).expect("addr"));
    let parameter = ValueId::new(168).expect("parameter");
    let result = ValueId::new(169).expect("result");
    let module = TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: MachineId::new(168).expect("machine"),
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
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            closed_reach_application: None,
            declared_service_reach: Vec::new(),
            id: MachineId::new(168).expect("machine"),
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: vec![ValueDeclaration {
                qualifications: Default::default(),
                id: parameter,
                scalar_type: address,
            }],
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: result,
                scalar_type: address,
            }),
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: BlockId::new(168).expect("block"),
            blocks: vec![Block {
                erased_scalar_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: BlockId::new(168).expect("block"),
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: Terminator::Return {
                    cleanup_actions: Vec::new(),
                    edge: EdgeId::new(168).expect("edge"),
                    value: parameter,
                },
            }],
            contract: MachineContract {
                erased_scalar_formals: Vec::new(),
                id: ContractId::new(168).expect("contract"),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };

    validate_module(&module).expect("the distinct address carrier is admitted");
}

#[test]
fn exact_integer_cast_requires_a_distinct_fixed_partial_conversion_and_obligation() {
    let source =
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 source"));
    let target =
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 target"));
    let operand = ValueId::new(170).expect("operand");
    let computed = ValueId::new(171).expect("computed");
    let result = ValueId::new(172).expect("result");
    let cast_obligation = ObligationId::new(170).expect("cast obligation");
    let mut module = TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: MachineId::new(170).expect("machine"),
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
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            closed_reach_application: None,
            declared_service_reach: Vec::new(),
            id: MachineId::new(170).expect("machine"),
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: vec![ValueDeclaration {
                qualifications: Default::default(),
                id: operand,
                scalar_type: source,
            }],
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: result,
                scalar_type: target,
            }),
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: BlockId::new(170).expect("block"),
            blocks: vec![Block {
                erased_scalar_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: BlockId::new(170).expect("block"),
                parameters: Vec::new(),
                operations: vec![Operation {
                    static_reach_binding: None,
                    id: OperationId::new(170).expect("operation"),
                    result: terminal_psi::OperationResult::Scalar(ValueDeclaration {
                        qualifications: Default::default(),
                        id: computed,
                        scalar_type: target,
                    }),
                    kind: OperationKind::IntegerExactCast {
                        operand,
                        obligation: cast_obligation,
                    },
                }],
                terminator: Terminator::Return {
                    cleanup_actions: Vec::new(),
                    edge: EdgeId::new(170).expect("edge"),
                    value: computed,
                },
            }],
            contract: MachineContract {
                erased_scalar_formals: Vec::new(),
                id: ContractId::new(170).expect("contract"),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    validate_module(&module).expect("admits a proof-owned fixed partial conversion");
    let reconstructed =
        reconstruct_operation_obligations(&module).expect("reconstruct exact-cast goal");
    assert_eq!(reconstructed.len(), 1);
    assert!(reconstructed[0].canonical_certificate);
    assert_eq!(
        reconstructed[0].obligation.proposition,
        Proposition::IntegerMathLessOrEqual(
            IntegerMathTerm::MathValue {
                source_type: IntegerType::new(IntegerSign::Unsigned, 64).expect("u64"),
                value: operand,
            },
            IntegerMathTerm::literal(IntegerValue::Unsigned(255)),
        ),
    );

    let mut redundant = module.clone();
    redundant.machines[0].blocks[0].operations[0]
        .result
        .scalar_mut()
        .unwrap()
        .scalar_type = source;
    redundant.machines[0]
        .result
        .scalar_mut()
        .unwrap()
        .scalar_type = source;
    assert!(matches!(
        validate_module(&redundant),
        Err(ModuleError::IntegerExactCastOperandTypeMismatch { .. })
    ));

    let address = ScalarType::Integer(IntegerType::address(64).expect("addr"));
    module.machines[0].parameters[0].scalar_type = address;
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::IntegerExactCastOperandTypeMismatch { .. })
    ));
}

#[test]
fn exact_right_shift_requires_fixed_integer_operands_and_an_obligation() {
    let value_type =
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 value"));
    let count_type =
        ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 16).expect("i16 count"));
    let value = ValueId::new(180).expect("value");
    let count = ValueId::new(181).expect("count");
    let computed = ValueId::new(182).expect("computed");
    let result = ValueId::new(183).expect("result");
    let mut module = TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: MachineId::new(180).expect("machine"),
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
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            closed_reach_application: None,
            declared_service_reach: Vec::new(),
            id: MachineId::new(180).expect("machine"),
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: vec![
                ValueDeclaration {
                    qualifications: Default::default(),
                    id: value,
                    scalar_type: value_type,
                },
                ValueDeclaration {
                    qualifications: Default::default(),
                    id: count,
                    scalar_type: count_type,
                },
            ],
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: result,
                scalar_type: value_type,
            }),
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: BlockId::new(180).expect("block"),
            blocks: vec![Block {
                erased_scalar_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: BlockId::new(180).expect("block"),
                parameters: Vec::new(),
                operations: vec![Operation {
                    static_reach_binding: None,
                    id: OperationId::new(180).expect("operation"),
                    result: terminal_psi::OperationResult::Scalar(ValueDeclaration {
                        qualifications: Default::default(),
                        id: computed,
                        scalar_type: value_type,
                    }),
                    kind: OperationKind::ExactIntegerShiftRight {
                        value,
                        count,
                        obligation: ObligationId::new(180).expect("shift obligation"),
                    },
                }],
                terminator: Terminator::Return {
                    cleanup_actions: Vec::new(),
                    edge: EdgeId::new(180).expect("edge"),
                    value: computed,
                },
            }],
            contract: MachineContract {
                erased_scalar_formals: Vec::new(),
                id: ContractId::new(180).expect("contract"),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    validate_module(&module).expect("admits proof-gated exact right shift");

    module.machines[0].parameters[1].scalar_type = ScalarType::Boolean;
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::ExactIntegerShiftOperandTypeMismatch { .. })
    ));
}

#[test]
fn exact_left_shift_requires_fixed_integer_operands_and_an_obligation() {
    let value_type =
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).expect("u32 value"));
    let count_type =
        ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 16).expect("i16 count"));
    let value = ValueId::new(190).expect("value");
    let count = ValueId::new(191).expect("count");
    let computed = ValueId::new(192).expect("computed");
    let result = ValueId::new(193).expect("result");
    let mut module = TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: MachineId::new(190).expect("machine"),
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
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            closed_reach_application: None,
            declared_service_reach: Vec::new(),
            id: MachineId::new(190).expect("machine"),
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: vec![
                ValueDeclaration {
                    qualifications: Default::default(),
                    id: value,
                    scalar_type: value_type,
                },
                ValueDeclaration {
                    qualifications: Default::default(),
                    id: count,
                    scalar_type: count_type,
                },
            ],
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: result,
                scalar_type: value_type,
            }),
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: BlockId::new(190).expect("block"),
            blocks: vec![Block {
                erased_scalar_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: BlockId::new(190).expect("block"),
                parameters: Vec::new(),
                operations: vec![Operation {
                    static_reach_binding: None,
                    id: OperationId::new(190).expect("operation"),
                    result: terminal_psi::OperationResult::Scalar(ValueDeclaration {
                        qualifications: Default::default(),
                        id: computed,
                        scalar_type: value_type,
                    }),
                    kind: OperationKind::ExactIntegerShiftLeft {
                        value,
                        count,
                        obligation: ObligationId::new(190).expect("shift obligation"),
                    },
                }],
                terminator: Terminator::Return {
                    cleanup_actions: Vec::new(),
                    edge: EdgeId::new(190).expect("edge"),
                    value: computed,
                },
            }],
            contract: MachineContract {
                erased_scalar_formals: Vec::new(),
                id: ContractId::new(190).expect("contract"),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    validate_module(&module).expect("admits proof-gated exact left shift");

    module.machines[0].parameters[1].scalar_type = ScalarType::Boolean;
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::ExactIntegerShiftOperandTypeMismatch { .. })
    ));
}

#[test]
fn exact_add_requires_same_fixed_integer_operands_and_an_obligation() {
    let scalar_type =
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).expect("u32"));
    let left = ValueId::new(194).expect("left");
    let right = ValueId::new(195).expect("right");
    let computed = ValueId::new(196).expect("computed");
    let result = ValueId::new(197).expect("result");
    let declaration = |id| ValueDeclaration {
        id,
        scalar_type,
        qualifications: Default::default(),
    };
    let mut module = TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: MachineId::new(194).expect("machine"),
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
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            closed_reach_application: None,
            declared_service_reach: Vec::new(),
            id: MachineId::new(194).expect("machine"),
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: vec![declaration(left), declaration(right)],
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(declaration(result)),
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: BlockId::new(194).expect("block"),
            blocks: vec![Block {
                erased_scalar_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: BlockId::new(194).expect("block"),
                parameters: Vec::new(),
                operations: vec![Operation {
                    static_reach_binding: None,
                    id: OperationId::new(194).expect("operation"),
                    result: terminal_psi::OperationResult::Scalar(declaration(computed)),
                    kind: OperationKind::ExactIntegerAdd {
                        left,
                        right,
                        obligation: ObligationId::new(194).expect("add obligation"),
                    },
                }],
                terminator: Terminator::Return {
                    cleanup_actions: Vec::new(),
                    edge: EdgeId::new(194).expect("edge"),
                    value: computed,
                },
            }],
            contract: MachineContract {
                erased_scalar_formals: Vec::new(),
                id: ContractId::new(194).expect("contract"),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    validate_module(&module).expect("admits proof-gated exact addition");

    module.machines[0].parameters[1].scalar_type = ScalarType::Boolean;
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::ExactIntegerAddOperandTypeMismatch { .. })
    ));
}
