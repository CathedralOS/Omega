use super::*;

pub(super) fn local_cse_verified() -> VerifiedPsiOptimizationUnit {
    let machine = MachineId::new(1_321).unwrap();
    let block = BlockId::new(1_322).unwrap();
    let operand = ValueId::new(1_323).unwrap();
    let leader = ValueId::new(1_324).unwrap();
    let redundant = ValueId::new(1_325).unwrap();
    let result = ValueId::new(1_326).unwrap();
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
                    id: OperationId::new(1_327).unwrap(),
                    result: OperationResult::Scalar(declaration(leader)),
                    kind: OperationKind::IntegerBitwiseNot { operand },
                },
                Operation {
                    id: OperationId::new(1_328).unwrap(),
                    result: OperationResult::Scalar(declaration(redundant)),
                    kind: OperationKind::IntegerBitwiseNot { operand },
                },
            ],
            terminator: Terminator::Return {
                edge: EdgeId::new(1_329).unwrap(),
                value: redundant,
                cleanup_actions: Vec::new(),
            },
        }],
    );
    module.machines[0].parameters.push(declaration(operand));
    verified(module, ProofBundle::default())
}

pub(super) fn proof_certified_local_cse_verified() -> VerifiedPsiOptimizationUnit {
    let machine = MachineId::new(1_381).unwrap();
    let block = BlockId::new(1_382).unwrap();
    let left = ValueId::new(1_383).unwrap();
    let right = ValueId::new(1_384).unwrap();
    let leader = ValueId::new(1_385).unwrap();
    let redundant = ValueId::new(1_386).unwrap();
    let result = ValueId::new(1_387).unwrap();
    let leader_operation = OperationId::new(1_388).unwrap();
    let redundant_operation = OperationId::new(1_389).unwrap();
    let leader_obligation = ObligationId::new(1_390).unwrap();
    let redundant_obligation = ObligationId::new(1_391).unwrap();
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
                    id: OperationId::new(1_393).unwrap(),
                    result: OperationResult::Scalar(declaration(left)),
                    kind: OperationKind::IntegerConstant {
                        value: IntegerValue::Unsigned(7),
                    },
                },
                Operation {
                    id: OperationId::new(1_394).unwrap(),
                    result: OperationResult::Scalar(declaration(right)),
                    kind: OperationKind::IntegerConstant {
                        value: IntegerValue::Unsigned(8),
                    },
                },
                Operation {
                    id: leader_operation,
                    result: OperationResult::Scalar(declaration(leader)),
                    kind: OperationKind::ExactIntegerAdd {
                        left,
                        right,
                        obligation: leader_obligation,
                    },
                },
                Operation {
                    id: redundant_operation,
                    result: OperationResult::Scalar(declaration(redundant)),
                    kind: OperationKind::ExactIntegerAdd {
                        left: right,
                        right: left,
                        obligation: redundant_obligation,
                    },
                },
            ],
            terminator: Terminator::Return {
                edge: EdgeId::new(1_392).unwrap(),
                value: redundant,
                cleanup_actions: Vec::new(),
            },
        }],
    );
    verified(
        module,
        ProofBundle {
            evidence_producers: Vec::new(),
            evidence: vec![
                ObligationEvidence {
                    obligation: leader_obligation,
                    route: EvidenceRoute::KernelDerived(PrimitiveJudgment::Truth),
                },
                ObligationEvidence {
                    obligation: redundant_obligation,
                    route: EvidenceRoute::KernelDerived(PrimitiveJudgment::Truth),
                },
            ],
        },
    )
}

pub(super) fn compatible_policy_local_cse_verified() -> VerifiedPsiOptimizationUnit {
    let machine = MachineId::new(1_401).unwrap();
    let block = BlockId::new(1_402).unwrap();
    let left = ValueId::new(1_403).unwrap();
    let right = ValueId::new(1_404).unwrap();
    let leader = ValueId::new(1_405).unwrap();
    let redundant = ValueId::new(1_406).unwrap();
    let result = ValueId::new(1_407).unwrap();
    let redundant_operation = OperationId::new(1_409).unwrap();
    let redundant_obligation = ObligationId::new(1_411).unwrap();
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
                    id: OperationId::new(1_413).unwrap(),
                    result: OperationResult::Scalar(declaration(left)),
                    kind: OperationKind::IntegerConstant {
                        value: IntegerValue::Unsigned(7),
                    },
                },
                Operation {
                    id: OperationId::new(1_414).unwrap(),
                    result: OperationResult::Scalar(declaration(right)),
                    kind: OperationKind::IntegerConstant {
                        value: IntegerValue::Unsigned(8),
                    },
                },
                Operation {
                    id: OperationId::new(1_408).unwrap(),
                    result: OperationResult::Scalar(declaration(leader)),
                    kind: OperationKind::SaturatingIntegerAdd { left, right },
                },
                Operation {
                    id: redundant_operation,
                    result: OperationResult::Scalar(declaration(redundant)),
                    kind: OperationKind::ExactIntegerAdd {
                        left: right,
                        right: left,
                        obligation: redundant_obligation,
                    },
                },
            ],
            terminator: Terminator::Return {
                edge: EdgeId::new(1_412).unwrap(),
                value: redundant,
                cleanup_actions: Vec::new(),
            },
        }],
    );
    verified(
        module,
        ProofBundle {
            evidence_producers: Vec::new(),
            evidence: vec![ObligationEvidence {
                obligation: redundant_obligation,
                route: EvidenceRoute::KernelDerived(PrimitiveJudgment::Truth),
            }],
        },
    )
}

pub(super) fn dominator_gvn_verified() -> VerifiedPsiOptimizationUnit {
    let machine = MachineId::new(1_361).unwrap();
    let child = BlockId::new(1_362).unwrap();
    let entry = BlockId::new(1_363).unwrap();
    let operand = ValueId::new(1_364).unwrap();
    let leader = ValueId::new(1_365).unwrap();
    let redundant = ValueId::new(1_366).unwrap();
    let result = ValueId::new(1_367).unwrap();
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap());
    let declaration = |id| ValueDeclaration { id, scalar_type };
    let mut module = module_with_blocks(
        machine,
        entry,
        TerminalMachineResult::Scalar(declaration(result)),
        vec![
            Block {
                id: child,
                parameters: Vec::new(),
                operations: vec![Operation {
                    id: OperationId::new(1_368).unwrap(),
                    result: OperationResult::Scalar(declaration(redundant)),
                    kind: OperationKind::IntegerBitwiseNot { operand },
                }],
                terminator: Terminator::Return {
                    edge: EdgeId::new(1_369).unwrap(),
                    value: redundant,
                    cleanup_actions: Vec::new(),
                },
            },
            Block {
                id: entry,
                parameters: Vec::new(),
                operations: vec![Operation {
                    id: OperationId::new(1_370).unwrap(),
                    result: OperationResult::Scalar(declaration(leader)),
                    kind: OperationKind::IntegerBitwiseNot { operand },
                }],
                terminator: Terminator::Jump {
                    edge: EdgeId::new(1_371).unwrap(),
                    target: child,
                    arguments: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                },
            },
        ],
    );
    module.machines[0].parameters.push(declaration(operand));
    verified(module, ProofBundle::default())
}

pub(super) fn phi_translated_gvn_verified() -> VerifiedPsiOptimizationUnit {
    phi_translated_gvn_verified_fixture(false, false)
}

pub(super) fn proof_certified_phi_translated_gvn_verified() -> VerifiedPsiOptimizationUnit {
    phi_translated_gvn_verified_fixture(true, false)
}

pub(super) fn compatible_policy_phi_translated_gvn_verified() -> VerifiedPsiOptimizationUnit {
    phi_translated_gvn_verified_fixture(false, true)
}

pub(super) fn phi_translated_gvn_verified_fixture(
    proof_certified: bool,
    compatible_policy: bool,
) -> VerifiedPsiOptimizationUnit {
    let machine = MachineId::new(1_451).unwrap();
    let join = BlockId::new(1_452).unwrap();
    let left = BlockId::new(1_453).unwrap();
    let entry = BlockId::new(1_454).unwrap();
    let right = BlockId::new(1_455).unwrap();
    let condition = ValueId::new(1_456).unwrap();
    let left_input = ValueId::new(1_457).unwrap();
    let right_input = ValueId::new(1_458).unwrap();
    let join_input = ValueId::new(1_459).unwrap();
    let left_leader = ValueId::new(1_460).unwrap();
    let right_leader = ValueId::new(1_461).unwrap();
    let redundant = ValueId::new(1_462).unwrap();
    let result = ValueId::new(1_471).unwrap();
    let zero = ValueId::new(1_475).unwrap();
    let redundant_obligation = ObligationId::new(1_472).unwrap();
    let left_obligation = ObligationId::new(1_473).unwrap();
    let right_obligation = ObligationId::new(1_474).unwrap();
    let integer = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap());
    let result_integer = integer;
    let declaration = |id, scalar_type| ValueDeclaration { id, scalar_type };
    let mut module = module_with_blocks(
        machine,
        entry,
        TerminalMachineResult::Scalar(declaration(result, result_integer)),
        vec![
            Block {
                id: join,
                parameters: vec![declaration(join_input, integer)],
                operations: vec![Operation {
                    id: OperationId::new(1_463).unwrap(),
                    result: OperationResult::Scalar(declaration(redundant, result_integer)),
                    kind: if proof_certified || compatible_policy {
                        OperationKind::ExactIntegerShiftLeft {
                            value: join_input,
                            count: zero,
                            obligation: redundant_obligation,
                        }
                    } else {
                        OperationKind::IntegerBitwiseNot {
                            operand: join_input,
                        }
                    },
                }],
                terminator: Terminator::Return {
                    edge: EdgeId::new(1_464).unwrap(),
                    value: redundant,
                    cleanup_actions: Vec::new(),
                },
            },
            Block {
                id: left,
                parameters: Vec::new(),
                operations: vec![Operation {
                    id: OperationId::new(1_465).unwrap(),
                    result: OperationResult::Scalar(declaration(left_leader, result_integer)),
                    kind: if proof_certified {
                        OperationKind::ExactIntegerShiftLeft {
                            value: left_input,
                            count: zero,
                            obligation: left_obligation,
                        }
                    } else if compatible_policy {
                        OperationKind::WrappingIntegerShiftLeft {
                            value: left_input,
                            count: zero,
                        }
                    } else {
                        OperationKind::IntegerBitwiseNot {
                            operand: left_input,
                        }
                    },
                }],
                terminator: Terminator::Jump {
                    edge: EdgeId::new(1_466).unwrap(),
                    target: join,
                    arguments: vec![left_input],
                    trivial_affine_discards: Vec::new(),
                },
            },
            Block {
                id: entry,
                parameters: Vec::new(),
                operations: if proof_certified || compatible_policy {
                    vec![Operation {
                        id: OperationId::new(1_476).unwrap(),
                        result: OperationResult::Scalar(declaration(zero, integer)),
                        kind: OperationKind::IntegerConstant {
                            value: IntegerValue::Unsigned(0),
                        },
                    }]
                } else {
                    Vec::new()
                },
                terminator: Terminator::Conditional {
                    condition,
                    when_true: SuccessorEdge {
                        edge: EdgeId::new(1_467).unwrap(),
                        target: left,
                        arguments: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                    when_false: SuccessorEdge {
                        edge: EdgeId::new(1_468).unwrap(),
                        target: right,
                        arguments: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                },
            },
            Block {
                id: right,
                parameters: Vec::new(),
                operations: vec![Operation {
                    id: OperationId::new(1_469).unwrap(),
                    result: OperationResult::Scalar(declaration(right_leader, result_integer)),
                    kind: if proof_certified {
                        OperationKind::ExactIntegerShiftLeft {
                            value: right_input,
                            count: zero,
                            obligation: right_obligation,
                        }
                    } else if compatible_policy {
                        OperationKind::WrappingIntegerShiftLeft {
                            value: right_input,
                            count: zero,
                        }
                    } else {
                        OperationKind::IntegerBitwiseNot {
                            operand: right_input,
                        }
                    },
                }],
                terminator: Terminator::Jump {
                    edge: EdgeId::new(1_470).unwrap(),
                    target: join,
                    arguments: vec![right_input],
                    trivial_affine_discards: Vec::new(),
                },
            },
        ],
    );
    module.machines[0].parameters.extend([
        declaration(condition, ScalarType::Boolean),
        declaration(left_input, integer),
        declaration(right_input, integer),
    ]);
    let proof_bundle = if proof_certified {
        ProofBundle {
            evidence_producers: Vec::new(),
            evidence: [redundant_obligation, left_obligation, right_obligation]
                .into_iter()
                .map(|obligation| ObligationEvidence {
                    obligation,
                    route: EvidenceRoute::KernelDerived(PrimitiveJudgment::Truth),
                })
                .collect(),
        }
    } else if compatible_policy {
        ProofBundle {
            evidence_producers: Vec::new(),
            evidence: vec![ObligationEvidence {
                obligation: redundant_obligation,
                route: EvidenceRoute::KernelDerived(PrimitiveJudgment::Truth),
            }],
        }
    } else {
        ProofBundle::default()
    };
    verified(module, proof_bundle)
}
