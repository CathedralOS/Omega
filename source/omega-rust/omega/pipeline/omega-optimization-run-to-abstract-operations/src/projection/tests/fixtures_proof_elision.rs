use super::*;

pub(super) fn exact_add_verified() -> VerifiedPsiOptimizationUnit {
    exact_add_verified_with_result(true)
}

pub(super) fn dead_exact_add_verified() -> VerifiedPsiOptimizationUnit {
    exact_add_verified_with_result(false)
}

pub(super) fn live_exact_add_zero_verified() -> VerifiedPsiOptimizationUnit {
    let machine = MachineId::new(1_081).unwrap();
    let block = BlockId::new(1_082).unwrap();
    let left = ValueId::new(1_083).unwrap();
    let zero = ValueId::new(1_084).unwrap();
    let computed = ValueId::new(1_085).unwrap();
    let result = ValueId::new(1_086).unwrap();
    let obligation = ObligationId::new(1_087).unwrap();
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap());
    let declaration = |id| ValueDeclaration { id, scalar_type };
    let mut module = module_with_blocks(
        machine,
        block,
        TerminalMachineResult::Scalar(declaration(result)),
        vec![Block {
            id: block,
            parameters: Vec::new(),
            operations: vec![
                Operation {
                    id: OperationId::new(1_088).unwrap(),
                    result: OperationResult::Scalar(declaration(zero)),
                    kind: OperationKind::IntegerConstant {
                        value: IntegerValue::Unsigned(0),
                    },
                },
                Operation {
                    id: OperationId::new(1_089).unwrap(),
                    result: OperationResult::Scalar(declaration(computed)),
                    kind: OperationKind::ExactIntegerAdd {
                        left,
                        right: zero,
                        obligation,
                    },
                },
            ],
            terminator: Terminator::Return {
                cleanup_actions: Vec::new(),
                edge: EdgeId::new(1_090).unwrap(),
                value: computed,
            },
        }],
    );
    module.machines[0].parameters.push(declaration(left));
    verified(
        module,
        ProofBundle {
            evidence_producers: Vec::new(),
            evidence: vec![ObligationEvidence {
                obligation,
                route: EvidenceRoute::KernelDerived(PrimitiveJudgment::Truth),
            }],
        },
    )
}

pub(super) fn live_exact_divide_by_one_verified() -> VerifiedPsiOptimizationUnit {
    let machine = MachineId::new(1_091).unwrap();
    let block = BlockId::new(1_092).unwrap();
    let dividend = ValueId::new(1_093).unwrap();
    let one = ValueId::new(1_094).unwrap();
    let quotient = ValueId::new(1_095).unwrap();
    let result = ValueId::new(1_096).unwrap();
    let obligation = ObligationId::new(1_097).unwrap();
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap());
    let declaration = |id| ValueDeclaration { id, scalar_type };
    let mut module = module_with_blocks(
        machine,
        block,
        TerminalMachineResult::Scalar(declaration(result)),
        vec![Block {
            id: block,
            parameters: Vec::new(),
            operations: vec![
                Operation {
                    id: OperationId::new(1_098).unwrap(),
                    result: OperationResult::Scalar(declaration(one)),
                    kind: OperationKind::IntegerConstant {
                        value: IntegerValue::Unsigned(1),
                    },
                },
                Operation {
                    id: OperationId::new(1_099).unwrap(),
                    result: OperationResult::Scalar(declaration(quotient)),
                    kind: OperationKind::ExactIntegerDivide {
                        left: dividend,
                        right: one,
                        obligation,
                    },
                },
            ],
            terminator: Terminator::Return {
                cleanup_actions: Vec::new(),
                edge: EdgeId::new(1_100).unwrap(),
                value: quotient,
            },
        }],
    );
    module.machines[0].parameters.push(declaration(dividend));
    verified(
        module,
        ProofBundle {
            evidence_producers: Vec::new(),
            evidence: vec![ObligationEvidence {
                obligation,
                route: EvidenceRoute::KernelDerived(PrimitiveJudgment::Truth),
            }],
        },
    )
}

pub(super) fn live_exact_multiply_by_zero_verified() -> VerifiedPsiOptimizationUnit {
    let machine = MachineId::new(1_101).unwrap();
    let block = BlockId::new(1_102).unwrap();
    let value = ValueId::new(1_103).unwrap();
    let zero = ValueId::new(1_104).unwrap();
    let product = ValueId::new(1_105).unwrap();
    let result = ValueId::new(1_106).unwrap();
    let obligation = ObligationId::new(1_107).unwrap();
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap());
    let declaration = |id| ValueDeclaration { id, scalar_type };
    let mut module = module_with_blocks(
        machine,
        block,
        TerminalMachineResult::Scalar(declaration(result)),
        vec![Block {
            id: block,
            parameters: Vec::new(),
            operations: vec![
                Operation {
                    id: OperationId::new(1_108).unwrap(),
                    result: OperationResult::Scalar(declaration(zero)),
                    kind: OperationKind::IntegerConstant {
                        value: IntegerValue::Unsigned(0),
                    },
                },
                Operation {
                    id: OperationId::new(1_109).unwrap(),
                    result: OperationResult::Scalar(declaration(product)),
                    kind: OperationKind::ExactIntegerMultiply {
                        left: value,
                        right: zero,
                        obligation,
                    },
                },
            ],
            terminator: Terminator::Return {
                cleanup_actions: Vec::new(),
                edge: EdgeId::new(1_110).unwrap(),
                value: product,
            },
        }],
    );
    module.machines[0].parameters.push(declaration(value));
    verified(
        module,
        ProofBundle {
            evidence_producers: Vec::new(),
            evidence: vec![ObligationEvidence {
                obligation,
                route: EvidenceRoute::KernelDerived(PrimitiveJudgment::Truth),
            }],
        },
    )
}

pub(super) fn live_exact_zero_dividend_verified() -> VerifiedPsiOptimizationUnit {
    let machine = MachineId::new(1_111).unwrap();
    let block = BlockId::new(1_112).unwrap();
    let zero = ValueId::new(1_113).unwrap();
    let divisor = ValueId::new(1_114).unwrap();
    let quotient = ValueId::new(1_115).unwrap();
    let result = ValueId::new(1_116).unwrap();
    let obligation = ObligationId::new(1_117).unwrap();
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap());
    let declaration = |id| ValueDeclaration { id, scalar_type };
    let module = module_with_blocks(
        machine,
        block,
        TerminalMachineResult::Scalar(declaration(result)),
        vec![Block {
            id: block,
            parameters: Vec::new(),
            operations: vec![
                Operation {
                    id: OperationId::new(1_118).unwrap(),
                    result: OperationResult::Scalar(declaration(zero)),
                    kind: OperationKind::IntegerConstant {
                        value: IntegerValue::Unsigned(0),
                    },
                },
                Operation {
                    id: OperationId::new(1_119).unwrap(),
                    result: OperationResult::Scalar(declaration(divisor)),
                    kind: OperationKind::IntegerConstant {
                        value: IntegerValue::Unsigned(1),
                    },
                },
                Operation {
                    id: OperationId::new(1_120).unwrap(),
                    result: OperationResult::Scalar(declaration(quotient)),
                    kind: OperationKind::ExactIntegerDivide {
                        left: zero,
                        right: divisor,
                        obligation,
                    },
                },
            ],
            terminator: Terminator::Return {
                cleanup_actions: Vec::new(),
                edge: EdgeId::new(1_121).unwrap(),
                value: quotient,
            },
        }],
    );
    verified(
        module,
        ProofBundle {
            evidence_producers: Vec::new(),
            evidence: vec![ObligationEvidence {
                obligation,
                route: EvidenceRoute::KernelDerived(PrimitiveJudgment::Truth),
            }],
        },
    )
}

pub(super) fn live_exact_zero_value_shift_verified() -> VerifiedPsiOptimizationUnit {
    let machine = MachineId::new(1_122).unwrap();
    let block = BlockId::new(1_123).unwrap();
    let zero = ValueId::new(1_124).unwrap();
    let count = ValueId::new(1_125).unwrap();
    let shifted = ValueId::new(1_126).unwrap();
    let result = ValueId::new(1_127).unwrap();
    let obligation = ObligationId::new(1_128).unwrap();
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap());
    let count_scalar_type =
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 1).unwrap());
    let declaration = |id| ValueDeclaration { id, scalar_type };
    let count_declaration = |id| ValueDeclaration {
        id,
        scalar_type: count_scalar_type,
    };
    let module = module_with_blocks(
        machine,
        block,
        TerminalMachineResult::Scalar(declaration(result)),
        vec![Block {
            id: block,
            parameters: Vec::new(),
            operations: vec![
                Operation {
                    id: OperationId::new(1_129).unwrap(),
                    result: OperationResult::Scalar(declaration(zero)),
                    kind: OperationKind::IntegerConstant {
                        value: IntegerValue::Unsigned(0),
                    },
                },
                Operation {
                    id: OperationId::new(1_130).unwrap(),
                    result: OperationResult::Scalar(count_declaration(count)),
                    kind: OperationKind::IntegerConstant {
                        value: IntegerValue::Unsigned(1),
                    },
                },
                Operation {
                    id: OperationId::new(1_131).unwrap(),
                    result: OperationResult::Scalar(declaration(shifted)),
                    kind: OperationKind::ExactIntegerShiftRight {
                        value: zero,
                        count,
                        obligation,
                    },
                },
            ],
            terminator: Terminator::Return {
                cleanup_actions: Vec::new(),
                edge: EdgeId::new(1_132).unwrap(),
                value: shifted,
            },
        }],
    );
    verified(
        module,
        ProofBundle {
            evidence_producers: Vec::new(),
            evidence: vec![ObligationEvidence {
                obligation,
                route: EvidenceRoute::KernelDerived(PrimitiveJudgment::Truth),
            }],
        },
    )
}

pub(super) fn live_exact_signed_negative_one_shift_right_verified() -> VerifiedPsiOptimizationUnit {
    let machine = MachineId::new(1_162).unwrap();
    let block = BlockId::new(1_163).unwrap();
    let negative_one = ValueId::new(1_164).unwrap();
    let count = ValueId::new(1_165).unwrap();
    let shifted = ValueId::new(1_166).unwrap();
    let result = ValueId::new(1_167).unwrap();
    let obligation = ObligationId::new(1_168).unwrap();
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 8).unwrap());
    let count_scalar_type =
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 1).unwrap());
    let declaration = |id| ValueDeclaration { id, scalar_type };
    let count_declaration = |id| ValueDeclaration {
        id,
        scalar_type: count_scalar_type,
    };
    let module = module_with_blocks(
        machine,
        block,
        TerminalMachineResult::Scalar(declaration(result)),
        vec![Block {
            id: block,
            parameters: Vec::new(),
            operations: vec![
                Operation {
                    id: OperationId::new(1_169).unwrap(),
                    result: OperationResult::Scalar(declaration(negative_one)),
                    kind: OperationKind::IntegerConstant {
                        value: IntegerValue::Signed(-1),
                    },
                },
                Operation {
                    id: OperationId::new(1_170).unwrap(),
                    result: OperationResult::Scalar(count_declaration(count)),
                    kind: OperationKind::IntegerConstant {
                        value: IntegerValue::Unsigned(1),
                    },
                },
                Operation {
                    id: OperationId::new(1_171).unwrap(),
                    result: OperationResult::Scalar(declaration(shifted)),
                    kind: OperationKind::ExactIntegerShiftRight {
                        value: negative_one,
                        count,
                        obligation,
                    },
                },
            ],
            terminator: Terminator::Return {
                cleanup_actions: Vec::new(),
                edge: EdgeId::new(1_172).unwrap(),
                value: shifted,
            },
        }],
    );
    verified(
        module,
        ProofBundle {
            evidence_producers: Vec::new(),
            evidence: vec![ObligationEvidence {
                obligation,
                route: EvidenceRoute::KernelDerived(PrimitiveJudgment::Truth),
            }],
        },
    )
}

pub(super) fn live_exact_self_subtract_verified() -> VerifiedPsiOptimizationUnit {
    let machine = MachineId::new(1_133).unwrap();
    let block = BlockId::new(1_134).unwrap();
    let operand = ValueId::new(1_135).unwrap();
    let difference = ValueId::new(1_136).unwrap();
    let result = ValueId::new(1_137).unwrap();
    let obligation = ObligationId::new(1_138).unwrap();
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap());
    let declaration = |id| ValueDeclaration { id, scalar_type };
    let mut module = module_with_blocks(
        machine,
        block,
        TerminalMachineResult::Scalar(declaration(result)),
        vec![Block {
            id: block,
            parameters: Vec::new(),
            operations: vec![Operation {
                id: OperationId::new(1_139).unwrap(),
                result: OperationResult::Scalar(declaration(difference)),
                kind: OperationKind::ExactIntegerSubtract {
                    left: operand,
                    right: operand,
                    obligation,
                },
            }],
            terminator: Terminator::Return {
                cleanup_actions: Vec::new(),
                edge: EdgeId::new(1_140).unwrap(),
                value: difference,
            },
        }],
    );
    module.machines[0].parameters.push(declaration(operand));
    let operand_term = ScalarTerm::value(operand, scalar_type);
    let goal = Proposition::LessOrEqual(operand_term.clone(), operand_term);
    module.machines[0].contract.requires.push(goal.clone());
    verified(
        module,
        ProofBundle {
            evidence_producers: Vec::new(),
            evidence: vec![ObligationEvidence {
                obligation,
                route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                    identity: EvidenceIdentity::new(1_141).unwrap(),
                    proof_system_marker: ProofSystemMarker::CURRENT,
                    proof: ProofNode {
                        conclusion: goal,
                        rule: ProofRule::Assumption { index: 0 },
                    },
                }),
            }],
        },
    )
}

pub(super) fn live_exact_self_division_or_remainder_verified(
    divide: bool,
) -> VerifiedPsiOptimizationUnit {
    let machine = MachineId::new(1_142).unwrap();
    let block = BlockId::new(1_143).unwrap();
    let operand = ValueId::new(1_144).unwrap();
    let remainder = ValueId::new(1_145).unwrap();
    let result = ValueId::new(1_146).unwrap();
    let obligation = ObligationId::new(1_147).unwrap();
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let scalar_type = ScalarType::Integer(integer);
    let declaration = |id| ValueDeclaration { id, scalar_type };
    let mut module = module_with_blocks(
        machine,
        block,
        TerminalMachineResult::Scalar(declaration(result)),
        vec![Block {
            id: block,
            parameters: Vec::new(),
            operations: vec![Operation {
                id: OperationId::new(1_148).unwrap(),
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
                edge: EdgeId::new(1_149).unwrap(),
                value: remainder,
            },
        }],
    );
    module.machines[0].parameters.push(declaration(operand));
    let one = ScalarTerm::integer(integer, IntegerValue::Unsigned(1)).unwrap();
    let goal = Proposition::LessOrEqual(one, ScalarTerm::value(operand, scalar_type));
    module.machines[0].contract.requires.push(goal.clone());
    verified(
        module,
        ProofBundle {
            evidence_producers: Vec::new(),
            evidence: vec![ObligationEvidence {
                obligation,
                route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                    identity: EvidenceIdentity::new(1_150).unwrap(),
                    proof_system_marker: ProofSystemMarker::CURRENT,
                    proof: ProofNode {
                        conclusion: goal,
                        rule: ProofRule::Assumption { index: 0 },
                    },
                }),
            }],
        },
    )
}

pub(super) fn live_exact_self_remainder_verified() -> VerifiedPsiOptimizationUnit {
    live_exact_self_division_or_remainder_verified(false)
}

pub(super) fn live_exact_self_divide_verified() -> VerifiedPsiOptimizationUnit {
    live_exact_self_division_or_remainder_verified(true)
}

pub(super) fn live_exact_remainder_by_one_verified() -> VerifiedPsiOptimizationUnit {
    live_exact_remainder_by_unit_verified(
        IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
        IntegerValue::Unsigned(1),
    )
}

pub(super) fn live_exact_signed_remainder_by_negative_one_verified() -> VerifiedPsiOptimizationUnit
{
    live_exact_remainder_by_unit_verified(
        IntegerType::new(IntegerSign::Signed, 8).unwrap(),
        IntegerValue::Signed(-1),
    )
}

pub(super) fn live_exact_remainder_by_unit_verified(
    integer: IntegerType,
    divisor: IntegerValue,
) -> VerifiedPsiOptimizationUnit {
    let machine = MachineId::new(1_151).unwrap();
    let block = BlockId::new(1_152).unwrap();
    let operand = ValueId::new(1_153).unwrap();
    let one = ValueId::new(1_154).unwrap();
    let remainder = ValueId::new(1_155).unwrap();
    let result = ValueId::new(1_156).unwrap();
    let obligation = ObligationId::new(1_157).unwrap();
    let scalar_type = ScalarType::Integer(integer);
    let declaration = |id| ValueDeclaration { id, scalar_type };
    let constant_left = divisor == IntegerValue::Signed(-1);
    let mut operations = Vec::new();
    if constant_left {
        operations.push(Operation {
            id: OperationId::new(1_161).unwrap(),
            result: OperationResult::Scalar(declaration(operand)),
            kind: OperationKind::IntegerConstant {
                value: IntegerValue::Signed(7),
            },
        });
    }
    operations.extend([
        Operation {
            id: OperationId::new(1_158).unwrap(),
            result: OperationResult::Scalar(declaration(one)),
            kind: OperationKind::IntegerConstant { value: divisor },
        },
        Operation {
            id: OperationId::new(1_159).unwrap(),
            result: OperationResult::Scalar(declaration(remainder)),
            kind: OperationKind::ExactIntegerRemainder {
                left: operand,
                right: one,
                obligation,
            },
        },
    ]);
    let mut module = module_with_blocks(
        machine,
        block,
        TerminalMachineResult::Scalar(declaration(result)),
        vec![Block {
            id: block,
            parameters: Vec::new(),
            operations,
            terminator: Terminator::Return {
                cleanup_actions: Vec::new(),
                edge: EdgeId::new(1_160).unwrap(),
                value: remainder,
            },
        }],
    );
    if !constant_left {
        module.machines[0].parameters.push(declaration(operand));
    }
    verified(
        module,
        ProofBundle {
            evidence_producers: Vec::new(),
            evidence: vec![ObligationEvidence {
                obligation,
                route: EvidenceRoute::KernelDerived(PrimitiveJudgment::Truth),
            }],
        },
    )
}

pub(super) fn exact_add_verified_with_result(return_result: bool) -> VerifiedPsiOptimizationUnit {
    let machine = MachineId::new(1_011).unwrap();
    let block = BlockId::new(1_012).unwrap();
    let left = ValueId::new(1_013).unwrap();
    let right = ValueId::new(1_014).unwrap();
    let computed = ValueId::new(1_015).unwrap();
    let result = ValueId::new(1_016).unwrap();
    let obligation = ObligationId::new(1_017).unwrap();
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap());
    let declaration = |id| ValueDeclaration { id, scalar_type };
    let machine_result = if return_result {
        TerminalMachineResult::Scalar(declaration(result))
    } else {
        TerminalMachineResult::Unit
    };
    let terminator = if return_result {
        Terminator::Return {
            cleanup_actions: Vec::new(),
            edge: EdgeId::new(1_021).unwrap(),
            value: computed,
        }
    } else {
        Terminator::ReturnUnit {
            edge: EdgeId::new(1_021).unwrap(),
            trivial_affine_discards: Vec::new(),
        }
    };
    verified(
        module_with_blocks(
            machine,
            block,
            machine_result,
            vec![Block {
                id: block,
                parameters: Vec::new(),
                operations: vec![
                    Operation {
                        id: OperationId::new(1_018).unwrap(),
                        result: OperationResult::Scalar(declaration(left)),
                        kind: OperationKind::IntegerConstant {
                            value: IntegerValue::Unsigned(7),
                        },
                    },
                    Operation {
                        id: OperationId::new(1_019).unwrap(),
                        result: OperationResult::Scalar(declaration(right)),
                        kind: OperationKind::IntegerConstant {
                            value: IntegerValue::Unsigned(8),
                        },
                    },
                    Operation {
                        id: OperationId::new(1_020).unwrap(),
                        result: OperationResult::Scalar(declaration(computed)),
                        kind: OperationKind::ExactIntegerAdd {
                            left,
                            right,
                            obligation,
                        },
                    },
                ],
                terminator,
            }],
        ),
        ProofBundle {
            evidence_producers: Vec::new(),
            evidence: vec![ObligationEvidence {
                obligation,
                route: EvidenceRoute::KernelDerived(PrimitiveJudgment::Truth),
            }],
        },
    )
}
