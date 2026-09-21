//! Shared admission for cross-edge relocation: locate the named `member`
//! in one block's body, require that block to end in an unconditional
//! `Jump` whose semantic successor is the destination's block, and hand
//! the crossed window to the shared run audit — `crossed_window` derives
//! the positions and edges every acyclic path between the two blocks
//! crosses (exactly this `Jump` edge under the gates below) and
//! `admit_run_relocation` proves the window independent once — no register
//! or condition-state hazard between the member and any crossed position,
//! no interference with the edge's register transports, no roster-carrying
//! member sharing the window with a second memory-access actor, no
//! barrier, call, hosted effect, or call-roster entry inside the window,
//! and no boundary settlement whose observed executed prefix changes.
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{
    SelectedBlockOrigin, SelectedFunction, SelectedInstructionId, SelectedTerminator,
};

use super::EdgeRelocationError;
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::{
    CrossingDirection, all_edges, crossed_window, plain_edge, terminator_instruction,
    terminator_successors,
};
use crate::rewrites::window_hazards::{RunRelocationRejection, admit_run_relocation, surface};

pub(super) struct Admission<'source> {
    pub function: &'source SelectedFunction,
    /// The member's own block.
    pub block_index: usize,
    /// The member's index inside that block's body.
    pub member_index: usize,
    /// The destination block's index in `function.blocks`.
    pub target_index: usize,
    /// The destination instruction's position in the target body: the
    /// member lands at this index. Naming the target's terminator-carried
    /// instruction lands the member at the body end, index
    /// `target.instructions.len()`.
    pub landing_index: usize,
}

pub(super) fn admit<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Admission<'source>, EdgeRelocationError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(EdgeRelocationError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(EdgeRelocationError::SourceMismatch)?;
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
        .ok_or(EdgeRelocationError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    let member_instruction = &block.instructions[member_index];
    // Only an unconditional semantic jump edge keeps the member's execution
    // count: every traversal of the member's block leaves through it, and —
    // with the single-predecessor rule below — every traversal of the
    // destination block arrives through it. A conditional terminator keeps
    // a second exit the member would still execute on after relocating.
    let SelectedTerminator::Jump { successor, .. } = &block.terminator else {
        return Err(EdgeRelocationError::UnsupportedPair);
    };
    // The crossed edge must be a plain semantic successor: case dispatch,
    // continuation, structural transfer, and per-edge fuel all carry
    // boundary effects this step does not cross. Structural bindings may
    // remain only while every transport is `Unused`, which moves nothing.
    if !plain_edge(successor) {
        return Err(EdgeRelocationError::UnsupportedPair);
    }
    let target_index = function
        .blocks
        .iter()
        .position(|candidate| candidate.id == successor.block)
        .ok_or(EdgeRelocationError::SourceMismatch)?;
    let target = &function.blocks[target_index];
    // A self-edge is the in-block family's case with a back-edge transport
    // reading, not a cross-edge window. The entry block is reached with no
    // predecessor at all, and an implementation block's origin carries edge
    // or case work the bounded audit does not cross.
    if target.id == block.id
        || target.id == function.entry_block
        || !matches!(target.origin, SelectedBlockOrigin::Source(_))
    {
        return Err(EdgeRelocationError::UnsupportedPair);
    }
    // Every edge into the destination block must be this one: a second
    // predecessor gives the block a path the member would newly execute on.
    if function
        .blocks
        .iter()
        .flat_map(|block| terminator_successors(&block.terminator))
        .filter(|edge| edge.block == target.id)
        .count()
        != 1
    {
        return Err(EdgeRelocationError::UnsupportedPair);
    }
    // The destination names a position in the target body — the member
    // lands at its index — or the target's terminator-carried instruction,
    // landing the member at the body end.
    let landing_index = target
        .instructions
        .iter()
        .position(|instruction| instruction.id == destination)
        .or_else(|| {
            (terminator_instruction(&target.terminator).id == destination)
                .then_some(target.instructions.len())
        })
        .ok_or(EdgeRelocationError::UnsupportedPair)?;
    // The crossed window is the shared derivation rather than this family's
    // own enumeration: the run is the one member, and the gates above leave
    // exactly one acyclic path — this `Jump` edge — so the path walk is
    // bounded by the function's edge roster alone. The shared audit applies
    // the hazard, memory-roster, transport, and settlement checks once.
    let edge_limit = all_edges(function).count();
    let crossing = crossed_window(
        function,
        block_index,
        member_index,
        member_index,
        target_index,
        landing_index,
        CrossingDirection::Forward,
        edge_limit,
    )
    .ok_or(EdgeRelocationError::WorkBudgetExceeded)?;
    admit_run_relocation(function, &[member_instruction], &crossing).map_err(rejection)?;
    // The scan walks every block body and terminator instruction once to
    // locate the member and count predecessor edges; the path walk touches
    // each edge once; the window audit walks the member's surface against
    // each crossed position's, plus the function's three rosters and the
    // edge's binding roster.
    let terminator = terminator_instruction(&block.terminator);
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
        .and_then(|total| total.checked_add(edge_limit))
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
                .checked_add(function.boundary_settlements.len())?
                .checked_add(successor.bindings.len())
        })
        .ok_or(EdgeRelocationError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| EdgeRelocationError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(EdgeRelocationError::WorkBudgetExceeded);
    }
    Ok(Admission {
        function,
        block_index,
        member_index,
        target_index,
        landing_index,
    })
}

/// Keeps the family's typed rejection vocabulary over the shared audit's
/// rejection kinds: an unschedulable position is the instruction-level
/// refusal and every window-level refusal is the pair kind.
fn rejection(rejection: RunRelocationRejection) -> EdgeRelocationError {
    match rejection {
        RunRelocationRejection::Unschedulable => EdgeRelocationError::UnsupportedInstruction,
        RunRelocationRejection::UnreachableDestination
        | RunRelocationRejection::Coupled
        | RunRelocationRejection::MemoryOrdering
        | RunRelocationRejection::TransportConflict
        | RunRelocationRejection::NonPlainEdge
        | RunRelocationRejection::Settlement => EdgeRelocationError::UnsupportedPair,
    }
}
