//! Control-flow projection fixtures.

use super::*;

pub(super) fn unreachable_private_machine_verified() -> VerifiedPsiOptimizationUnit {
    let entry_machine = MachineId::new(1_041).unwrap();
    let entry_block = BlockId::new(1_042).unwrap();
    let mut module = module_with_blocks(
        entry_machine,
        entry_block,
        TerminalMachineResult::Unit,
        vec![Block {
            id: entry_block,
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::ReturnUnit {
                edge: EdgeId::new(1_043).unwrap(),
                trivial_affine_discards: Vec::new(),
            },
        }],
    );
    let private_machine = MachineId::new(1_044).unwrap();
    let private_block = BlockId::new(1_045).unwrap();
    module.machines.push(TerminalMachine {
        id: private_machine,
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
        entry: private_block,
        blocks: vec![Block {
            id: private_block,
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::ReturnUnit {
                edge: EdgeId::new(1_046).unwrap(),
                trivial_affine_discards: Vec::new(),
            },
        }],
        contract: MachineContract {
            id: ContractId::new(1_047).unwrap(),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    });
    verified(module, ProofBundle::default())
}

pub(super) fn adjacent_terminal_jump_verified() -> VerifiedPsiOptimizationUnit {
    let machine = MachineId::new(1_051).unwrap();
    let entry = BlockId::new(1_052).unwrap();
    let target = BlockId::new(1_053).unwrap();
    verified(
        module_with_blocks(
            machine,
            entry,
            TerminalMachineResult::Unit,
            vec![
                Block {
                    id: entry,
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::Jump {
                        edge: EdgeId::new(1_054).unwrap(),
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
                        edge: EdgeId::new(1_055).unwrap(),
                        trivial_affine_discards: Vec::new(),
                    },
                },
            ],
        ),
        ProofBundle::default(),
    )
}

pub(super) fn non_adjacent_block_merge_verified() -> VerifiedPsiOptimizationUnit {
    let machine = MachineId::new(1_501).unwrap();
    let entry = BlockId::new(1_502).unwrap();
    let descendant = BlockId::new(1_503).unwrap();
    let target = BlockId::new(1_504).unwrap();
    let sibling = BlockId::new(1_505).unwrap();
    let predecessor = BlockId::new(1_506).unwrap();
    let condition = ValueId::new(1_507).unwrap();
    let incoming = ValueId::new(1_508).unwrap();
    let target_parameter = ValueId::new(1_509).unwrap();
    let target_result = ValueId::new(1_510).unwrap();
    let computed = ValueId::new(1_511).unwrap();
    let predecessor_value = ValueId::new(1_520).unwrap();
    let result = ValueId::new(1_522).unwrap();
    let boolean = |id| ValueDeclaration {
        id,
        scalar_type: ScalarType::Boolean,
    };
    let mut module = module_with_blocks(
        machine,
        entry,
        TerminalMachineResult::Scalar(boolean(result)),
        vec![
            Block {
                id: entry,
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: Terminator::Conditional {
                    condition,
                    when_true: SuccessorEdge {
                        edge: EdgeId::new(1_512).unwrap(),
                        target: predecessor,
                        arguments: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                    when_false: SuccessorEdge {
                        edge: EdgeId::new(1_513).unwrap(),
                        target: sibling,
                        arguments: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                },
            },
            Block {
                id: descendant,
                parameters: Vec::new(),
                operations: vec![Operation {
                    id: OperationId::new(1_514).unwrap(),
                    result: OperationResult::Scalar(boolean(computed)),
                    kind: OperationKind::BooleanEqual {
                        left: target_parameter,
                        right: target_result,
                    },
                }],
                terminator: Terminator::Return {
                    edge: EdgeId::new(1_515).unwrap(),
                    value: computed,
                    cleanup_actions: Vec::new(),
                },
            },
            Block {
                id: target,
                parameters: vec![boolean(target_parameter)],
                operations: vec![Operation {
                    id: OperationId::new(1_516).unwrap(),
                    result: OperationResult::Scalar(boolean(target_result)),
                    kind: OperationKind::BooleanNot {
                        operand: target_parameter,
                    },
                }],
                terminator: Terminator::Jump {
                    edge: EdgeId::new(1_517).unwrap(),
                    target: descendant,
                    arguments: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                },
            },
            Block {
                id: sibling,
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: Terminator::Return {
                    edge: EdgeId::new(1_518).unwrap(),
                    value: incoming,
                    cleanup_actions: Vec::new(),
                },
            },
            Block {
                id: predecessor,
                parameters: Vec::new(),
                operations: vec![Operation {
                    id: OperationId::new(1_521).unwrap(),
                    result: OperationResult::Scalar(boolean(predecessor_value)),
                    kind: OperationKind::BooleanNot { operand: incoming },
                }],
                terminator: Terminator::Jump {
                    edge: EdgeId::new(1_519).unwrap(),
                    target,
                    arguments: vec![predecessor_value],
                    trivial_affine_discards: Vec::new(),
                },
            },
        ],
    );
    module.machines[0]
        .parameters
        .extend([boolean(condition), boolean(incoming)]);
    verified(module, ProofBundle::default())
}

pub(super) fn shared_terminal_jump_verified() -> VerifiedPsiOptimizationUnit {
    let machine = MachineId::new(1_061).unwrap();
    let entry = BlockId::new(1_062).unwrap();
    let left = BlockId::new(1_063).unwrap();
    let right = BlockId::new(1_064).unwrap();
    let target = BlockId::new(1_065).unwrap();
    let condition = ValueId::new(1_066).unwrap();
    let left_value = ValueId::new(1_067).unwrap();
    let right_value = ValueId::new(1_068).unwrap();
    let boolean = |id| ValueDeclaration {
        id,
        scalar_type: ScalarType::Boolean,
    };
    let mut module = module_with_blocks(
        machine,
        entry,
        TerminalMachineResult::Unit,
        vec![
            Block {
                id: entry,
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: Terminator::Conditional {
                    condition,
                    when_true: SuccessorEdge {
                        edge: EdgeId::new(1_069).unwrap(),
                        target: left,
                        arguments: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                    when_false: SuccessorEdge {
                        edge: EdgeId::new(1_070).unwrap(),
                        target: right,
                        arguments: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                },
            },
            Block {
                id: left,
                parameters: Vec::new(),
                operations: vec![Operation {
                    id: OperationId::new(1_071).unwrap(),
                    result: OperationResult::Scalar(boolean(left_value)),
                    kind: OperationKind::BooleanConstant { value: true },
                }],
                terminator: Terminator::Jump {
                    edge: EdgeId::new(1_072).unwrap(),
                    target,
                    arguments: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                },
            },
            Block {
                id: right,
                parameters: Vec::new(),
                operations: vec![Operation {
                    id: OperationId::new(1_073).unwrap(),
                    result: OperationResult::Scalar(boolean(right_value)),
                    kind: OperationKind::BooleanConstant { value: false },
                }],
                terminator: Terminator::Jump {
                    edge: EdgeId::new(1_074).unwrap(),
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
                    edge: EdgeId::new(1_075).unwrap(),
                    trivial_affine_discards: Vec::new(),
                },
            },
        ],
    );
    module.machines[0].parameters.push(boolean(condition));
    verified(module, ProofBundle::default())
}
