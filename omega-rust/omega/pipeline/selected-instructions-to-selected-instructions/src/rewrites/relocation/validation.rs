//! Independent replay of member-run relocation: the validator
//! reconstructs the contiguous run the named members bound inside one
//! block's body, the destination block and landing index the destination
//! instruction names, and the window the move crosses on its own audit —
//! sharing no state with the producer's `admission` record — deriving
//! every acyclic path between the run block and the destination,
//! re-proving that the move changes no traversal (every predecessor edge
//! into the destination lies on a crossed path and every exit of every
//! crossed block reaches the destination), and applying the shared
//! hazard, transport, memory-roster, and settlement audit once. It then
//! requires the proposed program to place exactly the run's members at
//! the landing index in their original order and restores the complete
//! source by content — every crossed instruction, every other block and
//! instruction, register, roster row, call, settlement, and edge
//! included.
use std::collections::BTreeSet;
use std::sync::Arc;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{
    SelectedBlockOrigin, SelectedFunction, SelectedInstructionId, SelectedInstructionPlan,
};
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{MemberRunRelocationError, MemberRunRelocationReceipt, ValidatedMemberRunRelocation};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::{
    CrossingDirection, all_edges, crossed_window, edge_surface, terminator_instruction,
    terminator_successors,
};
use crate::rewrites::window_hazards::{RunRelocationRejection, admit_run_relocation, surface};

/// The acyclic-path walk bound for the validator's own derivation: a
/// function whose run-to-destination paths take more edges than this
/// abandons as over budget rather than reporting a truncated window —
/// the same bound the producer's admission publishes.
const PATH_EDGE_LIMIT: usize = 64;

/// The validator's own reconstruction of the relocation the contract
/// permits: the run's block and bounding positions, the destination
/// block, and the landing index the move takes there. It shares no state
/// with the producer's `admission` record.
struct Reconstructed<'source> {
    function: &'source SelectedFunction,
    /// The run's own block, as an index into `function.blocks`.
    run_block: usize,
    /// First body index the run occupies.
    run_start: usize,
    /// Last body index the run occupies.
    run_end: usize,
    /// Index into `function.blocks` of the block the run lands in —
    /// `run_block` itself for the in-block case.
    destination_block: usize,
    /// The body index the run lands at, in the destination block's
    /// source-order positions.
    landing_index: usize,
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

/// Reconstruct the legality of moving the run `first_member`..`last_member`
/// onto `destination` from the source records: locate the run as the
/// contiguous span the two members bound in one block in the named order,
/// resolve the destination to a landing position in any block's body or
/// terminator-carried instruction, derive the window the move crosses
/// over every acyclic path between the two blocks, re-prove the traversal
/// preservation — every predecessor edge into the destination lies on a
/// crossed path and every exit of every block a crossed path leaves
/// reaches the destination — then apply the shared schedulable, hazard,
/// memory-roster, transport, and settlement audit once and account the
/// family's measured steps against the budget. Nothing in this audit
/// reads the producer's admission decision, so a producer-side legality
/// error fails here even when the proposal matches the emitted edit.
#[allow(clippy::too_many_arguments)]
fn reconstruct<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    first_member: SelectedInstructionId,
    last_member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Reconstructed<'source>, MemberRunRelocationError> {
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
    // The validator's own audit walks the same surfaces the family
    // publishes: every block body and terminator instruction once to
    // locate the run and the destination; the path walk bounded by
    // PATH_EDGE_LIMIT; the gained/lost audit reading the predecessor and
    // per-block successor rosters once; the window audit walking every
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
    Ok(Reconstructed {
        function,
        run_block,
        run_start,
        run_end,
        destination_block,
        landing_index,
    })
}

/// Independently consume the proposed program: the validator re-derives
/// the admitted run, window and landing index from the source without
/// the producer's admission routine, the proposal must place exactly the
/// run's members on the landing index in the destination block in their
/// original order, and moving the run back must restore the complete
/// source by content — every crossed instruction, every other block and
/// instruction, register, roster row, call, settlement, and edge
/// included.
pub fn validate_member_run_relocation(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    first_member: SelectedInstructionId,
    last_member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedMemberRunRelocation, MemberRunRelocationError> {
    let reconstructed = reconstruct(
        source,
        function_index,
        first_member,
        last_member,
        destination,
        environment,
        budget,
    )?;
    let run_len = reconstructed.run_end - reconstructed.run_start + 1;
    // A forward in-block move counts its landing index in source order
    // and puts the run's last member on it; the run's transformed start
    // sits one width less the named slot behind.
    let insert_index = if reconstructed.run_block == reconstructed.destination_block
        && reconstructed.landing_index > reconstructed.run_end
    {
        reconstructed.landing_index - run_len + 1
    } else {
        reconstructed.landing_index
    };
    if proposed
        .functions
        .get(function_index)
        .and_then(|function| function.blocks.get(reconstructed.destination_block))
        .and_then(|block| block.instructions.get(insert_index..insert_index + run_len))
        != Some(
            &reconstructed.function.blocks[reconstructed.run_block].instructions
                [reconstructed.run_start..=reconstructed.run_end],
        )
    {
        return Err(MemberRunRelocationError::ReplayMismatch);
    }
    let mut restored = proposed.clone();
    let run: Vec<_> = restored.functions[function_index].blocks[reconstructed.destination_block]
        .instructions
        .drain(insert_index..insert_index + run_len)
        .collect();
    restored.functions[function_index].blocks[reconstructed.run_block]
        .instructions
        .splice(reconstructed.run_start..reconstructed.run_start, run);
    if restored != *source.selected_plan() {
        return Err(MemberRunRelocationError::ReplayMismatch);
    }
    Ok(ValidatedMemberRunRelocation {
        receipt: MemberRunRelocationReceipt {
            source_selected: source.selected_identity(),
            transformed_selected: selected_instruction_plan_identity(&proposed),
            optimization_unit: source.optimization_unit_identity(),
            fuel_schedule: source.fuel_schedule_identity(),
        },
        transformed: Arc::new(proposed),
    })
}
