//! Independent replay of the fork relocation: the validator reconstructs
//! the member's branch block, the landing arm the destination names, and
//! the landing index inside it from the source on its own audit — sharing
//! no state with the producer's `admission` record — then requires the
//! proposed program to place exactly the member on the landing index in
//! the arm block and restores the complete source by content — every
//! crossed instruction, every other block and instruction, register,
//! roster row, call, settlement, and edge included.
use std::sync::Arc;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{
    SelectedBlock, SelectedBlockOrigin, SelectedFunction, SelectedInstruction,
    SelectedInstructionId, SelectedInstructionPlan, SelectedSuccessor, SelectedTerminator,
};
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{ForkRelocationError, ForkRelocationReceipt, ValidatedForkRelocation};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::{
    all_edges, edge_surface, plain_edge, terminator_instruction, terminator_successors,
    transport_conflict,
};
use crate::rewrites::dead_path;
use crate::rewrites::window_hazards::{
    coupled, has_call_contract, register_writes, schedulable, surface,
};

/// The validator's own reconstruction of the relocation the contract
/// permits: the member's branch block and position, the landing arm, and
/// the landing index. It shares no state with the producer's `admission`
/// record.
struct Reconstructed<'source> {
    function: &'source SelectedFunction,
    block_index: usize,
    member_index: usize,
    target_index: usize,
    landing_index: usize,
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
            | ExactDivideI64 { .. }
            | ExactRemainderI64 { .. }
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

/// Reconstruct the legality of sinking `member` onto `destination` through
/// the branch fork from the source records: locate the member by identity,
/// re-derive the two-successor conditional the member's block must end in,
/// the arm the destination selects, and the landing index, then re-run the
/// fork's own audit — the member's schedulable and sinkable kind gates,
/// every crossed position's schedulability and coupling against the
/// member, each landing edge's plain-role, call-roster, coupling, and
/// transport checks, the boundary-settlement refusals, and the dead-path
/// audit over the member's writes on every skipped edge — and account the
/// family's measured steps against the budget. Nothing in this audit reads
/// the producer's admission decision, so a producer-side legality error
/// fails here even when the proposal matches the emitted edit.
fn reconstruct<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Reconstructed<'source>, ForkRelocationError> {
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
        | SelectedTerminator::Crash { .. }
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
    // between the member's old and new positions. The remaining branch
    // edges are the skipped paths the member never traverses — their
    // surfaces join only the dead-path audit.
    let mut landing_edges: Vec<&SelectedSuccessor> = Vec::new();
    let mut skipped_edges: Vec<&SelectedSuccessor> = Vec::new();
    for edge in &branch_edges {
        if edge.block == target.id {
            landing_edges.push(*edge);
        } else {
            skipped_edges.push(*edge);
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
    // The member trades order with the positions behind it in its own
    // block and before the landing index in the arm, and with each landing
    // edge — carried by the branch terminator as the edge's own position.
    // A roster-carrying crossed position is an accounted access the
    // row-less member cannot reorder, so only the schedulability and
    // hazard gates apply: the member carries no memory rows, so the
    // memory-ordering refusal the shared audit owns cannot fire here.
    for crossed in block.instructions[member_index + 1..]
        .iter()
        .chain(target.instructions[..landing_index].iter())
    {
        schedulable(function, crossed).ok_or(ForkRelocationError::UnsupportedInstruction)?;
        if coupled(member_instruction, crossed) {
            return Err(ForkRelocationError::UnsupportedPair);
        }
    }
    for edge in &landing_edges {
        if !plain_edge(edge) {
            return Err(ForkRelocationError::UnsupportedPair);
        }
        if has_call_contract(function, terminator.id) {
            return Err(ForkRelocationError::UnsupportedInstruction);
        }
        if coupled(member_instruction, terminator) {
            return Err(ForkRelocationError::UnsupportedPair);
        }
        if transport_conflict(member_instruction, edge) {
            return Err(ForkRelocationError::UnsupportedPair);
        }
    }
    // A settlement positioned past the member's index observed it inside
    // the branch block's executed prefix; a settlement positioned past
    // the landing index observes it inside the arm's — where it has not
    // run yet. Both refuse; positions at or before either boundary keep
    // the executed set they always had, and settlements on the skipped
    // paths never had the member in their streams.
    if function.boundary_settlements.iter().any(|settlement| {
        (settlement.block == block.id && settlement.instruction_index as usize > member_index)
            || (settlement.block == target.id
                && settlement.instruction_index as usize > landing_index)
    }) {
        return Err(ForkRelocationError::UnsupportedPair);
    }
    // The dead-path audit: every location the member writes must be dead —
    // unread until rewritten — along every path leaving the branch's other
    // edges. The vacated index stays silent: every position behind it in
    // the head that could observe the missing write is a crossed window
    // position the hazard audit already owns.
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
    Ok(Reconstructed {
        function,
        block_index,
        member_index,
        target_index,
        landing_index,
    })
}

/// Independently consume the proposed program: the validator's own
/// reconstruction re-derives the member's block, the landing arm, and the
/// landing index from the source, the proposal must place exactly the
/// member's instruction on the landing index in the arm block, and moving
/// it back must restore the complete source by content — every crossed
/// instruction, every other block and instruction, register, roster row,
/// call, settlement, and edge included.
pub fn validate_fork_relocation(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedForkRelocation, ForkRelocationError> {
    let reconstructed = reconstruct(
        source,
        function_index,
        member,
        destination,
        environment,
        budget,
    )?;
    let source_member = &reconstructed.function.blocks[reconstructed.block_index].instructions
        [reconstructed.member_index];
    if proposed
        .functions
        .get(function_index)
        .and_then(|function| function.blocks.get(reconstructed.target_index))
        .and_then(|block| block.instructions.get(reconstructed.landing_index))
        != Some(source_member)
    {
        return Err(ForkRelocationError::ReplayMismatch);
    }
    let mut restored = proposed.clone();
    let member_instruction = restored.functions[function_index].blocks[reconstructed.target_index]
        .instructions
        .remove(reconstructed.landing_index);
    restored.functions[function_index].blocks[reconstructed.block_index]
        .instructions
        .insert(reconstructed.member_index, member_instruction);
    if restored != *source.selected_plan() {
        return Err(ForkRelocationError::ReplayMismatch);
    }
    Ok(ValidatedForkRelocation {
        receipt: ForkRelocationReceipt {
            source_selected: source.selected_identity(),
            transformed_selected: selected_instruction_plan_identity(&proposed),
            optimization_unit: source.optimization_unit_identity(),
            fuel_schedule: source.fuel_schedule_identity(),
        },
        transformed: Arc::new(proposed),
    })
}
