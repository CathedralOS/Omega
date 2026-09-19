//! Shared admission for fork relocation: locate the named `member` in one
//! block's body, require that block to end in a two-successor conditional
//! branch, locate the named `destination` in the one arm the branch's plain
//! edges alone feed, and prove the move sound in both directions — the
//! crossed window independent (no register or condition-state hazard
//! between the member and any crossed position, no interference with the
//! landing edge's register transports, no barrier, call, hosted effect, or
//! call-roster entry inside the window, and no boundary settlement whose
//! observed executed prefix changes) and every member-written location dead
//! on every path the member no longer executes on.
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{
    SelectedBlock, SelectedBlockOrigin, SelectedFunction, SelectedInstruction,
    SelectedInstructionId, SelectedSuccessor, SelectedTerminator,
};

use super::ForkRelocationError;
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::{
    all_edges, edge_surface, plain_edge, terminator_instruction, terminator_successors,
    transport_conflict,
};
use crate::rewrites::dead_path;
use crate::rewrites::window_hazards::{
    coupled, has_call_contract, register_writes, schedulable, surface,
};

pub(super) struct Admission<'source> {
    pub function: &'source SelectedFunction,
    /// The member's own block: the fork's branching head.
    pub block_index: usize,
    /// The member's index inside that block's body.
    pub member_index: usize,
    /// The landing arm's index in `function.blocks`.
    pub target_index: usize,
    /// The destination instruction's position in the arm body: the member
    /// lands at this index. Naming the arm's terminator-carried
    /// instruction lands the member at the body end, index
    /// `target.instructions.len()`.
    pub landing_index: usize,
}

/// Whether the member's effect is pure register and condition-state work
/// that cannot observe or abandon the execution it leaves behind.
/// `schedulable` already cleared barrier kinds, call contracts, and
/// unaccounted memory-capable kinds, but a row-less load or private-slot
/// `Store64` still performs a memory access: sinking it would remove the
/// access — and any fault or slot write it carried — from every traversal
/// leaving through the branch's other edges. The same holds for kinds
/// whose target encoding may architecturally fault: their proof
/// obligations establish definedness for the source operation, but this
/// audit runs at the selected level where the encoded trap behavior is
/// the honest bound — an execution that could fault must still run on
/// every path that ran it before.
fn sinkable(instruction: &SelectedInstruction) -> bool {
    use selected_instructions::SelectedInstructionKind::*;
    !matches!(
        instruction.kind,
        CopyBytes
            | LoadPacked { .. }
            | StorePacked { .. }
            | Store { .. }
            | Load8Indexed
            | Load64 { .. }
            | Load8 { .. }
            | Load16 { .. }
            | Load32 { .. }
            | Store64 { .. }
            | ExactDivideU64 { .. }
            | SaturatingDivide { .. }
            | SaturatingRemainder { .. }
    )
}

/// The position `destination` names inside `block`: a body instruction's
/// own index, or the body end when it names the terminator-carried
/// instruction.
fn landing_position(block: &SelectedBlock, destination: SelectedInstructionId) -> Option<usize> {
    block
        .instructions
        .iter()
        .position(|instruction| instruction.id == destination)
        .or_else(|| {
            (terminator_instruction(&block.terminator).id == destination)
                .then_some(block.instructions.len())
        })
}

pub(super) fn admit<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Admission<'source>, ForkRelocationError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(ForkRelocationError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(ForkRelocationError::SourceMismatch)?;
    let (block_index, member_index) = function
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block_index, block)| {
            block
                .instructions
                .iter()
                .position(|instruction| instruction.id == member)
                .map(|member_index| (block_index, member_index))
        })
        .ok_or(ForkRelocationError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    let member_instruction = &block.instructions[member_index];
    // Only a two-successor conditional terminator gives the member's block
    // the fork this step sinks through; every other terminator shape is
    // the single-edge family's case or no relocation at all.
    let branch_edges: Vec<&SelectedSuccessor> = match &block.terminator {
        SelectedTerminator::ConditionalBranch {
            when_nonzero,
            when_zero,
            ..
        } => vec![when_nonzero, when_zero],
        SelectedTerminator::ConditionalBranchU64LessThan {
            when_less,
            when_not_less,
            ..
        }
        | SelectedTerminator::ConditionalBranchI64LessThan {
            when_less,
            when_not_less,
            ..
        } => vec![when_less, when_not_less],
        SelectedTerminator::Jump { .. }
        | SelectedTerminator::HostedExitProcess { .. }
        | SelectedTerminator::Return { .. } => {
            return Err(ForkRelocationError::UnsupportedPair);
        }
    };
    let terminator = terminator_instruction(&block.terminator);
    // The destination selects the landing arm: exactly one distinct branch
    // target may name it, either as a body instruction — the member lands
    // on its index — or as the target's terminator-carried instruction,
    // landing the member at the body end.
    let mut candidates: Vec<(usize, usize)> = Vec::new();
    for edge in &branch_edges {
        let Some(arm_index) = function
            .blocks
            .iter()
            .position(|candidate| candidate.id == edge.block)
        else {
            return Err(ForkRelocationError::SourceMismatch);
        };
        if candidates.iter().any(|(arm, _)| *arm == arm_index) {
            continue;
        }
        if let Some(landing_index) = landing_position(&function.blocks[arm_index], destination) {
            candidates.push((arm_index, landing_index));
        }
    }
    let [(target_index, landing_index)] = candidates.as_slice() else {
        return Err(ForkRelocationError::UnsupportedPair);
    };
    let target_index = *target_index;
    let landing_index = *landing_index;
    let target = &function.blocks[target_index];
    // The arm must be a plain source block the branch alone reaches: a
    // second predecessor would hand the arm's stream a member that never
    // ran on that path, the member's own block would make the move
    // self-referential, the entry block is reached with no predecessor at
    // all, and an implementation block's origin carries edge or case work
    // the bounded audit does not cross.
    if target.id == block.id
        || target.id == function.entry_block
        || !matches!(target.origin, SelectedBlockOrigin::Source(_))
    {
        return Err(ForkRelocationError::UnsupportedPair);
    }
    let branch_edges_into_arm = branch_edges
        .iter()
        .filter(|edge| edge.block == target.id)
        .count();
    if all_edges(function)
        .filter(|(_, edge)| edge.block == target.id)
        .count()
        != branch_edges_into_arm
    {
        return Err(ForkRelocationError::UnsupportedPair);
    }
    // The member physically crosses every branch edge into the arm: each
    // must be a plain semantic successor, and its register transports sit
    // between the member's old and new positions.
    let mut landing_edges: Vec<&SelectedSuccessor> = Vec::new();
    let mut skipped_edges: Vec<&SelectedSuccessor> = Vec::new();
    for edge in &branch_edges {
        if edge.block == target.id {
            landing_edges.push(*edge);
        } else {
            skipped_edges.push(*edge);
        }
    }
    for edge in &landing_edges {
        if !plain_edge(edge) || transport_conflict(member_instruction, edge) {
            return Err(ForkRelocationError::UnsupportedPair);
        }
    }
    // The member's execution becomes conditional on the landing edge: only
    // pure register and condition-state work may sink — a roster-carrying
    // or unaccounted memory access that ran on every traversal would run
    // only on the landing path after the move, and a row-less load or
    // private-slot store would shed its access the same way.
    if schedulable(function, member_instruction) != Some(false) || !sinkable(member_instruction) {
        return Err(ForkRelocationError::UnsupportedInstruction);
    }
    // The branch terminator is the crossed edge's position: it is exempt
    // from the barrier-kind rule but not from the call or hazard audit.
    if has_call_contract(function, terminator.id) {
        return Err(ForkRelocationError::UnsupportedInstruction);
    }
    if coupled(member_instruction, terminator) {
        return Err(ForkRelocationError::UnsupportedPair);
    }
    // The member trades order with the positions behind it in its own body
    // and the positions before the landing index in the arm. Every other
    // position keeps the member on the side it always had.
    for crossed in block.instructions[member_index + 1..]
        .iter()
        .chain(target.instructions[..landing_index].iter())
    {
        schedulable(function, crossed).ok_or(ForkRelocationError::UnsupportedInstruction)?;
        if coupled(member_instruction, crossed) {
            return Err(ForkRelocationError::UnsupportedPair);
        }
    }
    // A settlement positioned past the member's index observed it inside
    // the source block's executed prefix; a settlement positioned past the
    // landing index observes it inside the arm's. Both refuse; positions at
    // or before either boundary keep the executed set they always had.
    // Blocks on the skipped paths are unaffected: the member was never in
    // their streams, so no prefix there ever contained or loses it.
    if function.boundary_settlements.iter().any(|settlement| {
        (settlement.block == block.id && settlement.instruction_index as usize > member_index)
            || (settlement.block == target.id
                && settlement.instruction_index as usize > landing_index)
    }) {
        return Err(ForkRelocationError::UnsupportedPair);
    }
    // The dead-path audit: every location the member writes must be dead —
    // unread until rewritten — along every path leaving the branch's other
    // edges.
    if !skipped_edges.is_empty()
        && !dead_path::dead(
            function,
            dead_path::Relocation {
                members: &[member_instruction],
                vacated_block: block_index,
                vacated_first: member_index,
                vacated_last: member_index,
                landing_block: target_index,
                landing_index,
                landing: dead_path::Landing::Executed,
                // The vacated index stays silent: every position behind it
                // in the head that could observe the missing write is a
                // crossed window position the hazard audit already owns.
                vacated: dead_path::Vacated::Silent,
            },
            dead_path::Start::Edges(&skipped_edges),
        )
    {
        return Err(ForkRelocationError::UnsupportedPair);
    }
    // The scan walks every block body and terminator instruction once to
    // locate the member, and again with successor edges to count
    // predecessor edges; the window audit walks the member's surface
    // against each crossed position's; the dead-path audit rescans a
    // block's stream and edge surfaces only while its entry set grows —
    // at most once per member location per block.
    let member_locations = register_writes(member_instruction).count()
        + member_instruction.implicit_defs.len()
        + member_instruction.clobbers.len();
    let block_scan: usize = function
        .blocks
        .iter()
        .map(|block| {
            block
                .instructions
                .iter()
                .chain(std::iter::once(terminator_instruction(&block.terminator)))
                .map(surface)
                .sum::<usize>()
                + terminator_successors(&block.terminator)
                    .iter()
                    .map(|edge| edge_surface(edge))
                    .sum::<usize>()
        })
        .sum();
    let steps = plan
        .functions
        .iter()
        .try_fold(0usize, |total, function| {
            function.blocks.iter().try_fold(total, |total, block| {
                total.checked_add(block.instructions.len())?.checked_add(1)
            })
        })
        .and_then(|total| {
            function.blocks.iter().try_fold(total, |total, candidate| {
                candidate
                    .instructions
                    .iter()
                    .chain(std::iter::once(terminator_instruction(
                        &candidate.terminator,
                    )))
                    .try_fold(total, |total, _| total.checked_add(1))
            })
        })
        .and_then(|total| {
            block.instructions[member_index + 1..]
                .iter()
                .chain(target.instructions[..landing_index].iter())
                .chain(std::iter::once(terminator))
                .try_fold(total, |total, crossed| {
                    total
                        .checked_add(surface(member_instruction))?
                        .checked_add(surface(crossed))
                })
        })
        .and_then(|total| {
            total
                .checked_add(function.memory_accesses.len())?
                .checked_add(function.calls.len())?
                .checked_add(function.boundary_settlements.len())
        })
        .and_then(|total| {
            landing_edges
                .iter()
                .try_fold(total, |total, edge| total.checked_add(edge.bindings.len()))
        })
        .and_then(|total| {
            total.checked_add(block_scan.saturating_mul(member_locations.saturating_add(1)))
        })
        .ok_or(ForkRelocationError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| ForkRelocationError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(ForkRelocationError::WorkBudgetExceeded);
    }
    Ok(Admission {
        function,
        block_index,
        member_index,
        target_index,
        landing_index,
    })
}
