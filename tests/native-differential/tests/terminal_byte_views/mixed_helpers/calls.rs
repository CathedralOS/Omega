//! Composed callers retain the guarded reader and its canonical obligation unchanged.

use super::super::fixtures;
use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, ObligationId,
    OperationId, PlaceId, ScalarType, ValueId,
};
use terminal_psi::{
    Block, Operation, OperationKind, OperationResult, StructuralAccess, StructuralArgument,
    SuccessorEdge, TerminalMachine, TerminalMachineResult, TerminalModule, Terminator,
    ValueDeclaration,
};

fn value(identity: u64) -> ValueId {
    ValueId::new(identity).unwrap()
}

fn scalar(identity: u64) -> ValueDeclaration {
    ValueDeclaration {
        qualifications: Default::default(),
        id: value(identity),
        scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
    }
}

fn operation(identity: u64, result: u64, kind: OperationKind) -> Operation {
    Operation {
        static_reach_binding: None,
        id: OperationId::new(identity).unwrap(),
        result: OperationResult::Scalar(scalar(result)),
        kind,
    }
}

fn returned(edge: u64, result: u64) -> Terminator {
    Terminator::Return {
        edge: EdgeId::new(edge).unwrap(),
        value: value(result),
        cleanup_actions: Vec::new(),
    }
}

fn call(
    identity: u64,
    result: u64,
    callee: MachineId,
    arguments: &[u64],
    source: PlaceId,
) -> Operation {
    operation(
        identity,
        result,
        OperationKind::CallStructuralScalar {
            callee,
            arguments: arguments.iter().copied().map(value).collect(),
            structural_arguments: vec![StructuralArgument {
                place: source,
                path: Vec::new(),
                access: StructuralAccess::SharedBorrow,
            }],
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    )
}

fn caller(identity: u64, callee: MachineId, parameter_count: u64) -> TerminalMachine {
    let mut caller = fixtures::byte_view_length_module().machines.remove(0);
    caller.id = MachineId::new(identity).unwrap();
    caller.contract.id = ContractId::new(identity + 9).unwrap();
    caller.entry = BlockId::new(identity + 2).unwrap();
    caller.blocks[0].id = caller.entry;
    let source = PlaceId::new(identity + 3).unwrap();
    caller.structural_parameters[0].place = source;
    caller.structural_places[0].id = source;
    caller.result = TerminalMachineResult::Scalar(scalar(identity + 6));
    caller.parameters = (0..parameter_count)
        .map(|position| scalar(identity + 10 + position))
        .collect();
    caller.blocks[0].operations = vec![call(
        identity + 7,
        identity + 5,
        callee,
        &[identity + 10],
        source,
    )];
    caller.blocks[0].terminator = returned(identity + 8, identity + 5);
    caller
}

pub(super) fn composed() -> TerminalModule {
    let mut module = fixtures::byte_view_read_module();
    let mut inner = caller(100, module.entry, 2);
    let source = inner.structural_parameters[0].place;
    inner.blocks[0].operations.extend([
        // The execution roster must retain this call even though its value is unused.
        call(177, 175, module.entry, &[111], source),
        call(117, 115, module.entry, &[111], source),
        call(127, 125, module.entry, &[105], source),
        operation(
            137,
            135,
            OperationKind::ExactIntegerAdd {
                left: value(105),
                right: value(115),
                obligation: ObligationId::new(137).unwrap(),
            },
        ),
        operation(
            147,
            145,
            OperationKind::ExactIntegerAdd {
                left: value(135),
                right: value(125),
                obligation: ObligationId::new(147).unwrap(),
            },
        ),
    ]);
    inner.blocks[0].terminator = returned(108, 145);
    guard_exact_sums(&mut inner);
    let mut outer = caller(200, inner.id, 2);
    // Swapping scalar inputs requires a parallel ABI transfer. The inner reader
    // also moves the original descriptor from the third to the second ABI input.
    outer.blocks[0].operations = vec![call(
        207,
        205,
        inner.id,
        &[211, 210],
        outer.structural_parameters[0].place,
    )];
    module.entry = outer.id;
    module.machines.extend([inner, outer]);
    module
}

pub(super) fn conditional() -> TerminalModule {
    let mut module = fixtures::byte_view_read_module();
    let mut caller = caller(100, module.entry, 2);
    let source = caller.structural_parameters[0].place;
    caller.blocks[0].operations = vec![
        operation(
            117,
            115,
            OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(0),
            },
        ),
        Operation {
            static_reach_binding: None,
            id: OperationId::new(127).unwrap(),
            result: OperationResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: value(125),
                scalar_type: ScalarType::Boolean,
            }),
            kind: OperationKind::IntegerEqual {
                left: value(110),
                right: value(115),
            },
        },
    ];
    let successor = |edge, block| SuccessorEdge {
        edge: EdgeId::new(edge).unwrap(),
        target: BlockId::new(block).unwrap(),
        arguments: Vec::new(),
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    caller.blocks[0].terminator = Terminator::Conditional {
        condition: value(125),
        when_true: successor(128, 132),
        when_false: successor(138, 142),
    };
    caller.blocks.extend([
        Block {
            id: BlockId::new(132).unwrap(),
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            operations: vec![
                call(137, 135, module.entry, &[111], source),
                call(147, 145, module.entry, &[135], source),
                operation(
                    157,
                    155,
                    OperationKind::ExactIntegerAdd {
                        left: value(135),
                        right: value(145),
                        obligation: ObligationId::new(157).unwrap(),
                    },
                ),
            ],
            terminator: returned(148, 155),
        },
        Block {
            id: BlockId::new(142).unwrap(),
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            operations: vec![operation(
                167,
                165,
                OperationKind::IntegerConstant {
                    value: IntegerValue::Unsigned(777),
                },
            )],
            terminator: returned(158, 165),
        },
    ]);
    guard_exact_sums(&mut caller);
    module.entry = caller.id;
    module.machines.push(caller);
    module
}

fn successor(edge: u64, block: u64) -> SuccessorEdge {
    SuccessorEdge {
        edge: EdgeId::new(edge).unwrap(),
        target: BlockId::new(block).unwrap(),
        arguments: Vec::new(),
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
    }
}

fn empty_block(identity: u64) -> Block {
    Block {
        id: BlockId::new(identity).unwrap(),
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        operations: Vec::new(),
        terminator: returned(9002, 9001),
    }
}

fn bounded_operand(identity: u64, operand: ValueId, maximum: ValueId) -> Operation {
    Operation {
        static_reach_binding: None,
        id: OperationId::new(identity).unwrap(),
        result: OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: value(identity),
            scalar_type: ScalarType::Boolean,
        }),
        kind: OperationKind::IntegerLessOrEqual {
            left: operand,
            right: maximum,
        },
    }
}

/// Guard each exact sum at its authored position without dropping calls or moving
/// operations out of their selected arm. The fallback is unreachable for byte results.
fn guard_exact_sums(machine: &mut TerminalMachine) {
    let mut identity = 1000;
    for mut block in std::mem::take(&mut machine.blocks) {
        let terminator = block.terminator.clone();
        for next in std::mem::take(&mut block.operations) {
            if let OperationKind::ExactIntegerAdd { left, right, .. } = &next.kind {
                block.operations.push(operation(
                    identity,
                    identity,
                    OperationKind::IntegerConstant {
                        value: IntegerValue::Unsigned(1024),
                    },
                ));
                block
                    .operations
                    .push(bounded_operand(identity + 1, *left, value(identity)));
                block.terminator = Terminator::Conditional {
                    condition: value(identity + 1),
                    when_true: successor(identity + 5, identity + 2),
                    when_false: successor(identity + 6, 9000),
                };
                machine.blocks.push(block);
                block = empty_block(identity + 2);
                block
                    .operations
                    .push(bounded_operand(identity + 3, *right, value(identity)));
                block.terminator = Terminator::Conditional {
                    condition: value(identity + 3),
                    when_true: successor(identity + 7, identity + 4),
                    when_false: successor(identity + 8, 9000),
                };
                machine.blocks.push(block);
                block = empty_block(identity + 4);
                identity += 10;
            }
            block.operations.push(next);
        }
        block.terminator = terminator;
        machine.blocks.push(block);
    }
    let mut rejected = empty_block(9000);
    rejected.operations.push(operation(
        9001,
        9001,
        OperationKind::IntegerConstant {
            value: IntegerValue::Unsigned(9999),
        },
    ));
    machine.blocks.push(rejected);
    machine.blocks.sort_by_key(|block| block.id);
}
