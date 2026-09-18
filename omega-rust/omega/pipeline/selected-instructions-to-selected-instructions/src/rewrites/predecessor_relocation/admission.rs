//! Shared admission for relocation into the sole predecessor: locate the
//! named `member` in one block's body, require that block to be reached by
//! exactly one edge — the unconditional `Jump` the destination block ends
//! in — and prove the window the move crosses independent: no register or
//! condition-state hazard between the member and any crossed position, no
//! interference with the edge's register transports, no roster-carrying
//! member sharing the window with a second memory-access actor, no
//! barrier, call, hosted effect, or call-roster entry inside the window,
//! and no boundary settlement whose observed executed prefix changes.
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{
    SelectedBlockOrigin, SelectedFunction, SelectedInstructionId, SelectedSuccessorRole,
    SelectedTerminator, SelectedValueTransport,
};

use super::PredecessorRelocationError;
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::{terminator_instruction, terminator_successors};
use crate::rewrites::window_hazards::{
    coupled, has_call_contract, has_memory_rows, register_reads, register_writes, schedulable,
    surface,
};

pub(super) struct Admission<'source> {
    pub function: &'source SelectedFunction,
    /// The member's own block — the edge's successor.
    pub block_index: usize,
    /// The member's index inside that block's body.
    pub member_index: usize,
    /// The destination block's index in `function.blocks` — the sole
    /// predecessor the edge leaves.
    pub target_index: usize,
    /// The destination instruction's position in the predecessor body: the
    /// member lands at this index. Naming the predecessor's
    /// terminator-carried `Jump` instruction lands the member at the body
    /// end, index `target.instructions.len()`.
    pub landing_index: usize,
}

pub(super) fn admit<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Admission<'source>, PredecessorRelocationError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(PredecessorRelocationError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(PredecessorRelocationError::SourceMismatch)?;
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
        .ok_or(PredecessorRelocationError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    let member_instruction = &block.instructions[member_index];
    // Every edge into the member's block must be the one crossed edge: a
    // second predecessor gives the block a path the member would stop
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
        return Err(PredecessorRelocationError::UnsupportedPair);
    };
    if incoming.next().is_some() {
        return Err(PredecessorRelocationError::UnsupportedPair);
    }
    let target = &function.blocks[target_index];
    // Only an unconditional jump keeps the member's execution count: every
    // traversal of the predecessor leaves through it into the member's
    // block, so the member executes once per predecessor traversal on
    // either side of the move. A conditional predecessor keeps a second
    // exit the member would newly execute on after relocating. The found
    // edge is this terminator's one successor, so `successor` names the
    // crossed edge directly.
    let SelectedTerminator::Jump {
        instruction: terminator,
        successor,
    } = &target.terminator
    else {
        return Err(PredecessorRelocationError::UnsupportedPair);
    };
    // The crossed edge must be a plain semantic successor: case dispatch,
    // continuation, structural transfer, and per-edge fuel all carry
    // boundary effects this step does not cross. Structural bindings may
    // remain only while every transport is `Unused`, which moves nothing.
    if successor.role != SelectedSuccessorRole::Semantic
        || successor.structural_case.is_some()
        || !successor.fuel.is_empty()
        || successor.structural_bindings.iter().any(|binding| {
            binding.transport != selected_instructions::SelectedStructuralTransport::Unused
        })
    {
        return Err(PredecessorRelocationError::UnsupportedPair);
    }
    // A self-edge is the in-block family's case with a back-edge transport
    // reading, not a cross-edge window. The entry block is reached with no
    // predecessor at all — an edge naming it would feed it a traversal
    // entry never saw — and an implementation block's origin carries edge
    // or case work the bounded audit does not cross.
    if target.id == block.id
        || block.id == function.entry_block
        || !matches!(target.origin, SelectedBlockOrigin::Source(_))
    {
        return Err(PredecessorRelocationError::UnsupportedPair);
    }
    // The destination names a position in the predecessor body — the
    // member lands at its index — or the predecessor's terminator-carried
    // `Jump` instruction, landing the member at the body end.
    let landing_index = target
        .instructions
        .iter()
        .position(|instruction| instruction.id == destination)
        .or_else(|| {
            (terminator_instruction(&target.terminator).id == destination)
                .then_some(target.instructions.len())
        })
        .ok_or(PredecessorRelocationError::UnsupportedPair)?;
    let member_accounted = schedulable(function, member_instruction)
        .ok_or(PredecessorRelocationError::UnsupportedInstruction)?;
    // The edge's register transports sit between the member's new and old
    // positions: a member defining the transported argument would hand the
    // binding a new value where the source bound the old, a member
    // defining the parameter would be overwritten before the member's
    // block observes it, and a member reading the parameter would observe
    // the pre-transport value after the move. Reading the argument is
    // harmless — the binding never writes it.
    for binding in &successor.bindings {
        if let SelectedValueTransport::Registers {
            argument,
            parameter,
        } = binding.transport
            && (register_writes(member_instruction)
                .any(|register| register == argument || register == parameter)
                || register_reads(member_instruction).any(|register| register == parameter))
        {
            return Err(PredecessorRelocationError::UnsupportedPair);
        }
    }
    // The `Jump` instruction itself is the crossed edge's position: it is
    // exempt from the barrier-kind rule but not from the hazard, call, or
    // memory accounting. Rows the roster records with the edge's own origin
    // count as the edge position's memory surface.
    if has_call_contract(function, terminator.id) {
        return Err(PredecessorRelocationError::UnsupportedInstruction);
    }
    let edge_accounted = has_memory_rows(function, terminator.id)
        || function.memory_accesses.iter().any(|access| {
            access.origin
                == selected_instructions::SelectedMemoryAccessOrigin::Edge(successor.psi_edge)
        });
    if member_accounted && edge_accounted {
        return Err(PredecessorRelocationError::UnsupportedPair);
    }
    if coupled(member_instruction, terminator) {
        return Err(PredecessorRelocationError::UnsupportedPair);
    }
    // The member trades order with the positions at and after the landing
    // index in the predecessor body — they ran before it and now run after
    // — and the positions before it in its own body, which now follow it
    // across the boundary. Every other position keeps the member on the
    // side it always had.
    for crossed in target.instructions[landing_index..]
        .iter()
        .chain(block.instructions[..member_index].iter())
    {
        let crossed_accounted = schedulable(function, crossed)
            .ok_or(PredecessorRelocationError::UnsupportedInstruction)?;
        if member_accounted && crossed_accounted {
            return Err(PredecessorRelocationError::UnsupportedPair);
        }
        if coupled(member_instruction, crossed) {
            return Err(PredecessorRelocationError::UnsupportedPair);
        }
    }
    // A settlement positioned past the landing index observes the member
    // inside the destination block's prefix; a settlement positioned past
    // the member's index observed it inside the member's own block's. Both
    // refuse; positions at or before either boundary keep the executed set
    // they always had.
    if function.boundary_settlements.iter().any(|settlement| {
        (settlement.block == block.id && settlement.instruction_index as usize > member_index)
            || (settlement.block == target.id
                && settlement.instruction_index as usize > landing_index)
    }) {
        return Err(PredecessorRelocationError::UnsupportedPair);
    }
    // The scan walks every block body and terminator instruction once to
    // locate the member and count the block's predecessor edges; the
    // window audit walks the member's surface against each crossed
    // position's, plus the function's three rosters and the edge's binding
    // roster.
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
                .checked_add(function.boundary_settlements.len())?
                .checked_add(successor.bindings.len())
        })
        .ok_or(PredecessorRelocationError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| PredecessorRelocationError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(PredecessorRelocationError::WorkBudgetExceeded);
    }
    Ok(Admission {
        function,
        block_index,
        member_index,
        target_index,
        landing_index,
    })
}
