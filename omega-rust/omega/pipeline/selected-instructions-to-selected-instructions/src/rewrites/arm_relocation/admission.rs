//! Shared admission for arm relocation: locate the named `member` in one
//! block's body, require that block to be a conditional arm — a plain
//! source block whose every predecessor edge leaves one fork head ending
//! in a two-successor conditional branch — locate the named `destination`
//! in that head's body, and prove the move sound in both directions — the
//! crossed window independent (no register or condition-state hazard
//! between the member and any crossed position, no interference with a
//! crossed edge's register transports, no barrier, call, hosted effect,
//! or call-roster entry inside the window, and no boundary settlement
//! whose observed executed prefix changes) and every member-written
//! location dead on every path the member's new position adds it to.
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{
    SelectedBlockOrigin, SelectedFunction, SelectedInstruction, SelectedInstructionId,
    SelectedSuccessor, SelectedTerminator,
};

use super::ArmRelocationError;
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::{
    edge_surface, plain_edge, terminator_instruction, terminator_successors, transport_conflict,
};
use crate::rewrites::dead_path;
use crate::rewrites::window_hazards::{
    coupled, has_call_contract, register_writes, schedulable, surface,
};

pub(super) struct Admission<'source> {
    pub function: &'source SelectedFunction,
    /// The member's own block: the branch's arm.
    pub block_index: usize,
    /// The member's index inside that block's body.
    pub member_index: usize,
    /// The fork head's index in `function.blocks`.
    pub target_index: usize,
    /// The destination instruction's position in the head body: the
    /// member lands at this index. Naming the head's terminator-carried
    /// instruction lands the member at the body end, index
    /// `target.instructions.len()`.
    pub landing_index: usize,
}

/// Whether the member's effect is pure register and condition-state work
/// that adds no observable execution on the paths it never ran on.
/// `schedulable` already cleared barrier kinds, call contracts, and
/// unaccounted memory-capable kinds, but a row-less load or private-slot
/// `Store64` still performs a memory access: hoisting it would add the
/// access — and any fault or slot write it carried — to every traversal
/// leaving through the head's other edges. The same holds for kinds whose
/// target encoding may architecturally fault: their proof obligations
/// establish definedness for the source operation, but this audit runs at
/// the selected level where the encoded trap behavior is the honest bound
/// — an execution that could fault must still run only on the paths that
/// ran it before.
fn hoistable(instruction: &SelectedInstruction) -> bool {
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

pub(super) fn admit<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Admission<'source>, ArmRelocationError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(ArmRelocationError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(ArmRelocationError::SourceMismatch)?;
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
        .ok_or(ArmRelocationError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    let member_instruction = &block.instructions[member_index];
    // The member's block must be a conditional arm of one fork head: a
    // plain source block, never the entry block — an edge into the entry
    // would feed it a traversal entry never saw — and reached only by
    // edges leaving that head. A predecessor leaving any other block
    // would hand the arm's stream a member that ran an extra time on that
    // path, and an arm with no predecessors is unreachable: hoisting the
    // member would start its execution.
    if block.id == function.entry_block || !matches!(block.origin, SelectedBlockOrigin::Source(_)) {
        return Err(ArmRelocationError::UnsupportedPair);
    }
    let mut head_index: Option<usize> = None;
    for (source_index, edge) in function
        .blocks
        .iter()
        .enumerate()
        .flat_map(|(index, candidate)| {
            terminator_successors(&candidate.terminator)
                .into_iter()
                .map(move |successor| (index, successor))
        })
    {
        if edge.block != block.id {
            continue;
        }
        match head_index {
            Some(existing) if existing != source_index => {
                return Err(ArmRelocationError::UnsupportedPair);
            }
            Some(_) => {}
            None => head_index = Some(source_index),
        }
    }
    let target_index = head_index.ok_or(ArmRelocationError::UnsupportedPair)?;
    let target = &function.blocks[target_index];
    // The head must not collapse back into the member's own block — an
    // arm that heads its own fork is the in-block family's case with a
    // back-edge reading — and an implementation block's origin carries
    // edge or case work the bounded audit does not cross.
    if target.id == block.id || !matches!(target.origin, SelectedBlockOrigin::Source(_)) {
        return Err(ArmRelocationError::UnsupportedPair);
    }
    // Only a two-successor conditional terminator gives the landing block
    // the fork this step rises through: an unconditional `Jump` is the
    // sole-predecessor family's case, and a terminator naming no
    // successors could never have reached the arm.
    let branch_edges: Vec<&SelectedSuccessor> = match &target.terminator {
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
            return Err(ArmRelocationError::UnsupportedPair);
        }
    };
    let terminator = terminator_instruction(&target.terminator);
    // The destination names the landing position directly: a body
    // instruction's own index, or the head's terminator-carried
    // instruction landing the member at the body end.
    let landing_index = target
        .instructions
        .iter()
        .position(|instruction| instruction.id == destination)
        .or_else(|| (terminator.id == destination).then_some(target.instructions.len()))
        .ok_or(ArmRelocationError::UnsupportedPair)?;
    // The member physically crosses every head edge into the arm: each
    // must be a plain semantic successor, and its register transports sit
    // between the member's old and new positions. The head's remaining
    // edges are the speculative side the member's new position adds it
    // to — they are never crossed positions.
    let mut landing_edges: Vec<&SelectedSuccessor> = Vec::new();
    let mut skipped_edges: Vec<&SelectedSuccessor> = Vec::new();
    for edge in &branch_edges {
        if edge.block == block.id {
            landing_edges.push(*edge);
        } else {
            skipped_edges.push(*edge);
        }
    }
    for edge in &landing_edges {
        if !plain_edge(edge) || transport_conflict(member_instruction, edge) {
            return Err(ArmRelocationError::UnsupportedPair);
        }
    }
    // The member's execution becomes unconditional: it newly runs on every
    // traversal leaving through the head's other edges, so only pure
    // register and condition-state work may rise — a roster-carrying or
    // unaccounted memory access or a potentially-faulting kind that ran
    // only on the arm's path would run on every head traversal after the
    // move.
    if schedulable(function, member_instruction) != Some(false) || !hoistable(member_instruction) {
        return Err(ArmRelocationError::UnsupportedInstruction);
    }
    // The branch terminator is the crossed edges' position: it is exempt
    // from the barrier-kind rule but not from the call or hazard audit.
    if has_call_contract(function, terminator.id) {
        return Err(ArmRelocationError::UnsupportedInstruction);
    }
    if coupled(member_instruction, terminator) {
        return Err(ArmRelocationError::UnsupportedPair);
    }
    // The member trades order with the positions at and after the landing
    // index in the head — they ran before it and now run after — and the
    // positions ahead of it in its own body, which now follow it across
    // the boundary. Every other position keeps the member on the side it
    // always had.
    for crossed in target.instructions[landing_index..]
        .iter()
        .chain(block.instructions[..member_index].iter())
    {
        schedulable(function, crossed).ok_or(ArmRelocationError::UnsupportedInstruction)?;
        if coupled(member_instruction, crossed) {
            return Err(ArmRelocationError::UnsupportedPair);
        }
    }
    // A settlement positioned past the member's index observed it inside
    // the arm's executed prefix; a settlement positioned past the landing
    // index observes it inside the head's. Both refuse; positions at or
    // before either boundary keep the executed set they always had.
    // Blocks on the speculative paths are unaffected: the member was
    // never in their streams and still never enters them.
    if function.boundary_settlements.iter().any(|settlement| {
        (settlement.block == block.id && settlement.instruction_index as usize > member_index)
            || (settlement.block == target.id
                && settlement.instruction_index as usize > landing_index)
    }) {
        return Err(ArmRelocationError::UnsupportedPair);
    }
    // The dead-path audit: every location the member writes must be dead —
    // unread until rewritten — along every path leaving the head's other
    // edges, where the member's execution is new.
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
                landing: dead_path::Landing::Speculated,
                // The vacated index stays silent: every walked path
                // reaching the arm crossed the head's new position first,
                // so the member's write is never missing there.
                vacated: dead_path::Vacated::Silent,
            },
            dead_path::Start::Edges(&skipped_edges),
        )
    {
        return Err(ArmRelocationError::UnsupportedPair);
    }
    // The scan walks every block body and terminator instruction once to
    // locate the member, and again with successor edges to find the arm's
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
            target.instructions[landing_index..]
                .iter()
                .chain(block.instructions[..member_index].iter())
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
        .ok_or(ArmRelocationError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| ArmRelocationError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(ArmRelocationError::WorkBudgetExceeded);
    }
    Ok(Admission {
        function,
        block_index,
        member_index,
        target_index,
        landing_index,
    })
}
