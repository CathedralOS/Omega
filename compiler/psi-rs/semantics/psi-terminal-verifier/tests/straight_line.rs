use psi_core::{
    BlockId, ContractId, EdgeId, EvidenceIdentity, IntegerSign, IntegerType, IntegerValue,
    MachineId, ObligationId, OperationId, Proposition, ScalarTerm, ScalarType, ValueId,
};
use psi_proof_kernel::{
    AdmissionProfile, CertificateEnvelope, EvidenceRoute, ProofNode, ProofRule, ProofSystemVersion,
};
use psi_terminal::{
    Block, ContractClause, MachineContract, Operation, OperationKind, SemanticVersion,
    TerminalMachine, TerminalModule, Terminator, ValueDeclaration,
};
use psi_terminal_verifier::{
    ContractClauseKind, ModuleError, ObligationEvidence, ProofBundle, VerificationError,
    validate_module, verify_module,
};

#[test]
fn straight_line_integer_contract_is_reconstructed_and_verified() {
    let fixture = Fixture::new();
    let verified = verify_module(
        &fixture.module,
        &fixture.proof_bundle(),
        &AdmissionProfile::default(),
    )
    .expect("terminal module verifies independently of producer state");

    assert_eq!(verified.module(), &fixture.module);
    assert_eq!(verified.accepted_facts().len(), 1);
    assert_eq!(verified.accepted_facts()[0].obligation, fixture.obligation);
}

#[test]
fn verifier_reconstructs_every_contract_obligation() {
    let fixture = Fixture::new();
    assert_eq!(
        verify_module(
            &fixture.module,
            &ProofBundle::default(),
            &AdmissionProfile::default(),
        )
        .expect_err("proof bundle cannot omit a bodyful guarantee"),
        VerificationError::MissingEvidence(fixture.obligation)
    );
}

#[test]
fn v2_boolean_constant_axiom_proves_the_return_contract() {
    let constant = ValueId::new(10).expect("constant");
    let result = ValueId::new(11).expect("result");
    let obligation = ObligationId::new(10).expect("obligation");
    let term = |id| ScalarTerm::value(id, ScalarType::Boolean);
    let goal = Proposition::Equal(term(result), ScalarTerm::boolean(true));
    let module = TerminalModule {
        semantic_version: SemanticVersion::V2,
        entry: MachineId::new(10).expect("machine"),
        machines: vec![TerminalMachine {
            id: MachineId::new(10).expect("machine"),
            parameters: Vec::new(),
            result: ValueDeclaration {
                id: result,
                scalar_type: ScalarType::Boolean,
            },
            entry: BlockId::new(10).expect("block"),
            blocks: vec![Block {
                id: BlockId::new(10).expect("block"),
                parameters: Vec::new(),
                operations: vec![Operation {
                    id: OperationId::new(10).expect("operation"),
                    result: ValueDeclaration {
                        id: constant,
                        scalar_type: ScalarType::Boolean,
                    },
                    kind: OperationKind::BooleanConstant { value: true },
                }],
                terminator: Terminator::Return {
                    edge: EdgeId::new(10).expect("edge"),
                    value: constant,
                },
            }],
            contract: MachineContract {
                id: ContractId::new(10).expect("contract"),
                requires: Vec::new(),
                ensures: vec![ContractClause {
                    obligation,
                    proposition: goal.clone(),
                }],
            },
        }],
    };
    let proof = ProofNode {
        conclusion: goal,
        rule: ProofRule::EqualityTransitivity {
            left_equals_middle: Box::new(ProofNode {
                conclusion: Proposition::Equal(term(result), term(constant)),
                rule: ProofRule::SemanticAxiom { index: 1 },
            }),
            middle_equals_right: Box::new(ProofNode {
                conclusion: Proposition::Equal(term(constant), ScalarTerm::boolean(true)),
                rule: ProofRule::SemanticAxiom { index: 0 },
            }),
        },
    };
    let bundle = ProofBundle {
        evidence: vec![ObligationEvidence {
            obligation,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(10).expect("certificate"),
                proof_system_version: ProofSystemVersion::CURRENT,
                proof,
            }),
        }],
    };

    verify_module(&module, &bundle, &AdmissionProfile::default())
        .expect("v2 Boolean semantics should reconstruct both axioms");
}

#[test]
fn v3_wrapping_add_axiom_proves_the_return_contract() {
    let (module, goal, obligation) = wrapping_add_module();
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let scalar_type = ScalarType::Integer(integer);
    let term = |raw| ScalarTerm::value(ValueId::new(raw).unwrap(), scalar_type);
    let sum = ScalarTerm::wrapping_integer_add(integer, term(20), term(21)).unwrap();
    let proof = ProofNode {
        conclusion: goal,
        rule: ProofRule::EqualityTransitivity {
            left_equals_middle: Box::new(ProofNode {
                conclusion: Proposition::Equal(term(23), term(22)),
                rule: ProofRule::SemanticAxiom { index: 1 },
            }),
            middle_equals_right: Box::new(ProofNode {
                conclusion: Proposition::Equal(term(22), sum),
                rule: ProofRule::SemanticAxiom { index: 0 },
            }),
        },
    };
    let bundle = ProofBundle {
        evidence: vec![ObligationEvidence {
            obligation,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(20).expect("certificate"),
                proof_system_version: ProofSystemVersion::CURRENT,
                proof,
            }),
        }],
    };

    verify_module(&module, &bundle, &AdmissionProfile::default())
        .expect("v3 wrapping-add semantics should reconstruct both axioms");
}

#[test]
fn wrapping_add_requires_v3_and_defined_exact_type_operands() {
    let (mut old_version, _, _) = wrapping_add_module();
    old_version.semantic_version = SemanticVersion::V2;
    assert!(matches!(
        validate_module(&old_version),
        Err(ModuleError::OperationRequiresSemanticVersion {
            required: SemanticVersion::V3,
            actual: SemanticVersion::V2,
            ..
        })
    ));

    let (mut use_before_definition, _, _) = wrapping_add_module();
    use_before_definition.machines[0].blocks[0].operations[0].kind =
        OperationKind::WrappingIntegerAdd {
            left: ValueId::new(22).expect("sum result"),
            right: ValueId::new(21).expect("right parameter"),
        };
    assert_eq!(
        validate_module(&use_before_definition).expect_err("self-reference must fail closed"),
        ModuleError::ValueUsedBeforeDefinition(ValueId::new(22).expect("sum result"))
    );

    let (mut wrong_type, _, _) = wrapping_add_module();
    wrong_type.machines[0].parameters[1].scalar_type = ScalarType::Boolean;
    wrong_type.machines[0].contract.ensures.clear();
    assert_eq!(
        validate_module(&wrong_type).expect_err("mixed operand types must fail closed"),
        ModuleError::WrappingIntegerAddOperandTypeMismatch {
            operation: OperationId::new(20).expect("add operation"),
            operand: ValueId::new(21).expect("right parameter"),
            expected: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).expect("u8")),
            actual: ScalarType::Boolean,
        }
    );
}

#[test]
fn v4_saturating_add_axiom_proves_the_return_contract() {
    let (module, goal, obligation) = saturating_add_module();
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let scalar_type = ScalarType::Integer(integer);
    let term = |raw| ScalarTerm::value(ValueId::new(raw).unwrap(), scalar_type);
    let sum = ScalarTerm::saturating_integer_add(integer, term(30), term(31)).unwrap();
    let proof = ProofNode {
        conclusion: goal,
        rule: ProofRule::EqualityTransitivity {
            left_equals_middle: Box::new(ProofNode {
                conclusion: Proposition::Equal(term(33), term(32)),
                rule: ProofRule::SemanticAxiom { index: 1 },
            }),
            middle_equals_right: Box::new(ProofNode {
                conclusion: Proposition::Equal(term(32), sum),
                rule: ProofRule::SemanticAxiom { index: 0 },
            }),
        },
    };
    let bundle = ProofBundle {
        evidence: vec![ObligationEvidence {
            obligation,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(30).expect("certificate"),
                proof_system_version: ProofSystemVersion::CURRENT,
                proof,
            }),
        }],
    };

    verify_module(&module, &bundle, &AdmissionProfile::default())
        .expect("v4 saturating-add semantics should reconstruct both axioms");
}

#[test]
fn saturating_add_requires_v4_and_defined_exact_type_operands() {
    let (mut old_version, _, _) = saturating_add_module();
    old_version.semantic_version = SemanticVersion::V3;
    assert!(matches!(
        validate_module(&old_version),
        Err(ModuleError::OperationRequiresSemanticVersion {
            required: SemanticVersion::V4,
            actual: SemanticVersion::V3,
            ..
        })
    ));

    let (mut use_before_definition, _, _) = saturating_add_module();
    use_before_definition.machines[0].blocks[0].operations[0].kind =
        OperationKind::SaturatingIntegerAdd {
            left: ValueId::new(32).expect("sum result"),
            right: ValueId::new(31).expect("right parameter"),
        };
    assert_eq!(
        validate_module(&use_before_definition).expect_err("self-reference must fail closed"),
        ModuleError::ValueUsedBeforeDefinition(ValueId::new(32).expect("sum result"))
    );

    let (mut wrong_type, _, _) = saturating_add_module();
    wrong_type.machines[0].parameters[1].scalar_type = ScalarType::Boolean;
    wrong_type.machines[0].contract.ensures.clear();
    assert_eq!(
        validate_module(&wrong_type).expect_err("mixed operand types must fail closed"),
        ModuleError::SaturatingIntegerAddOperandTypeMismatch {
            operation: OperationId::new(30).expect("add operation"),
            operand: ValueId::new(31).expect("right parameter"),
            expected: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).expect("u8")),
            actual: ScalarType::Boolean,
        }
    );
}

#[test]
fn v5_wrapping_subtract_axiom_proves_the_return_contract() {
    let (module, goal, obligation) = wrapping_subtract_module();
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let scalar_type = ScalarType::Integer(integer);
    let term = |raw| ScalarTerm::value(ValueId::new(raw).unwrap(), scalar_type);
    let difference = ScalarTerm::wrapping_integer_subtract(integer, term(40), term(41)).unwrap();
    let proof = ProofNode {
        conclusion: goal,
        rule: ProofRule::EqualityTransitivity {
            left_equals_middle: Box::new(ProofNode {
                conclusion: Proposition::Equal(term(43), term(42)),
                rule: ProofRule::SemanticAxiom { index: 1 },
            }),
            middle_equals_right: Box::new(ProofNode {
                conclusion: Proposition::Equal(term(42), difference),
                rule: ProofRule::SemanticAxiom { index: 0 },
            }),
        },
    };
    let bundle = ProofBundle {
        evidence: vec![ObligationEvidence {
            obligation,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(40).expect("certificate"),
                proof_system_version: ProofSystemVersion::CURRENT,
                proof,
            }),
        }],
    };

    verify_module(&module, &bundle, &AdmissionProfile::default())
        .expect("v5 wrapping-subtract semantics should reconstruct both axioms");
}

#[test]
fn wrapping_subtract_requires_v5_and_defined_exact_type_operands() {
    let (mut old_version, _, _) = wrapping_subtract_module();
    old_version.semantic_version = SemanticVersion::V4;
    assert!(matches!(
        validate_module(&old_version),
        Err(ModuleError::OperationRequiresSemanticVersion {
            required: SemanticVersion::V5,
            actual: SemanticVersion::V4,
            ..
        })
    ));

    let (mut use_before_definition, _, _) = wrapping_subtract_module();
    use_before_definition.machines[0].blocks[0].operations[0].kind =
        OperationKind::WrappingIntegerSubtract {
            left: ValueId::new(42).expect("difference result"),
            right: ValueId::new(41).expect("right parameter"),
        };
    assert_eq!(
        validate_module(&use_before_definition).expect_err("self-reference must fail closed"),
        ModuleError::ValueUsedBeforeDefinition(ValueId::new(42).expect("difference result"))
    );

    let (mut wrong_type, _, _) = wrapping_subtract_module();
    wrong_type.machines[0].parameters[1].scalar_type = ScalarType::Boolean;
    wrong_type.machines[0].contract.ensures.clear();
    assert_eq!(
        validate_module(&wrong_type).expect_err("mixed operand types must fail closed"),
        ModuleError::WrappingIntegerSubtractOperandTypeMismatch {
            operation: OperationId::new(40).expect("subtract operation"),
            operand: ValueId::new(41).expect("right parameter"),
            expected: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).expect("u8")),
            actual: ScalarType::Boolean,
        }
    );
}

#[test]
fn v6_saturating_subtract_axiom_proves_the_return_contract() {
    let (module, goal, obligation) = saturating_subtract_module();
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let scalar_type = ScalarType::Integer(integer);
    let term = |raw| ScalarTerm::value(ValueId::new(raw).unwrap(), scalar_type);
    let difference = ScalarTerm::saturating_integer_subtract(integer, term(50), term(51)).unwrap();
    let proof = ProofNode {
        conclusion: goal,
        rule: ProofRule::EqualityTransitivity {
            left_equals_middle: Box::new(ProofNode {
                conclusion: Proposition::Equal(term(53), term(52)),
                rule: ProofRule::SemanticAxiom { index: 1 },
            }),
            middle_equals_right: Box::new(ProofNode {
                conclusion: Proposition::Equal(term(52), difference),
                rule: ProofRule::SemanticAxiom { index: 0 },
            }),
        },
    };
    let bundle = ProofBundle {
        evidence: vec![ObligationEvidence {
            obligation,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(50).expect("certificate"),
                proof_system_version: ProofSystemVersion::CURRENT,
                proof,
            }),
        }],
    };

    verify_module(&module, &bundle, &AdmissionProfile::default())
        .expect("v6 saturating-subtract semantics should reconstruct both axioms");
}

#[test]
fn saturating_subtract_requires_v6_and_defined_exact_type_operands() {
    let (mut old_version, _, _) = saturating_subtract_module();
    old_version.semantic_version = SemanticVersion::V5;
    assert!(matches!(
        validate_module(&old_version),
        Err(ModuleError::OperationRequiresSemanticVersion {
            required: SemanticVersion::V6,
            actual: SemanticVersion::V5,
            ..
        })
    ));

    let (mut use_before_definition, _, _) = saturating_subtract_module();
    use_before_definition.machines[0].blocks[0].operations[0].kind =
        OperationKind::SaturatingIntegerSubtract {
            left: ValueId::new(52).expect("difference result"),
            right: ValueId::new(51).expect("right parameter"),
        };
    assert_eq!(
        validate_module(&use_before_definition).expect_err("self-reference must fail closed"),
        ModuleError::ValueUsedBeforeDefinition(ValueId::new(52).expect("difference result"))
    );

    let (mut wrong_type, _, _) = saturating_subtract_module();
    wrong_type.machines[0].parameters[1].scalar_type = ScalarType::Boolean;
    wrong_type.machines[0].contract.ensures.clear();
    assert_eq!(
        validate_module(&wrong_type).expect_err("mixed operand types must fail closed"),
        ModuleError::SaturatingIntegerSubtractOperandTypeMismatch {
            operation: OperationId::new(50).expect("subtract operation"),
            operand: ValueId::new(51).expect("right parameter"),
            expected: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).expect("u8")),
            actual: ScalarType::Boolean,
        }
    );
}

#[test]
fn v7_wrapping_multiply_axiom_proves_the_return_contract() {
    let (module, goal, obligation) = wrapping_multiply_module();
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let scalar_type = ScalarType::Integer(integer);
    let term = |raw| ScalarTerm::value(ValueId::new(raw).unwrap(), scalar_type);
    let product = ScalarTerm::wrapping_integer_multiply(integer, term(60), term(61)).unwrap();
    let proof = ProofNode {
        conclusion: goal,
        rule: ProofRule::EqualityTransitivity {
            left_equals_middle: Box::new(ProofNode {
                conclusion: Proposition::Equal(term(63), term(62)),
                rule: ProofRule::SemanticAxiom { index: 1 },
            }),
            middle_equals_right: Box::new(ProofNode {
                conclusion: Proposition::Equal(term(62), product),
                rule: ProofRule::SemanticAxiom { index: 0 },
            }),
        },
    };
    let bundle = ProofBundle {
        evidence: vec![ObligationEvidence {
            obligation,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(60).expect("certificate"),
                proof_system_version: ProofSystemVersion::CURRENT,
                proof,
            }),
        }],
    };

    verify_module(&module, &bundle, &AdmissionProfile::default())
        .expect("v7 wrapping-multiply semantics should reconstruct both axioms");
}

#[test]
fn wrapping_multiply_requires_v7_and_defined_exact_type_operands() {
    let (mut old_version, _, _) = wrapping_multiply_module();
    old_version.semantic_version = SemanticVersion::V6;
    assert!(matches!(
        validate_module(&old_version),
        Err(ModuleError::OperationRequiresSemanticVersion {
            required: SemanticVersion::V7,
            actual: SemanticVersion::V6,
            ..
        })
    ));

    let (mut use_before_definition, _, _) = wrapping_multiply_module();
    use_before_definition.machines[0].blocks[0].operations[0].kind =
        OperationKind::WrappingIntegerMultiply {
            left: ValueId::new(62).expect("product result"),
            right: ValueId::new(61).expect("right parameter"),
        };
    assert_eq!(
        validate_module(&use_before_definition).expect_err("self-reference must fail closed"),
        ModuleError::ValueUsedBeforeDefinition(ValueId::new(62).expect("product result"))
    );

    let (mut wrong_type, _, _) = wrapping_multiply_module();
    wrong_type.machines[0].parameters[1].scalar_type = ScalarType::Boolean;
    wrong_type.machines[0].contract.ensures.clear();
    assert_eq!(
        validate_module(&wrong_type).expect_err("mixed operand types must fail closed"),
        ModuleError::WrappingIntegerMultiplyOperandTypeMismatch {
            operation: OperationId::new(60).expect("multiply operation"),
            operand: ValueId::new(61).expect("right parameter"),
            expected: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).expect("u8")),
            actual: ScalarType::Boolean,
        }
    );
}

#[test]
fn initial_control_vocabulary_rejects_unreachable_semantic_axioms() {
    let mut fixture = Fixture::new();
    fixture.module.machines[0].blocks.push(Block {
        id: BlockId::new(3).expect("unreachable block"),
        parameters: Vec::new(),
        operations: Vec::new(),
        terminator: Terminator::Return {
            edge: EdgeId::new(3).expect("unreachable edge"),
            value: fixture.constant,
        },
    });
    assert_eq!(
        validate_module(&fixture.module).expect_err("unreachable facts must not enter proofs"),
        ModuleError::UnreachableBlock(BlockId::new(3).expect("unreachable block"))
    );
}

#[test]
fn entry_requirements_cannot_assume_an_internal_operation_value() {
    let mut fixture = Fixture::new();
    fixture.module.machines[0]
        .contract
        .requires
        .push(Proposition::Equal(
            ScalarTerm::value(fixture.constant, ScalarType::Integer(fixture.integer)),
            ScalarTerm::integer(fixture.integer, IntegerValue::Signed(7)).expect("seven"),
        ));
    assert_eq!(
        validate_module(&fixture.module).expect_err("internal facts do not exist at entry"),
        ModuleError::ContractValueOutsideScope {
            contract: ContractId::new(1).expect("contract"),
            clause: ContractClauseKind::Requires,
            value: fixture.constant,
        }
    );
}

fn wrapping_add_module() -> (TerminalModule, Proposition, ObligationId) {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let scalar_type = ScalarType::Integer(integer);
    let left = ValueId::new(20).expect("left parameter");
    let right = ValueId::new(21).expect("right parameter");
    let sum = ValueId::new(22).expect("sum result");
    let result = ValueId::new(23).expect("machine result");
    let obligation = ObligationId::new(20).expect("obligation");
    let term = |id| ScalarTerm::value(id, scalar_type);
    let goal = Proposition::Equal(
        term(result),
        ScalarTerm::wrapping_integer_add(integer, term(left), term(right)).unwrap(),
    );
    let machine = TerminalMachine {
        id: MachineId::new(20).expect("machine"),
        parameters: vec![
            ValueDeclaration {
                id: left,
                scalar_type,
            },
            ValueDeclaration {
                id: right,
                scalar_type,
            },
        ],
        result: ValueDeclaration {
            id: result,
            scalar_type,
        },
        entry: BlockId::new(20).expect("block"),
        blocks: vec![Block {
            id: BlockId::new(20).expect("block"),
            parameters: Vec::new(),
            operations: vec![Operation {
                id: OperationId::new(20).expect("add operation"),
                result: ValueDeclaration {
                    id: sum,
                    scalar_type,
                },
                kind: OperationKind::WrappingIntegerAdd { left, right },
            }],
            terminator: Terminator::Return {
                edge: EdgeId::new(20).expect("return edge"),
                value: sum,
            },
        }],
        contract: MachineContract {
            id: ContractId::new(20).expect("contract"),
            requires: Vec::new(),
            ensures: vec![ContractClause {
                obligation,
                proposition: goal.clone(),
            }],
        },
    };
    (
        TerminalModule {
            semantic_version: SemanticVersion::V3,
            entry: machine.id,
            machines: vec![machine],
        },
        goal,
        obligation,
    )
}

fn saturating_add_module() -> (TerminalModule, Proposition, ObligationId) {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let scalar_type = ScalarType::Integer(integer);
    let left = ValueId::new(30).expect("left parameter");
    let right = ValueId::new(31).expect("right parameter");
    let sum = ValueId::new(32).expect("sum result");
    let result = ValueId::new(33).expect("machine result");
    let obligation = ObligationId::new(30).expect("obligation");
    let term = |id| ScalarTerm::value(id, scalar_type);
    let goal = Proposition::Equal(
        term(result),
        ScalarTerm::saturating_integer_add(integer, term(left), term(right)).unwrap(),
    );
    let machine = TerminalMachine {
        id: MachineId::new(30).expect("machine"),
        parameters: vec![
            ValueDeclaration {
                id: left,
                scalar_type,
            },
            ValueDeclaration {
                id: right,
                scalar_type,
            },
        ],
        result: ValueDeclaration {
            id: result,
            scalar_type,
        },
        entry: BlockId::new(30).expect("block"),
        blocks: vec![Block {
            id: BlockId::new(30).expect("block"),
            parameters: Vec::new(),
            operations: vec![Operation {
                id: OperationId::new(30).expect("add operation"),
                result: ValueDeclaration {
                    id: sum,
                    scalar_type,
                },
                kind: OperationKind::SaturatingIntegerAdd { left, right },
            }],
            terminator: Terminator::Return {
                edge: EdgeId::new(30).expect("return edge"),
                value: sum,
            },
        }],
        contract: MachineContract {
            id: ContractId::new(30).expect("contract"),
            requires: Vec::new(),
            ensures: vec![ContractClause {
                obligation,
                proposition: goal.clone(),
            }],
        },
    };
    (
        TerminalModule {
            semantic_version: SemanticVersion::V4,
            entry: machine.id,
            machines: vec![machine],
        },
        goal,
        obligation,
    )
}

fn wrapping_subtract_module() -> (TerminalModule, Proposition, ObligationId) {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let scalar_type = ScalarType::Integer(integer);
    let left = ValueId::new(40).expect("left parameter");
    let right = ValueId::new(41).expect("right parameter");
    let difference = ValueId::new(42).expect("difference result");
    let result = ValueId::new(43).expect("machine result");
    let obligation = ObligationId::new(40).expect("obligation");
    let term = |id| ScalarTerm::value(id, scalar_type);
    let goal = Proposition::Equal(
        term(result),
        ScalarTerm::wrapping_integer_subtract(integer, term(left), term(right)).unwrap(),
    );
    let machine = TerminalMachine {
        id: MachineId::new(40).expect("machine"),
        parameters: vec![
            ValueDeclaration {
                id: left,
                scalar_type,
            },
            ValueDeclaration {
                id: right,
                scalar_type,
            },
        ],
        result: ValueDeclaration {
            id: result,
            scalar_type,
        },
        entry: BlockId::new(40).expect("block"),
        blocks: vec![Block {
            id: BlockId::new(40).expect("block"),
            parameters: Vec::new(),
            operations: vec![Operation {
                id: OperationId::new(40).expect("subtract operation"),
                result: ValueDeclaration {
                    id: difference,
                    scalar_type,
                },
                kind: OperationKind::WrappingIntegerSubtract { left, right },
            }],
            terminator: Terminator::Return {
                edge: EdgeId::new(40).expect("return edge"),
                value: difference,
            },
        }],
        contract: MachineContract {
            id: ContractId::new(40).expect("contract"),
            requires: Vec::new(),
            ensures: vec![ContractClause {
                obligation,
                proposition: goal.clone(),
            }],
        },
    };
    (
        TerminalModule {
            semantic_version: SemanticVersion::V5,
            entry: machine.id,
            machines: vec![machine],
        },
        goal,
        obligation,
    )
}

fn saturating_subtract_module() -> (TerminalModule, Proposition, ObligationId) {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let scalar_type = ScalarType::Integer(integer);
    let left = ValueId::new(50).expect("left parameter");
    let right = ValueId::new(51).expect("right parameter");
    let difference = ValueId::new(52).expect("difference result");
    let result = ValueId::new(53).expect("machine result");
    let obligation = ObligationId::new(50).expect("obligation");
    let term = |id| ScalarTerm::value(id, scalar_type);
    let goal = Proposition::Equal(
        term(result),
        ScalarTerm::saturating_integer_subtract(integer, term(left), term(right)).unwrap(),
    );
    let machine = TerminalMachine {
        id: MachineId::new(50).expect("machine"),
        parameters: vec![
            ValueDeclaration {
                id: left,
                scalar_type,
            },
            ValueDeclaration {
                id: right,
                scalar_type,
            },
        ],
        result: ValueDeclaration {
            id: result,
            scalar_type,
        },
        entry: BlockId::new(50).expect("block"),
        blocks: vec![Block {
            id: BlockId::new(50).expect("block"),
            parameters: Vec::new(),
            operations: vec![Operation {
                id: OperationId::new(50).expect("subtract operation"),
                result: ValueDeclaration {
                    id: difference,
                    scalar_type,
                },
                kind: OperationKind::SaturatingIntegerSubtract { left, right },
            }],
            terminator: Terminator::Return {
                edge: EdgeId::new(50).expect("return edge"),
                value: difference,
            },
        }],
        contract: MachineContract {
            id: ContractId::new(50).expect("contract"),
            requires: Vec::new(),
            ensures: vec![ContractClause {
                obligation,
                proposition: goal.clone(),
            }],
        },
    };
    (
        TerminalModule {
            semantic_version: SemanticVersion::V6,
            entry: machine.id,
            machines: vec![machine],
        },
        goal,
        obligation,
    )
}

fn wrapping_multiply_module() -> (TerminalModule, Proposition, ObligationId) {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let scalar_type = ScalarType::Integer(integer);
    let left = ValueId::new(60).expect("left parameter");
    let right = ValueId::new(61).expect("right parameter");
    let product = ValueId::new(62).expect("product result");
    let result = ValueId::new(63).expect("machine result");
    let obligation = ObligationId::new(60).expect("obligation");
    let term = |id| ScalarTerm::value(id, scalar_type);
    let goal = Proposition::Equal(
        term(result),
        ScalarTerm::wrapping_integer_multiply(integer, term(left), term(right)).unwrap(),
    );
    let machine = TerminalMachine {
        id: MachineId::new(60).expect("machine"),
        parameters: vec![
            ValueDeclaration {
                id: left,
                scalar_type,
            },
            ValueDeclaration {
                id: right,
                scalar_type,
            },
        ],
        result: ValueDeclaration {
            id: result,
            scalar_type,
        },
        entry: BlockId::new(60).expect("block"),
        blocks: vec![Block {
            id: BlockId::new(60).expect("block"),
            parameters: Vec::new(),
            operations: vec![Operation {
                id: OperationId::new(60).expect("multiply operation"),
                result: ValueDeclaration {
                    id: product,
                    scalar_type,
                },
                kind: OperationKind::WrappingIntegerMultiply { left, right },
            }],
            terminator: Terminator::Return {
                edge: EdgeId::new(60).expect("return edge"),
                value: product,
            },
        }],
        contract: MachineContract {
            id: ContractId::new(60).expect("contract"),
            requires: Vec::new(),
            ensures: vec![ContractClause {
                obligation,
                proposition: goal.clone(),
            }],
        },
    };
    (
        TerminalModule {
            semantic_version: SemanticVersion::V7,
            entry: machine.id,
            machines: vec![machine],
        },
        goal,
        obligation,
    )
}

struct Fixture {
    module: TerminalModule,
    integer: IntegerType,
    constant: ValueId,
    forwarded: ValueId,
    result: ValueId,
    obligation: ObligationId,
}

impl Fixture {
    fn new() -> Self {
        let integer = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
        let scalar_type = ScalarType::Integer(integer);
        let constant = ValueId::new(1).expect("constant value");
        let forwarded = ValueId::new(2).expect("forwarded value");
        let result = ValueId::new(3).expect("result value");
        let obligation = ObligationId::new(1).expect("ensures obligation");
        let seven = ScalarTerm::integer(integer, IntegerValue::Signed(7)).expect("seven");
        let goal = Proposition::Equal(ScalarTerm::value(result, scalar_type), seven);

        let machine = TerminalMachine {
            id: MachineId::new(1).expect("machine"),
            parameters: Vec::new(),
            result: ValueDeclaration {
                id: result,
                scalar_type,
            },
            entry: BlockId::new(1).expect("entry block"),
            blocks: vec![
                Block {
                    id: BlockId::new(1).expect("entry block"),
                    parameters: Vec::new(),
                    operations: vec![Operation {
                        id: OperationId::new(1).expect("constant operation"),
                        result: ValueDeclaration {
                            id: constant,
                            scalar_type,
                        },
                        kind: OperationKind::IntegerConstant {
                            value: IntegerValue::Signed(7),
                        },
                    }],
                    terminator: Terminator::Jump {
                        edge: EdgeId::new(1).expect("jump edge"),
                        target: BlockId::new(2).expect("exit block"),
                        arguments: vec![constant],
                    },
                },
                Block {
                    id: BlockId::new(2).expect("exit block"),
                    parameters: vec![ValueDeclaration {
                        id: forwarded,
                        scalar_type,
                    }],
                    operations: Vec::new(),
                    terminator: Terminator::Return {
                        edge: EdgeId::new(2).expect("return edge"),
                        value: forwarded,
                    },
                },
            ],
            contract: MachineContract {
                id: ContractId::new(1).expect("contract"),
                requires: Vec::new(),
                ensures: vec![ContractClause {
                    obligation,
                    proposition: goal,
                }],
            },
        };
        Self {
            module: TerminalModule {
                semantic_version: SemanticVersion::CURRENT,
                entry: machine.id,
                machines: vec![machine],
            },
            integer,
            constant,
            forwarded,
            result,
            obligation,
        }
    }

    fn proof_bundle(&self) -> ProofBundle {
        let scalar_type = ScalarType::Integer(self.integer);
        let term = |id| ScalarTerm::value(id, scalar_type);
        let seven = ScalarTerm::integer(self.integer, IntegerValue::Signed(7)).expect("seven");
        let constant_fact = Proposition::Equal(term(self.constant), seven.clone());
        let forwarding_fact = Proposition::Equal(term(self.forwarded), term(self.constant));
        let return_fact = Proposition::Equal(term(self.result), term(self.forwarded));
        let forwarded_is_seven = Proposition::Equal(term(self.forwarded), seven.clone());
        let goal = Proposition::Equal(term(self.result), seven);
        let proof = ProofNode {
            conclusion: goal,
            rule: ProofRule::EqualityTransitivity {
                left_equals_middle: Box::new(ProofNode {
                    conclusion: return_fact,
                    rule: ProofRule::SemanticAxiom { index: 2 },
                }),
                middle_equals_right: Box::new(ProofNode {
                    conclusion: forwarded_is_seven,
                    rule: ProofRule::EqualityTransitivity {
                        left_equals_middle: Box::new(ProofNode {
                            conclusion: forwarding_fact,
                            rule: ProofRule::SemanticAxiom { index: 1 },
                        }),
                        middle_equals_right: Box::new(ProofNode {
                            conclusion: constant_fact,
                            rule: ProofRule::SemanticAxiom { index: 0 },
                        }),
                    },
                }),
            },
        };
        ProofBundle {
            evidence: vec![ObligationEvidence {
                obligation: self.obligation,
                route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                    identity: EvidenceIdentity::new(1).expect("certificate"),
                    proof_system_version: ProofSystemVersion::CURRENT,
                    proof,
                }),
            }],
        }
    }
}
