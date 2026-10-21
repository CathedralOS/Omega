//! Raw representation queries only; these rows grant no semantic admission.
use super::*;
use arena::{Arena, HandleSpan};
use register_model::RegisterClassId;
use selected_instructions::{
    BlockLiveness, FunctionLiveness, InstructionLiveness, LivenessPosition, OperandPosition,
    SelectedBlockId, SelectedInstructionId,
};
use semantic_vocabulary::{BlockId, MachineId};

fn operands(
    arena: &mut Arena<OperandPosition>,
    position: u32,
    instruction: u32,
    base: u32,
) -> HandleSpan<OperandPosition> {
    arena.insert_many((0..3).map(|ordinal| OperandPosition {
        position: LivenessPosition(position),
        instruction: SelectedInstructionId(instruction),
        operand: ordinal as u16,
        virtual_register: VirtualRegisterId(base + ordinal),
        class: RegisterClassId(0),
        access: if ordinal == 2 {
            RegisterOperandAccess::Def
        } else {
            RegisterOperandAccess::Use
        },
        tied_to: (ordinal == 2).then_some(0),
        early_clobber: ordinal == 2,
        ..OperandPosition::default()
    }))
}
fn block(
    block: u32,
    position: u32,
    instruction: u32,
    operands: HandleSpan<OperandPosition>,
) -> BlockLiveness {
    BlockLiveness {
        block: SelectedBlockId(block),
        source_block: BlockId::new(u64::from(block) + 1).unwrap(),
        virtual_live_in: vec![],
        virtual_live_out: vec![],
        unit_live_in: vec![],
        unit_live_out: vec![],
        successors: vec![],
        instructions: vec![InstructionLiveness {
            position: LivenessPosition(position),
            instruction: SelectedInstructionId(instruction),
            operands,
            virtual_uses: vec![],
            virtual_defs: vec![],
            virtual_live_in: vec![],
            virtual_live_out: vec![],
            unit_uses: vec![],
            unit_defs: vec![],
            unit_clobbers: vec![],
            unit_live_in: vec![],
            unit_live_out: vec![],
        }],
    }
}
fn function() -> FunctionLiveness {
    let mut arena = Arena::new();
    // Table order and sparse instruction identities do not imply ownership.
    let second = operands(&mut arena, 1, 17, 20);
    let first = operands(&mut arena, 0, 900, 10);
    FunctionLiveness {
        machine: MachineId::new(1).unwrap(),
        entry_definitions: vec![],
        operand_positions: arena,
        blocks: vec![block(100, 0, 900, first), block(200, 1, 17, second)],
    }
}
#[test]
fn constraints_follow_instruction_spans_and_owning_blocks() {
    let function = function();
    let ties = derive_tied_pairs(0, &function).unwrap();
    let early = derive_early_clobbers(0, &function).unwrap();
    assert_eq!(
        ties.iter()
            .map(|tie| (
                tie.block.0,
                tie.instruction.0,
                tie.use_virtual_register.0,
                tie.def_virtual_register.0
            ))
            .collect::<Vec<_>>(),
        [(100, 900, 10, 12), (200, 17, 20, 22)]
    );
    assert_eq!(
        early
            .iter()
            .map(|row| (
                row.block.0,
                row.instruction.0,
                row.uses[0].virtual_register.0
            ))
            .collect::<Vec<_>>(),
        [(100, 900, 11), (200, 17, 21)]
    );
}
#[test]
fn missing_tie_operand_still_refuses() {
    let mut function = function();
    let definition = function.blocks[0].instructions[0].operands.handle_at(2);
    function.operand_positions.get_mut(definition).tied_to = Some(77);
    assert_eq!(
        derive_tied_pairs(3, &function),
        Err(LiveRangeError::UnsupportedTiedOperand {
            function: 3,
            instruction: 900,
            operand: 2
        })
    );
}
#[test]
fn liveness_identity_binds_operand_membership() {
    let mut function = function();
    let original = selected_instructions::liveness_function_digest(&function);
    function.blocks[0].instructions[0].operands = function.blocks[1].instructions[0].operands;
    assert_ne!(
        original,
        selected_instructions::liveness_function_digest(&function)
    );
}
