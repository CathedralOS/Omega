//! Shared admission for cross-edge relocation: locate the named `member`
//! in one block's body, require that block to end in an unconditional
//! `Jump` whose semantic successor is the destination's block, and prove
//! the window the move crosses independent — no register or
//! condition-state hazard between the member and any crossed position, no
//! interference with the edge's register transports, no roster-carrying
//! member sharing the window with a second memory-access actor, no
//! barrier, call, hosted effect, or call-roster entry inside the window,
//! and no boundary settlement whose observed executed prefix changes.
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{
    SelectedBlock, SelectedBlockOrigin, SelectedFunction, SelectedInstruction,
    SelectedInstructionId, SelectedSuccessor, SelectedSuccessorRole, SelectedTerminator,
    SelectedValueTransport,
};

use super::EdgeRelocationError;
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::window_hazards::{
    coupled, has_call_contract, has_memory_rows, register_reads, register_writes, schedulable,
    surface,
};

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

/// Every successor edge a terminator names; `HostedExitProcess` and
/// `Return` name none.
fn terminator_successors(block: &SelectedBlock) -> Vec<&SelectedSuccessor> {
    match &block.terminator {
        SelectedTerminator::Jump { successor, .. } => vec![successor],
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
        SelectedTerminator::HostedExitProcess { .. } | SelectedTerminator::Return { .. } => {
            Vec::new()
        }
    }
}

/// The instruction a terminator carries — the `Jump`'s own row is a
/// position the move crosses; conditional and return forms are refused
/// before this is reached.
fn terminator_instruction(block: &SelectedBlock) -> &SelectedInstruction {
    match &block.terminator {
        SelectedTerminator::HostedExitProcess { instruction, .. }
        | SelectedTerminator::Jump { instruction, .. }
        | SelectedTerminator::ConditionalBranch { instruction, .. }
        | SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. }
        | SelectedTerminator::ConditionalBranchI64LessThan { instruction, .. }
        | SelectedTerminator::Return { instruction, .. } => instruction,
    }
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
    let SelectedTerminator::Jump {
        instruction: terminator,
        successor,
    } = &block.terminator
    else {
        return Err(EdgeRelocationError::UnsupportedPair);
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
        .flat_map(terminator_successors)
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
            (terminator_instruction(target).id == destination).then_some(target.instructions.len())
        })
        .ok_or(EdgeRelocationError::UnsupportedPair)?;
    let member_accounted = schedulable(function, member_instruction)
        .ok_or(EdgeRelocationError::UnsupportedInstruction)?;
    // The edge's register transports sit between the member's old and new
    // positions: a member defining the transported argument would hand the
    // binding a stale value, a member defining the parameter would be
    // overwritten by it, and a member reading the parameter would observe
    // the transported value only after the move. Reading the argument is
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
            return Err(EdgeRelocationError::UnsupportedPair);
        }
    }
    // The `Jump` instruction itself is the crossed edge's position: it is
    // exempt from the barrier-kind rule but not from the hazard, call, or
    // memory accounting. Rows the roster records with the edge's own origin
    // count as the edge position's memory surface.
    if has_call_contract(function, terminator.id) {
        return Err(EdgeRelocationError::UnsupportedInstruction);
    }
    let edge_accounted = has_memory_rows(function, terminator.id)
        || function.memory_accesses.iter().any(|access| {
            access.origin
                == selected_instructions::SelectedMemoryAccessOrigin::Edge(successor.psi_edge)
        });
    if member_accounted && edge_accounted {
        return Err(EdgeRelocationError::UnsupportedPair);
    }
    if coupled(member_instruction, terminator) {
        return Err(EdgeRelocationError::UnsupportedPair);
    }
    // The member trades order with the positions behind it in its own body
    // and the positions before the landing index in the destination body.
    // Every other position keeps the member on the side it always had.
    for crossed in block.instructions[member_index + 1..]
        .iter()
        .chain(target.instructions[..landing_index].iter())
    {
        let crossed_accounted =
            schedulable(function, crossed).ok_or(EdgeRelocationError::UnsupportedInstruction)?;
        if member_accounted && crossed_accounted {
            return Err(EdgeRelocationError::UnsupportedPair);
        }
        if coupled(member_instruction, crossed) {
            return Err(EdgeRelocationError::UnsupportedPair);
        }
    }
    // A settlement positioned past the member's index observed it inside
    // the source block's prefix; a settlement positioned past the landing
    // index observes it inside the destination's. Both refuse; positions at
    // or before either boundary keep the executed set they always had.
    if function.boundary_settlements.iter().any(|settlement| {
        (settlement.block == block.id && settlement.instruction_index as usize > member_index)
            || (settlement.block == target.id
                && settlement.instruction_index as usize > landing_index)
    }) {
        return Err(EdgeRelocationError::UnsupportedPair);
    }
    // The scan walks every block body and terminator instruction once to
    // locate the member and count predecessor edges; the window audit walks
    // the member's surface against each crossed position's, plus the
    // function's three rosters and the edge's binding roster.
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
                    .chain(std::iter::once(terminator_instruction(candidate)))
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
