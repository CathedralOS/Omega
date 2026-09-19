//! Shared admission for inflow relocation: locate the named `member` in
//! one block's body, locate the named `destination` in a block that ends
//! in the lone unconditional `Jump` reaching the member's block — a plain
//! semantic successor edge — require the member's block to be a join at
//! least one other predecessor's edge also reaches, and prove the move
//! sound in both directions — the crossed window independent (no register
//! or condition-state hazard between the member and any crossed position,
//! no interference with the crossed edge's register transports, no
//! barrier, call, hosted effect, or call-roster entry inside the window,
//! and no boundary settlement whose observed executed prefix changes) and
//! every member-written location dead on the paths the move removes,
//! where the member's vacated index publishes them on arrivals that no
//! longer run it.
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{
    SelectedBlockOrigin, SelectedFunction, SelectedInstruction, SelectedInstructionId,
    SelectedTerminator,
};

use super::InflowRelocationError;
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
    /// The member's own block: the join the inflow feeds.
    pub block_index: usize,
    /// The member's index inside that block's body.
    pub member_index: usize,
    /// The inflow block's index in `function.blocks`: the predecessor the
    /// destination names.
    pub target_index: usize,
    /// The destination instruction's position in the inflow body: the
    /// member lands at this index. Naming the inflow's terminator-carried
    /// instruction lands the member at the body end, index
    /// `target.instructions.len()`.
    pub landing_index: usize,
}

/// Whether the member's effect is pure register and condition-state work
/// whose missing execution leaves no observable trace on the arrivals the
/// move removes. `schedulable` already cleared barrier kinds, call
/// contracts, and unaccounted memory-capable kinds, but a row-less load
/// or private-slot `Store64` still performs a memory access: hoisting it
/// would remove the access — and any fault or slot write it carried —
/// from every traversal entering the join through the other inflows. The
/// same holds for kinds whose target encoding may architecturally fault:
/// their proof obligations establish definedness for the source
/// operation, but this audit runs at the selected level where the encoded
/// trap behavior is the honest bound — an execution that could fault must
/// still run on every path that ran it before.
fn removable(instruction: &SelectedInstruction) -> bool {
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
) -> Result<Admission<'source>, InflowRelocationError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(InflowRelocationError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(InflowRelocationError::SourceMismatch)?;
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
        .ok_or(InflowRelocationError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    let member_instruction = &block.instructions[member_index];
    // The destination names the landing position directly: the block
    // holding it must be the inflow — one predecessor ending in the lone
    // `Jump` that reaches the member's block. Naming a terminator-carried
    // instruction lands the member at the body end.
    let (target_index, landing_index) = function
        .blocks
        .iter()
        .enumerate()
        .find_map(|(index, candidate)| {
            candidate
                .instructions
                .iter()
                .position(|instruction| instruction.id == destination)
                .map(|position| (index, position))
                .or_else(|| {
                    (terminator_instruction(&candidate.terminator).id == destination)
                        .then_some((index, candidate.instructions.len()))
                })
        })
        .ok_or(InflowRelocationError::UnsupportedPair)?;
    let target = &function.blocks[target_index];
    // Only an unconditional semantic jump edge keeps the member's
    // execution count on this inflow's path: every traversal of the
    // inflow block leaves through it and reaches the member's block
    // exactly once, so the member still runs on every arrival through
    // the crossed edge. A conditional terminator keeps a second exit the
    // member would newly execute on — the arm and join families' burden
    // — and a terminator naming no successor reaches no join.
    let SelectedTerminator::Jump {
        instruction: terminator,
        successor,
    } = &target.terminator
    else {
        return Err(InflowRelocationError::UnsupportedPair);
    };
    if successor.block != block.id {
        return Err(InflowRelocationError::UnsupportedPair);
    }
    // The crossed edge must be a plain semantic successor: case dispatch,
    // continuation, structural transfer, and per-edge fuel all carry
    // boundary effects this step does not cross. Structural bindings may
    // remain only while every transport is `Unused`, which moves nothing.
    if !plain_edge(successor) {
        return Err(InflowRelocationError::UnsupportedPair);
    }
    // A destination in the member's own block is the in-block family's
    // case with a back-edge transport reading, not a cross-edge window.
    // The member's block may not be the entry block — the first
    // traversal crosses no inflow edge at all, so the move would remove
    // an execution the dead-path audit cannot see — and the inflow block
    // must be a plain source block: an implementation block's origin
    // carries edge or case work the bounded audit does not cross.
    if target.id == block.id
        || block.id == function.entry_block
        || !matches!(target.origin, SelectedBlockOrigin::Source(_))
    {
        return Err(InflowRelocationError::UnsupportedPair);
    }
    // At least one edge into the member's block must leave another block:
    // the single-predecessor shape is the predecessor family's case,
    // where no traversal loses the member. Every other inflow — whatever
    // it carries — runs before the member's old position on its own
    // arrivals and is never crossed; the dead-path audit reads its
    // register surface where the move removes the member's write.
    if !all_edges(function)
        .any(|(source_block, edge)| edge.block == block.id && source_block != target.id)
    {
        return Err(InflowRelocationError::UnsupportedPair);
    }
    // The member's execution leaves every arrival through the join's
    // other inflows, so only pure register and condition-state work may
    // rise — a roster-carrying or unaccounted memory access or a
    // potentially-faulting kind that ran on every arrival would silently
    // vanish from the arrivals the move removes.
    if schedulable(function, member_instruction) != Some(false) || !removable(member_instruction) {
        return Err(InflowRelocationError::UnsupportedInstruction);
    }
    // The crossed edge's register transports sit between the member's old
    // and new positions.
    if transport_conflict(member_instruction, successor) {
        return Err(InflowRelocationError::UnsupportedPair);
    }
    // The `Jump` instruction itself is the crossed edge's position: it is
    // exempt from the barrier-kind rule but not from the call or hazard
    // audit. Its memory rows, and any rows the roster logs with the
    // edge's own origin, are accounted positions a row-less member
    // crosses without reordering a recorded access.
    if has_call_contract(function, terminator.id) {
        return Err(InflowRelocationError::UnsupportedInstruction);
    }
    if coupled(member_instruction, terminator) {
        return Err(InflowRelocationError::UnsupportedPair);
    }
    // The member trades order with the positions at and after the
    // landing index in the inflow body and the positions before its index
    // in its own body. Every other position keeps the member on the side
    // it always had. A roster-carrying crossed position is an accounted
    // access the row-less member cannot reorder, so only the
    // schedulability and hazard gates apply.
    for crossed in target.instructions[landing_index..]
        .iter()
        .chain(block.instructions[..member_index].iter())
    {
        schedulable(function, crossed).ok_or(InflowRelocationError::UnsupportedInstruction)?;
        if coupled(member_instruction, crossed) {
            return Err(InflowRelocationError::UnsupportedPair);
        }
    }
    // A settlement positioned past the member's index observed it inside
    // the source block's executed prefix; a settlement positioned past
    // the landing index observes it inside the inflow block's — on this
    // inflow's arrivals, where it has not run yet. Both refuse; positions
    // at or before either boundary keep the executed set they always had.
    // The other inflow blocks are unaffected: the member never enters
    // their streams.
    if function.boundary_settlements.iter().any(|settlement| {
        (settlement.block == block.id && settlement.instruction_index as usize > member_index)
            || (settlement.block == target.id
                && settlement.instruction_index as usize > landing_index)
    }) {
        return Err(InflowRelocationError::UnsupportedPair);
    }
    // The dead-path audit: every location the member writes must be dead
    // — unread until rewritten — on every arrival through the join's
    // other inflows. The walk enters the member's block with nothing
    // live: the other inflow edges run before the member's old position
    // on their own arrivals, so their transports meet no divergence at
    // the boundary — it is the vacated index that publishes the member's
    // locations, the missing write's divergence on the removed paths. A
    // walked path looping back through the inflow block runs the member
    // at its new position — an ordinary execution, whose write
    // republishes the locations it carries.
    if !dead_path::dead(
        function,
        dead_path::Relocation {
            members: &[member_instruction],
            vacated_block: block_index,
            vacated_first: member_index,
            vacated_last: member_index,
            landing_block: target_index,
            landing_index,
            landing: dead_path::Landing::Executed,
            // The member's write is missing where the source still ran
            // it: the walked arrivals enter the member's block through
            // the other inflow edges, beside its new position rather than
            // through it, so the vacated index publishes its locations.
            vacated: dead_path::Vacated::Removed,
        },
        dead_path::Start::Block(block_index),
    ) {
        return Err(InflowRelocationError::UnsupportedPair);
    }
    // The scan walks every block body and terminator instruction once to
    // locate the member, and again with successor edges to locate the
    // destination and find the join's other inflows; the window audit
    // walks the member's surface against each crossed position's; the
    // dead-path audit rescans a block's stream and edge surfaces only
    // while its entry set grows — at most once per member location per
    // block.
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
            function.blocks.iter().try_fold(total, |total, candidate| {
                total.checked_add(terminator_successors(&candidate.terminator).len())
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
        .and_then(|total| total.checked_add(successor.bindings.len()))
        .and_then(|total| {
            total.checked_add(block_scan.saturating_mul(member_locations.saturating_add(1)))
        })
        .ok_or(InflowRelocationError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| InflowRelocationError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(InflowRelocationError::WorkBudgetExceeded);
    }
    Ok(Admission {
        function,
        block_index,
        member_index,
        target_index,
        landing_index,
    })
}
