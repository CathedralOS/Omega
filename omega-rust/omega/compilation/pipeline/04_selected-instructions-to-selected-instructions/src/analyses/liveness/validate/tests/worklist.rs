//! Exact replay remains a full-scan reference for producer work reduction.

use super::replay_function;
use crate::analyses::liveness::{compute, edge_values, tests::successor_parameter_function};
use register_model::RegisterUnitId;
use selected_instructions::{
    SelectedBlock, SelectedBlockId, SelectedBlockOrigin, SelectedFunction, SelectedInstructionId,
    SelectedInstructionKind, SelectedSuccessor, SelectedSuccessorRole, SelectedTerminator,
    VirtualRegisterId,
};
use semantic_vocabulary::{BlockId, EdgeId};

fn graph(targets: &[Vec<usize>]) -> SelectedFunction {
    let mut function = successor_parameter_function();
    let SelectedTerminator::Jump { instruction, .. } = &function.blocks[0].terminator else {
        unreachable!()
    };
    let template = instruction.clone();
    let SelectedTerminator::Return { instruction, .. } = &function.blocks[1].terminator else {
        unreachable!()
    };
    let mut used = instruction.operands[0];
    used.virtual_register = VirtualRegisterId(0);
    function
        .virtual_registers
        .retain(|register| register.id == VirtualRegisterId(0));
    let block_id = |ordinal: usize| SelectedBlockId(7 + 11 * ordinal as u32);
    let source_id = |ordinal: usize| BlockId::new(ordinal as u64 + 1).unwrap();
    function.entry_block = block_id(0);
    function.blocks = targets
        .iter()
        .enumerate()
        .map(|(block_index, targets)| {
            let mut instruction = template.clone();
            instruction.id = SelectedInstructionId(block_index as u32);
            let successor = |target_index: usize| SelectedSuccessor {
                role: SelectedSuccessorRole::Semantic,
                structural_case: None,
                structural_bindings: Vec::new(),
                psi_edge: EdgeId::new(block_index as u64 + 1).unwrap(),
                block: block_id(target_index),
                source_target: source_id(target_index),
                bindings: Vec::new(),
                fuel: Vec::new(),
            };
            let terminator = match targets.as_slice() {
                [] => {
                    instruction.kind = SelectedInstructionKind::ReturnScalar;
                    instruction.operands.push(used);
                    instruction.implicit_uses.push(RegisterUnitId(99));
                    SelectedTerminator::Return {
                        instruction,
                        psi_return_edge: EdgeId::new(block_index as u64 + 1).unwrap(),
                    }
                }
                [target] => SelectedTerminator::Jump {
                    instruction,
                    successor: successor(*target),
                },
                [left, right] => {
                    instruction.kind = SelectedInstructionKind::CompareI64Zero;
                    SelectedTerminator::ConditionalBranch {
                        instruction,
                        when_nonzero: successor(*left),
                        when_zero: successor(*right),
                    }
                }
                _ => unreachable!(),
            };
            SelectedBlock {
                id: block_id(block_index),
                origin: SelectedBlockOrigin::Source(source_id(block_index)),
                instructions: Vec::new(),
                terminator,
            }
        })
        .collect();
    function
}

fn compare(function: &SelectedFunction) -> (usize, usize, usize, usize) {
    compute::BLOCK_VISITS.set(0);
    edge_values::INCOMING_ARGUMENT_RECONSTRUCTIONS.set(0);
    let actual = compute::compute_function(0, function).unwrap();
    let producer_visits = compute::BLOCK_VISITS.get();
    let producer_transports = edge_values::INCOMING_ARGUMENT_RECONSTRUCTIONS.get();
    super::super::replay::BLOCK_VISITS.set(0);
    edge_values::INCOMING_ARGUMENT_RECONSTRUCTIONS.set(0);
    let expected = replay_function(0, function).unwrap();
    assert_eq!(actual, expected);
    assert_eq!(
        edge_values::INCOMING_ARGUMENT_RECONSTRUCTIONS.get(),
        producer_transports,
        "independent replay validates transports once, not on every fixed-point visit"
    );
    (
        producer_visits,
        super::super::replay::BLOCK_VISITS.get(),
        producer_transports,
        edge_values::INCOMING_ARGUMENT_RECONSTRUCTIONS.get(),
    )
}

#[test]
fn prepared_replay_preserves_sparse_live_through_and_unused_duplicate_registers() {
    let mut function = graph(&[vec![1], vec![2], vec![]]);
    let register = VirtualRegisterId(u32::MAX - 1);
    function.virtual_registers[0].id = register;
    for block in &mut function.blocks {
        if let SelectedTerminator::Return { instruction, .. } = &mut block.terminator {
            instruction.operands[0].virtual_register = register;
        }
    }
    // An unqueried duplicate must not acquire a new eager rejection boundary.
    let mut unused = function.virtual_registers[0].clone();
    unused.id = VirtualRegisterId(17);
    function.virtual_registers.extend([unused.clone(), unused]);
    function.virtual_registers.reverse();
    function.blocks.reverse();
    let (_, replay_visits, _, replay_transports) = compare(&function);
    assert!(replay_visits > function.blocks.len());
    assert_eq!(replay_transports, 0);
}

#[test]
fn prepared_replay_keeps_distinct_arguments_on_edges_to_the_same_block() {
    let mut function = successor_parameter_function();
    let SelectedTerminator::Jump {
        instruction,
        successor,
    } = &function.blocks[0].terminator
    else {
        unreachable!()
    };
    let mut branch = instruction.clone();
    branch.kind = SelectedInstructionKind::CompareI64Zero;
    let when_nonzero = successor.clone();
    let mut when_zero = successor.clone();
    when_zero.psi_edge = EdgeId::new(7).unwrap();
    when_zero.bindings[0].semantic.argument = semantic_vocabulary::ValueId::new(2).unwrap();
    when_zero.bindings[0].transport = selected_instructions::SelectedValueTransport::Registers {
        argument: VirtualRegisterId(1),
        parameter: VirtualRegisterId(2),
    };
    function.blocks[0].terminator = SelectedTerminator::ConditionalBranch {
        instruction: branch,
        when_nonzero,
        when_zero,
    };
    compare(&function);
    let replayed = replay_function(0, &function).unwrap();
    assert_eq!(
        replayed.blocks[0].successors[0].virtual_live,
        [VirtualRegisterId(0)]
    );
    assert_eq!(
        replayed.blocks[0].successors[1].virtual_live,
        [VirtualRegisterId(1)]
    );
}

#[test]
fn prepared_transports_preserve_parameter_and_duplicate_register_rejection() {
    let mut function = successor_parameter_function();
    compare(&function);
    function.blocks.reverse();
    compare(&function);
    for source in [function, graph(&[vec![1], vec![]])] {
        for mutation in 0..2 {
            let mut malformed = source.clone();
            if mutation == 0 {
                malformed
                    .virtual_registers
                    .push(malformed.virtual_registers[0].clone());
            } else {
                malformed.virtual_registers.remove(0);
            }
            assert_eq!(
                compute::compute_function(0, &malformed),
                replay_function(0, &malformed)
            );
            assert!(compute::compute_function(0, &malformed).is_err());
        }
    }
}

#[test]
fn prepared_transports_preserve_loop_parameter_substitution() {
    let mut function = successor_parameter_function();
    let SelectedTerminator::Jump { successor, .. } = &function.blocks[0].terminator else {
        unreachable!()
    };
    let mut repeated = successor.clone();
    repeated.psi_edge = EdgeId::new(3).unwrap();
    repeated.bindings[0].semantic.argument = repeated.bindings[0].semantic.parameter;
    repeated.bindings[0].transport = selected_instructions::SelectedValueTransport::Registers {
        argument: VirtualRegisterId(2),
        parameter: VirtualRegisterId(2),
    };
    let mut exit = function.blocks[1].clone();
    exit.id = SelectedBlockId(2);
    exit.origin = SelectedBlockOrigin::Source(BlockId::new(3).unwrap());
    let SelectedTerminator::Return { instruction, .. } = &mut exit.terminator else {
        unreachable!()
    };
    instruction.id = SelectedInstructionId(2);
    let SelectedTerminator::Return { instruction, .. } = &function.blocks[1].terminator else {
        unreachable!()
    };
    let mut branch = instruction.clone();
    branch.kind = SelectedInstructionKind::CompareI64Zero;
    let outgoing = SelectedSuccessor {
        block: exit.id,
        source_target: exit.source_block(),
        bindings: Vec::new(),
        psi_edge: EdgeId::new(4).unwrap(),
        ..repeated.clone()
    };
    function.blocks[1].terminator = SelectedTerminator::ConditionalBranch {
        instruction: branch,
        when_nonzero: repeated,
        when_zero: outgoing,
    };
    function.blocks.push(exit);
    compare(&function);
    function.blocks.reverse();
    compare(&function);
}
