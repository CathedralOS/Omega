//! The control-flow surface of one selected block, shared by every rewrite
//! that walks a function: the successor edges a terminator names, the
//! instruction it carries, the whole-function edge roster, and the per-edge
//! checks and surfaces the relocation admissions apply to an edge a member
//! crosses. `acyclic_paths` and `crossed_window` additionally derive the
//! general relocation window — the positions and edges every acyclic path
//! between a member run and its destination crosses — so a family that
//! admits a run-and-destination pair applies its own traversal-parity gates
//! and hands the window to one derivation and one audit.

/// Every successor edge a terminator names, in its physical record order;
/// `HostedExitProcess` and `Return` name none.
pub(super) use selected_instructions::terminator_successors;

use selected_instructions::{
    SelectedBlock, SelectedFunction, SelectedInstruction, SelectedInstructionId, SelectedSuccessor,
    terminator_instruction,
};

/// The function a plan-level rewrite entrypoint resolves — every
/// admission and audit begins by naming the index its member recorded.
/// The `(block_index, member_index)` of the instruction `member` names.
pub(super) fn member_position(
    function: &SelectedFunction,
    member: SelectedInstructionId,
) -> Option<(usize, usize)> {
    function
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block_index, block)| {
            block
                .instruction_index(member)
                .map(|member_index| (block_index, member_index))
        })
}

pub(super) fn function_at<'p, E>(
    plan: &'p selected_instructions::SelectedInstructionPlan,
    function_index: usize,
    miss: E,
) -> Result<&'p SelectedFunction, E> {
    plan.functions.get(function_index).ok_or(miss)
}

/// The instruction a terminator carries — a position every traversal of
/// its block observes, and the body-end landing position a destination can
/// name.

/// Every position of a block: its body instructions, then the instruction
/// its terminator carries.
pub(super) fn block_instructions(
    block: &SelectedBlock,
) -> impl Iterator<Item = &SelectedInstruction> {
    block
        .instructions
        .iter()
        .chain(std::iter::once(terminator_instruction(&block.terminator)))
}

/// Every instruction identity a block carries, in block order.
pub(super) fn block_instruction_ids(block: &SelectedBlock) -> impl Iterator<Item = u32> + '_ {
    block_instructions(block).map(|instruction| instruction.id.0)
}
/// The register-plus-unit transport surface one edge carries — the
/// dead-path audit's per-edge scan cost.
pub(super) fn edge_surface(successor: &SelectedSuccessor) -> usize {
    successor.bindings.len()
        + successor.structural_bindings.len()
        + successor
            .structural_case
            .as_ref()
            .map_or(0, |case| case.payloads.len())
}
