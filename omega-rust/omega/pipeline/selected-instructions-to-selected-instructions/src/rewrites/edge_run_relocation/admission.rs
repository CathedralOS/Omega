//! Shared admission for cross-edge run relocation: locate the named
//! `first_member` and `last_member` bounding one contiguous run inside a
//! block's body, require that block to end in an unconditional `Jump`
//! whose semantic successor is the destination's block, and prove the
//! window the move crosses independent — no register or condition-state
//! hazard between any member and any crossed position, no member
//! interference with the edge's register transports, no roster-carrying
//! run sharing the window with a second memory-access actor, no barrier,
//! call, hosted effect, or call-roster entry inside the window, and no
//! boundary settlement whose observed executed prefix changes.
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{
    SelectedBlockOrigin, SelectedFunction, SelectedInstructionId, SelectedTerminator,
};

use super::EdgeRunRelocationError;
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::{
    edge_accounted, plain_edge, terminator_instruction, terminator_successors, transport_conflict,
};
use crate::rewrites::window_hazards::{coupled, has_call_contract, schedulable, surface};

pub(super) struct Admission<'source> {
    pub function: &'source SelectedFunction,
    /// The run's own block: it ends in the crossed `Jump` edge.
    pub block_index: usize,
    /// The run's first member index in the block body.
    pub first_index: usize,
    /// The run's last member index; the run is the contiguous span
    /// `first_index..=last_index` of at least two members.
    pub last_index: usize,
    /// The destination block's index in `function.blocks`.
    pub target_index: usize,
    /// The destination instruction's position in the target body: the run
    /// lands at this index. Naming the target's terminator-carried
    /// instruction lands the run at the body end, index
    /// `target.instructions.len()`.
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
) -> Result<Admission<'source>, EdgeRunRelocationError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(EdgeRunRelocationError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(EdgeRunRelocationError::SourceMismatch)?;
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
        .ok_or(EdgeRunRelocationError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    // The run is the contiguous span the two named members bound in this
    // block, in the named order; a repeated id or a last member that does
    // not follow the first names no multi-member run. A single member is
    // the one-instruction cross-edge relocation the sibling family already
    // proves.
    let last_index = block
        .instructions
        .iter()
        .position(|instruction| instruction.id == last_member)
        .filter(|position| *position > first_index)
        .ok_or(EdgeRunRelocationError::UnsupportedPair)?;
    let run = &block.instructions[first_index..=last_index];
    // Only an unconditional semantic jump edge keeps the run's execution
    // count: every traversal of the run's block leaves through it, and —
    // with the single-predecessor rule below — every traversal of the
    // destination block arrives through it. A conditional terminator keeps
    // a second exit the run would still execute on after relocating.
    let SelectedTerminator::Jump {
        instruction: terminator,
        successor,
    } = &block.terminator
    else {
        return Err(EdgeRunRelocationError::UnsupportedPair);
    };
    // The crossed edge must be a plain semantic successor: case dispatch,
    // continuation, structural transfer, and per-edge fuel all carry
    // boundary effects this step does not cross.
    if !plain_edge(successor) {
        return Err(EdgeRunRelocationError::UnsupportedPair);
    }
    let target_index = function
        .blocks
        .iter()
        .position(|candidate| candidate.id == successor.block)
        .ok_or(EdgeRunRelocationError::SourceMismatch)?;
    let target = &function.blocks[target_index];
    // A self-edge is the in-block run family's case with a back-edge
    // transport reading, not a cross-edge window. The entry block is
    // reached with no predecessor at all, and an implementation block's
    // origin carries edge or case work the bounded audit does not cross.
    if target.id == block.id
        || target.id == function.entry_block
        || !matches!(target.origin, SelectedBlockOrigin::Source(_))
    {
        return Err(EdgeRunRelocationError::UnsupportedPair);
    }
    // Every edge into the destination block must be this one: a second
    // predecessor gives the block a path the run would newly execute on.
    if function
        .blocks
        .iter()
        .flat_map(|block| terminator_successors(&block.terminator))
        .filter(|edge| edge.block == target.id)
        .count()
        != 1
    {
        return Err(EdgeRunRelocationError::UnsupportedPair);
    }
    // The destination names a position in the target body — the run lands
    // at its index — or the target's terminator-carried instruction,
    // landing the run at the body end.
    let landing_index = target
        .instructions
        .iter()
        .position(|instruction| instruction.id == destination)
        .or_else(|| {
            (terminator_instruction(&target.terminator).id == destination)
                .then_some(target.instructions.len())
        })
        .ok_or(EdgeRunRelocationError::UnsupportedPair)?;
    // Every member meets the schedulable bar itself; the run's memory
    // accounting is the union of its members' roster rows.
    let mut run_accounted = false;
    for member in run {
        run_accounted |=
            schedulable(function, member).ok_or(EdgeRunRelocationError::UnsupportedInstruction)?;
    }
    // The edge's register transports sit between the run's old and new
    // positions: a member defining the transported argument would hand the
    // binding a stale value, a member defining the parameter would be
    // overwritten by it, and a member reading the parameter would observe
    // the transported value only after the move. Reading the argument is
    // harmless — the binding never writes it.
    for member in run {
        if transport_conflict(member, successor) {
            return Err(EdgeRunRelocationError::UnsupportedPair);
        }
    }
    // The `Jump` instruction itself is the crossed edge's position: it is
    // exempt from the barrier-kind rule but not from the hazard, call, or
    // memory accounting. Rows the roster records with the edge's own
    // origin count as the edge position's memory surface.
    if has_call_contract(function, terminator.id) {
        return Err(EdgeRunRelocationError::UnsupportedInstruction);
    }
    if run_accounted && edge_accounted(function, terminator, successor) {
        return Err(EdgeRunRelocationError::UnsupportedPair);
    }
    for member in run {
        if coupled(member, terminator) {
            return Err(EdgeRunRelocationError::UnsupportedPair);
        }
    }
    // Every member trades order with the positions behind the run in its
    // own body and the positions before the landing index in the
    // destination body. Every other position keeps the run on the side it
    // always had.
    for crossed in block.instructions[last_index + 1..]
        .iter()
        .chain(target.instructions[..landing_index].iter())
    {
        // Every crossed instruction meets the same schedulable bar as the
        // members: no barrier kind, no call contract, and no unaccounted
        // memory reach. Its roster rows may keep their relative order only
        // while no member records any — a row-carrying run passing a
        // second accounted actor would reorder recorded accesses.
        let crossed_accounted =
            schedulable(function, crossed).ok_or(EdgeRunRelocationError::UnsupportedInstruction)?;
        if run_accounted && crossed_accounted {
            return Err(EdgeRunRelocationError::UnsupportedPair);
        }
        for member in run {
            if coupled(member, crossed) {
                return Err(EdgeRunRelocationError::UnsupportedPair);
            }
        }
    }
    // A settlement positioned past the run's first index observed a member
    // inside the source block's executed prefix; a settlement positioned
    // past the landing index observes the run inside the destination's.
    // Both refuse; positions at or before either boundary keep the
    // executed set they always had.
    if function.boundary_settlements.iter().any(|settlement| {
        (settlement.block == block.id && settlement.instruction_index as usize > first_index)
            || (settlement.block == target.id
                && settlement.instruction_index as usize > landing_index)
    }) {
        return Err(EdgeRunRelocationError::UnsupportedPair);
    }
    // The scan walks every block body and terminator instruction once to
    // locate the run's first member and count predecessor edges; the
    // window audit walks every member-against-crossed operand and unit
    // surface, plus the function's three rosters and the edge's binding
    // roster once per member.
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
                block.instructions[last_index + 1..]
                    .iter()
                    .chain(target.instructions[..landing_index].iter())
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
        .ok_or(EdgeRunRelocationError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| EdgeRunRelocationError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(EdgeRunRelocationError::WorkBudgetExceeded);
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
