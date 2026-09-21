//! The one relocation admission: locate the contiguous member run the two
//! named members bound inside one block's body, resolve the destination
//! instruction to a landing position in its block, derive the window the
//! move crosses over every acyclic path between the two blocks, prove the
//! move changes no traversal — every predecessor edge into the
//! destination lies on a crossed path and every exit of every crossed
//! block reaches the destination — then apply the shared hazard,
//! transport, memory-roster and settlement audit once.
use std::collections::BTreeSet;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{SelectedBlockOrigin, SelectedInstructionId};

use super::MemberRunRelocationError;
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::{
    CrossingDirection, PATH_EDGE_LIMIT, all_edges, crossed_window, edge_surface,
    terminator_instruction, terminator_successors,
};
use crate::rewrites::window_hazards::{RunRelocationRejection, admit_run_relocation, surface};

pub(super) struct Admission {
    /// The run's own block, as an index into `function.blocks`.
    pub run_block: usize,
    /// First body index the run occupies.
    pub run_start: usize,
    /// Last body index the run occupies.
    pub run_end: usize,
    /// Index into `function.blocks` of the block the run lands in —
    /// `run_block` itself for the in-block case.
    pub destination_block: usize,
    /// The body index the run lands at, in the destination block's
    /// source-order positions.
    pub landing_index: usize,
}

/// Map the shared audit's rejection onto this module's public error: an
/// unschedulable member or crossed position is `UnsupportedInstruction`;
/// every shape-level refusal — unreachable destination, non-plain edge,
/// transport conflict, coupling, memory ordering, settlement — is
/// `UnsupportedPair`.
fn reject(rejection: RunRelocationRejection) -> MemberRunRelocationError {
    match rejection {
        RunRelocationRejection::Unschedulable => MemberRunRelocationError::UnsupportedInstruction,
        RunRelocationRejection::UnreachableDestination
        | RunRelocationRejection::Coupled
        | RunRelocationRejection::MemoryOrdering
        | RunRelocationRejection::TransportConflict
        | RunRelocationRejection::NonPlainEdge
        | RunRelocationRejection::Settlement => MemberRunRelocationError::UnsupportedPair,
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn admit<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    first_member: SelectedInstructionId,
    last_member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Admission, MemberRunRelocationError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(MemberRunRelocationError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(MemberRunRelocationError::SourceMismatch)?;
    let (run_block, run_start) = function
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block_index, block)| {
            block
                .instructions
                .iter()
                .position(|instruction| instruction.id == first_member)
                .map(|run_start| (block_index, run_start))
        })
        .ok_or(MemberRunRelocationError::SourceMismatch)?;
    // The run is the contiguous span the two named members bound in this
    // block, in the named order; one member is the member-level move. A
    // last member absent from the same body, or earlier than the first,
    // names no run.
    let block = &function.blocks[run_block];
    let run_end = block
        .instructions
        .iter()
        .position(|instruction| instruction.id == last_member)
        .filter(|position| *position >= run_start)
        .ok_or(MemberRunRelocationError::UnsupportedPair)?;
    // The destination names a body position in any block — the run lands
    // at its index — or a terminator-carried instruction, landing the run
    // at that block's body end.
    let (destination_block, landing_index) = function
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block_index, candidate)| {
            candidate
                .instructions
                .iter()
                .position(|instruction| instruction.id == destination)
                .map(|index| (block_index, index))
                .or_else(|| {
                    (terminator_instruction(&candidate.terminator).id == destination)
                        .then_some((block_index, candidate.instructions.len()))
                })
        })
        .ok_or(MemberRunRelocationError::UnsupportedPair)?;
    let in_block = run_block == destination_block;
    if in_block && landing_index >= run_start && landing_index <= run_end {
        // Landing inside the run's own span is a degenerate move.
        return Err(MemberRunRelocationError::UnsupportedPair);
    }
    let crossing = crossed_window(
        function,
        run_block,
        run_start,
        run_end,
        destination_block,
        landing_index,
        CrossingDirection::Forward,
        PATH_EDGE_LIMIT,
    )
    .ok_or(MemberRunRelocationError::WorkBudgetExceeded)?;
    if !in_block {
        let destination = &function.blocks[destination_block];
        // The entry block is reached with no predecessor at all, and an
        // implementation block's origin carries edge or case work the
        // audit does not cross.
        if destination.id == function.entry_block
            || !matches!(destination.origin, SelectedBlockOrigin::Source(_))
        {
            return Err(MemberRunRelocationError::UnsupportedPair);
        }
        if !crossing.reachable {
            return Err(MemberRunRelocationError::UnsupportedPair);
        }
        // Traversals gained: an edge into the destination that no path
        // from the run crosses gives the run a traversal that never ran
        // it before the move.
        let crossed_edges: BTreeSet<_> = crossing
            .edges
            .iter()
            .map(|edge| (edge.block, edge.successor.psi_edge))
            .collect();
        for (from, predecessor) in all_edges(function) {
            if predecessor.block == destination.id
                && !crossed_edges.contains(&(from, predecessor.psi_edge))
            {
                return Err(MemberRunRelocationError::UnsupportedPair);
            }
        }
        // Traversals lost: every exit of every block a path leaves must
        // itself lie on a complete path to the destination — otherwise
        // some traversal of the run's region never reaches the run's new
        // position.
        for source_block in crossing
            .edges
            .iter()
            .map(|edge| edge.block)
            .collect::<BTreeSet<_>>()
        {
            let source = function
                .blocks
                .iter()
                .find(|block| block.id == source_block)
                .ok_or(MemberRunRelocationError::SourceMismatch)?;
            if terminator_successors(&source.terminator)
                .iter()
                .any(|exit| !crossed_edges.contains(&(source_block, exit.psi_edge)))
            {
                return Err(MemberRunRelocationError::UnsupportedPair);
            }
        }
    }
    let members: Vec<_> = block.instructions[run_start..=run_end].iter().collect();
    admit_run_relocation(function, &members, &crossing).map_err(reject)?;
    // The scan walks every block body and terminator instruction once to
    // locate the run and the destination; the path walk is bounded by
    // PATH_EDGE_LIMIT; the gained/lost audit reads the predecessor and
    // per-block successor rosters once; the window audit walks every
    // member-against-crossed-position and member-against-crossed-edge
    // surface, plus the function's three rosters once.
    let crossed_positions = crossing
        .positions
        .iter()
        .flat_map(|(_, positions)| positions.iter());
    let steps = plan
        .functions
        .iter()
        .try_fold(0usize, |total, function| {
            function.blocks.iter().try_fold(total, |total, block| {
                total.checked_add(block.instructions.len())?.checked_add(1)
            })
        })
        .and_then(|total| total.checked_add(PATH_EDGE_LIMIT))
        .and_then(|total| {
            function.blocks.iter().try_fold(total, |total, block| {
                total.checked_add(terminator_successors(&block.terminator).len())
            })
        })
        .and_then(|total| {
            members.iter().try_fold(total, |total, member| {
                crossed_positions
                    .clone()
                    .try_fold(total, |total, _| total.checked_add(surface(member)))
            })
        })
        .and_then(|total| {
            crossing
                .positions
                .iter()
                .try_fold(total, |total, (block_index, positions)| {
                    positions.iter().try_fold(total, |total, position| {
                        total.checked_add(surface(
                            &function.blocks[*block_index].instructions[*position],
                        ))
                    })
                })
        })
        .and_then(|total| {
            crossing.edges.iter().try_fold(total, |total, edge| {
                total
                    .checked_add(surface(edge.instruction).saturating_mul(members.len()))?
                    .checked_add(edge_surface(edge.successor).saturating_mul(members.len()))
            })
        })
        .and_then(|total| {
            total
                .checked_add(function.memory_accesses.len())?
                .checked_add(function.calls.len())?
                .checked_add(function.boundary_settlements.len())
        })
        .ok_or(MemberRunRelocationError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| MemberRunRelocationError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(MemberRunRelocationError::WorkBudgetExceeded);
    }
    Ok(Admission {
        run_block,
        run_start,
        run_end,
        destination_block,
        landing_index,
    })
}
