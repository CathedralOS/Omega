//! Shared admission for relocation of a run into the sole predecessor:
//! locate the named `first_member` and `last_member` bounding one
//! contiguous run inside a block's body, require that block to be reached
//! by exactly one edge — the unconditional `Jump` the destination block
//! ends in — and prove the window the move crosses independent: no
//! register or condition-state hazard between any member and any crossed
//! position, no member interference with the edge's register transports,
//! no roster-carrying run sharing the window with a second memory-access
//! actor, no barrier, call, hosted effect, or call-roster entry inside
//! the window, and no boundary settlement whose observed executed prefix
//! changes.
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{
    SelectedBlockOrigin, SelectedFunction, SelectedInstructionId, SelectedTerminator,
};

use super::PredecessorRunRelocationError;
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::{
    edge_accounted, plain_edge, terminator_instruction, terminator_successors, transport_conflict,
};
use crate::rewrites::window_hazards::{coupled, has_call_contract, schedulable, surface};

pub(super) struct Admission<'source> {
    pub function: &'source SelectedFunction,
    /// The run's own block — the edge's successor.
    pub block_index: usize,
    /// The run's first member index in the block body.
    pub first_index: usize,
    /// The run's last member index; the run is the contiguous span
    /// `first_index..=last_index` of at least two members.
    pub last_index: usize,
    /// The destination block's index in `function.blocks` — the sole
    /// predecessor the edge leaves.
    pub target_index: usize,
    /// The destination instruction's position in the predecessor body:
    /// the run lands at this index. Naming the predecessor's
    /// terminator-carried `Jump` instruction lands the run at the body
    /// end, index `target.instructions.len()`.
    pub landing_index: usize,
}

pub(super) fn admit<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    first_member: SelectedInstructionId,
    last_member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Admission<'source>, PredecessorRunRelocationError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(PredecessorRunRelocationError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(PredecessorRunRelocationError::SourceMismatch)?;
    let (block_index, first_index) = function
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block_index, block)| {
            block
                .instructions
                .iter()
                .position(|instruction| instruction.id == first_member)
                .map(|first_index| (block_index, first_index))
        })
        .ok_or(PredecessorRunRelocationError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    // The run is the contiguous span the two named members bound in this
    // block, in the named order; a repeated id or a last member that does
    // not follow the first names no multi-member run. A single member is
    // the one-instruction predecessor relocation the sibling family
    // already proves.
    let last_index = block
        .instructions
        .iter()
        .position(|instruction| instruction.id == last_member)
        .filter(|position| *position > first_index)
        .ok_or(PredecessorRunRelocationError::UnsupportedPair)?;
    let run = &block.instructions[first_index..=last_index];
    // Every edge into the run's block must be the one crossed edge: a
    // second predecessor gives the block a path the run would stop
    // executing on. The scan yields the sole predecessor's index; the
    // entry block reaches this check with no edge at all.
    let mut incoming = function
        .blocks
        .iter()
        .enumerate()
        .flat_map(|(index, candidate)| {
            terminator_successors(&candidate.terminator)
                .into_iter()
                .map(move |successor| (index, successor))
        })
        .filter(|(_, successor)| successor.block == block.id);
    let Some((target_index, _)) = incoming.next() else {
        return Err(PredecessorRunRelocationError::UnsupportedPair);
    };
    if incoming.next().is_some() {
        return Err(PredecessorRunRelocationError::UnsupportedPair);
    }
    let target = &function.blocks[target_index];
    // Only an unconditional jump keeps the run's execution count: every
    // traversal of the predecessor leaves through it into the run's
    // block, so the run executes once per predecessor traversal on either
    // side of the move. A conditional predecessor keeps a second exit the
    // run would newly execute on after relocating. The found edge is this
    // terminator's one successor, so `successor` names the crossed edge
    // directly.
    let SelectedTerminator::Jump {
        instruction: terminator,
        successor,
    } = &target.terminator
    else {
        return Err(PredecessorRunRelocationError::UnsupportedPair);
    };
    // The crossed edge must be a plain semantic successor: case dispatch,
    // continuation, structural transfer, and per-edge fuel all carry
    // boundary effects this step does not cross.
    if !plain_edge(successor) {
        return Err(PredecessorRunRelocationError::UnsupportedPair);
    }
    // A self-edge is the in-block run family's case with a back-edge
    // transport reading, not a cross-edge window. The entry block is
    // reached with no predecessor at all — an edge naming it would feed
    // it a traversal entry never saw — and an implementation block's
    // origin carries edge or case work the bounded audit does not cross.
    if target.id == block.id
        || block.id == function.entry_block
        || !matches!(target.origin, SelectedBlockOrigin::Source(_))
    {
        return Err(PredecessorRunRelocationError::UnsupportedPair);
    }
    // The destination names a position in the predecessor body — the run
    // lands at its index — or the predecessor's terminator-carried `Jump`
    // instruction, landing the run at the body end.
    let landing_index = target
        .instructions
        .iter()
        .position(|instruction| instruction.id == destination)
        .or_else(|| {
            (terminator_instruction(&target.terminator).id == destination)
                .then_some(target.instructions.len())
        })
        .ok_or(PredecessorRunRelocationError::UnsupportedPair)?;
    // Every member meets the schedulable bar itself; the run's memory
    // accounting is the union of its members' roster rows.
    let mut run_accounted = false;
    for member in run {
        run_accounted |= schedulable(function, member)
            .ok_or(PredecessorRunRelocationError::UnsupportedInstruction)?;
    }
    // The edge's register transports sit between the run's new and old
    // positions: a member defining the transported argument would hand
    // the binding a new value where the source bound the old, a member
    // defining the parameter would be overwritten before the run's block
    // observes it, and a member reading the parameter would observe the
    // pre-transport value after the move. Reading the argument is
    // harmless — the binding never writes it.
    for member in run {
        if transport_conflict(member, successor) {
            return Err(PredecessorRunRelocationError::UnsupportedPair);
        }
    }
    // The `Jump` instruction itself is the crossed edge's position: it is
    // exempt from the barrier-kind rule but not from the hazard, call, or
    // memory accounting. Rows the roster records with the edge's own
    // origin count as the edge position's memory surface.
    if has_call_contract(function, terminator.id) {
        return Err(PredecessorRunRelocationError::UnsupportedInstruction);
    }
    if run_accounted && edge_accounted(function, terminator, successor) {
        return Err(PredecessorRunRelocationError::UnsupportedPair);
    }
    for member in run {
        if coupled(member, terminator) {
            return Err(PredecessorRunRelocationError::UnsupportedPair);
        }
    }
    // Every member trades order with the positions at and after the
    // landing index in the predecessor body — they ran before the run and
    // now run after it — and the positions before the run in its own
    // body, which now follow it across the boundary. Every other position
    // keeps the run on the side it always had.
    for crossed in target.instructions[landing_index..]
        .iter()
        .chain(block.instructions[..first_index].iter())
    {
        // Every crossed instruction meets the same schedulable bar as the
        // members: no barrier kind, no call contract, and no unaccounted
        // memory reach. Its roster rows may keep their relative order
        // only while no member records any — a row-carrying run passing a
        // second accounted actor would reorder recorded accesses.
        let crossed_accounted = schedulable(function, crossed)
            .ok_or(PredecessorRunRelocationError::UnsupportedInstruction)?;
        if run_accounted && crossed_accounted {
            return Err(PredecessorRunRelocationError::UnsupportedPair);
        }
        for member in run {
            if coupled(member, crossed) {
                return Err(PredecessorRunRelocationError::UnsupportedPair);
            }
        }
    }
    // A settlement positioned past the landing index observes the run
    // inside the predecessor block's prefix; a settlement positioned past
    // the run's first index observed a member inside its own block's.
    // Both refuse; positions at or before either boundary keep the
    // executed set they always had.
    if function.boundary_settlements.iter().any(|settlement| {
        (settlement.block == block.id && settlement.instruction_index as usize > first_index)
            || (settlement.block == target.id
                && settlement.instruction_index as usize > landing_index)
    }) {
        return Err(PredecessorRunRelocationError::UnsupportedPair);
    }
    // The scan walks every block body and terminator instruction once to
    // locate the run's first member and count the block's predecessor
    // edges; the window audit walks every member-against-crossed operand
    // and unit surface, plus the function's three rosters and the edge's
    // binding roster once per member.
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
            run.iter().try_fold(total, |total, member| {
                target.instructions[landing_index..]
                    .iter()
                    .chain(block.instructions[..first_index].iter())
                    .chain(std::iter::once(terminator))
                    .try_fold(total, |total, crossed| {
                        total
                            .checked_add(surface(member))?
                            .checked_add(surface(crossed))
                    })
            })
        })
        .and_then(|total| {
            total
                .checked_add(function.memory_accesses.len())?
                .checked_add(function.calls.len())?
                .checked_add(function.boundary_settlements.len())?
                .checked_add(successor.bindings.len().saturating_mul(run.len()))
        })
        .ok_or(PredecessorRunRelocationError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| PredecessorRunRelocationError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(PredecessorRunRelocationError::WorkBudgetExceeded);
    }
    Ok(Admission {
        function,
        block_index,
        first_index,
        last_index,
        target_index,
        landing_index,
    })
}
