//! Shared admission for diamond relocation: locate the named `member` in
//! one block's body, require that block to end in a two-successor
//! conditional branch whose distinct targets — the arms — are each reached
//! by that branch's edges alone and each end in an unconditional `Jump` to
//! one common join reached by arm edges alone, and prove the window the
//! move crosses independent — no register or condition-state hazard
//! between the member and any crossed position, no interference with any
//! crossed edge's register transports, no roster-carrying member sharing
//! the window with a second memory-access actor, no barrier, call, hosted
//! effect, or call-roster entry inside the window, and no boundary
//! settlement whose observed executed prefix changes.
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{
    SelectedBlock, SelectedBlockId, SelectedBlockOrigin, SelectedFunction, SelectedInstruction,
    SelectedInstructionId, SelectedSuccessor, SelectedSuccessorRole, SelectedTerminator,
    SelectedValueTransport,
};

use super::DiamondRelocationError;
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::window_hazards::{
    coupled, has_call_contract, has_memory_rows, register_reads, register_writes, schedulable,
    surface,
};

pub(super) struct Admission<'source> {
    pub function: &'source SelectedFunction,
    /// The member's own block: the diamond's branching head.
    pub block_index: usize,
    /// The member's index inside that block's body.
    pub member_index: usize,
    /// The join block's index in `function.blocks`.
    pub target_index: usize,
    /// The destination instruction's position in the join body: the member
    /// lands at this index. Naming the join's terminator-carried
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

/// The instruction a terminator carries. The branch and jump instructions
/// the window's edges ride on are positions the move crosses; every form
/// is returned so predecessor scans see a uniform surface.
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

/// Every successor edge in the function paired with the block it leaves,
/// for predecessor-count audits.
fn all_edges(
    function: &SelectedFunction,
) -> impl Iterator<Item = (SelectedBlockId, &SelectedSuccessor)> {
    function.blocks.iter().flat_map(|block| {
        terminator_successors(block)
            .into_iter()
            .map(move |edge| (block.id, edge))
    })
}

/// A plain semantic successor edge: case dispatch, continuation,
/// structural transfer, and per-edge fuel all carry boundary effects this
/// step does not cross. Structural bindings may remain only while every
/// transport is `Unused`, which moves nothing.
fn plain_edge(successor: &SelectedSuccessor) -> bool {
    successor.role == SelectedSuccessorRole::Semantic
        && successor.structural_case.is_none()
        && successor.fuel.is_empty()
        && successor.structural_bindings.iter().all(|binding| {
            binding.transport == selected_instructions::SelectedStructuralTransport::Unused
        })
}

/// The member must not interfere with one crossed edge's register
/// transports: a member defining the transported argument would hand the
/// binding a stale value, a member defining the parameter would be
/// overwritten by it, and a member reading the parameter would observe the
/// transported value only after the move. Reading the argument is harmless
/// — the binding never writes it.
fn transport_conflict(member: &SelectedInstruction, successor: &SelectedSuccessor) -> bool {
    successor.bindings.iter().any(|binding| {
        matches!(
            binding.transport,
            SelectedValueTransport::Registers { argument, parameter }
                if register_writes(member)
                    .any(|register| register == argument || register == parameter)
                    || register_reads(member).any(|register| register == parameter))
    })
}

/// The roster surface one crossed edge position carries: rows the
/// terminator instruction itself records plus any rows the roster logs
/// with the edge's own origin.
fn edge_accounted(
    function: &SelectedFunction,
    terminator: &SelectedInstruction,
    successor: &SelectedSuccessor,
) -> bool {
    has_memory_rows(function, terminator.id)
        || function.memory_accesses.iter().any(|access| {
            access.origin
                == selected_instructions::SelectedMemoryAccessOrigin::Edge(successor.psi_edge)
        })
}

pub(super) fn admit<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Admission<'source>, DiamondRelocationError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(DiamondRelocationError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(DiamondRelocationError::SourceMismatch)?;
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
        .ok_or(DiamondRelocationError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    let member_instruction = &block.instructions[member_index];
    // Only a two-successor conditional terminator gives the member's block
    // the branch this step crosses; every other terminator shape is the
    // single-edge family's case or no relocation at all.
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
        | SelectedTerminator::HostedExitProcess { .. }
        | SelectedTerminator::Return { .. } => {
            return Err(DiamondRelocationError::UnsupportedPair);
        }
    };
    let terminator = terminator_instruction(block);
    for edge in &branch_edges {
        if !plain_edge(edge) {
            return Err(DiamondRelocationError::UnsupportedPair);
        }
    }
    // The arms are the branch's distinct targets: both edges naming one
    // block is a degenerate diamond with a single arm. Each arm must be a
    // plain source block the branch alone reaches — a second predecessor
    // would give the join a path the member never executed on, the member's
    // own block would make the move self-referential, the entry block is
    // reached with no predecessor at all, and an implementation block's
    // origin carries edge or case work the bounded audit does not cross.
    let mut arm_indices: Vec<usize> = Vec::new();
    for edge in &branch_edges {
        let arm_index = function
            .blocks
            .iter()
            .position(|candidate| candidate.id == edge.block)
            .ok_or(DiamondRelocationError::SourceMismatch)?;
        if !arm_indices.contains(&arm_index) {
            arm_indices.push(arm_index);
        }
    }
    for &arm_index in &arm_indices {
        let arm = &function.blocks[arm_index];
        if arm.id == block.id
            || arm.id == function.entry_block
            || !matches!(arm.origin, SelectedBlockOrigin::Source(_))
        {
            return Err(DiamondRelocationError::UnsupportedPair);
        }
        let branch_edges_into_arm = branch_edges
            .iter()
            .filter(|edge| edge.block == arm.id)
            .count();
        if all_edges(function)
            .filter(|(_, edge)| edge.block == arm.id)
            .count()
            != branch_edges_into_arm
        {
            return Err(DiamondRelocationError::UnsupportedPair);
        }
    }
    // Each arm must leave through an unconditional `Jump` to one common
    // join on a plain semantic edge — the converging side of the diamond.
    let mut target_index: Option<usize> = None;
    let mut arm_edges: Vec<&SelectedSuccessor> = Vec::new();
    let mut arm_terminators: Vec<&SelectedInstruction> = Vec::new();
    for &arm_index in &arm_indices {
        let arm = &function.blocks[arm_index];
        let SelectedTerminator::Jump {
            instruction: arm_terminator,
            successor: arm_edge,
        } = &arm.terminator
        else {
            return Err(DiamondRelocationError::UnsupportedPair);
        };
        if !plain_edge(arm_edge) {
            return Err(DiamondRelocationError::UnsupportedPair);
        }
        let join_index = function
            .blocks
            .iter()
            .position(|candidate| candidate.id == arm_edge.block)
            .ok_or(DiamondRelocationError::SourceMismatch)?;
        match target_index {
            Some(existing) if existing != join_index => {
                return Err(DiamondRelocationError::UnsupportedPair);
            }
            Some(_) => {}
            None => target_index = Some(join_index),
        }
        arm_edges.push(arm_edge);
        arm_terminators.push(arm_terminator);
    }
    let target_index = target_index.ok_or(DiamondRelocationError::UnsupportedPair)?;
    let target = &function.blocks[target_index];
    // The join must not collapse back into the diamond: landing inside the
    // member's own block or inside an arm is an in-block or self-loop move
    // this step does not cross, the entry block is reached with no
    // predecessor at all, and an implementation block's origin carries edge
    // or case work the bounded audit does not cross.
    if target.id == block.id
        || target.id == function.entry_block
        || arm_indices
            .iter()
            .any(|&arm| function.blocks[arm].id == target.id)
        || !matches!(target.origin, SelectedBlockOrigin::Source(_))
    {
        return Err(DiamondRelocationError::UnsupportedPair);
    }
    // Every edge into the join must leave an arm: a second predecessor
    // gives the join a path the member would newly execute on.
    let mut join_incoming = 0usize;
    for (source_block, _) in all_edges(function).filter(|(_, edge)| edge.block == target.id) {
        if !arm_indices
            .iter()
            .any(|&arm| function.blocks[arm].id == source_block)
        {
            return Err(DiamondRelocationError::UnsupportedPair);
        }
        join_incoming += 1;
    }
    if join_incoming != arm_indices.len() {
        return Err(DiamondRelocationError::UnsupportedPair);
    }
    // The destination names a position in the join body — the member lands
    // at its index — or the join's terminator-carried instruction, landing
    // the member at the body end.
    let landing_index = target
        .instructions
        .iter()
        .position(|instruction| instruction.id == destination)
        .or_else(|| {
            (terminator_instruction(target).id == destination).then_some(target.instructions.len())
        })
        .ok_or(DiamondRelocationError::UnsupportedPair)?;
    let member_accounted = schedulable(function, member_instruction)
        .ok_or(DiamondRelocationError::UnsupportedInstruction)?;
    // Every crossed edge's register transports sit between the member's old
    // and new positions — the two branch edges and each arm's jump edge.
    for edge in branch_edges
        .iter()
        .copied()
        .chain(arm_edges.iter().copied())
    {
        if transport_conflict(member_instruction, edge) {
            return Err(DiamondRelocationError::UnsupportedPair);
        }
    }
    // The branch terminator plus its two edges is the first crossed
    // position: it is exempt from the barrier-kind rule but not from the
    // call, hazard, or memory accounting.
    if has_call_contract(function, terminator.id) {
        return Err(DiamondRelocationError::UnsupportedInstruction);
    }
    let branch_accounted = branch_edges
        .iter()
        .copied()
        .any(|edge| edge_accounted(function, terminator, edge));
    if member_accounted && branch_accounted {
        return Err(DiamondRelocationError::UnsupportedPair);
    }
    if coupled(member_instruction, terminator) {
        return Err(DiamondRelocationError::UnsupportedPair);
    }
    // Each arm's whole body sits inside the window, then its `Jump`
    // terminator and outgoing edge form that arm's boundary position.
    for position in 0..arm_indices.len() {
        let arm = &function.blocks[arm_indices[position]];
        let arm_edge = arm_edges[position];
        let arm_terminator = arm_terminators[position];
        for crossed in &arm.instructions {
            let crossed_accounted = schedulable(function, crossed)
                .ok_or(DiamondRelocationError::UnsupportedInstruction)?;
            if member_accounted && crossed_accounted {
                return Err(DiamondRelocationError::UnsupportedPair);
            }
            if coupled(member_instruction, crossed) {
                return Err(DiamondRelocationError::UnsupportedPair);
            }
        }
        if has_call_contract(function, arm_terminator.id) {
            return Err(DiamondRelocationError::UnsupportedInstruction);
        }
        let arm_boundary_accounted = edge_accounted(function, arm_terminator, arm_edge);
        if member_accounted && arm_boundary_accounted {
            return Err(DiamondRelocationError::UnsupportedPair);
        }
        if coupled(member_instruction, arm_terminator) {
            return Err(DiamondRelocationError::UnsupportedPair);
        }
    }
    // The member trades order with the positions behind it in its own body
    // and the positions before the landing index in the join body. Every
    // other position keeps the member on the side it always had.
    for crossed in block.instructions[member_index + 1..]
        .iter()
        .chain(target.instructions[..landing_index].iter())
    {
        let crossed_accounted =
            schedulable(function, crossed).ok_or(DiamondRelocationError::UnsupportedInstruction)?;
        if member_accounted && crossed_accounted {
            return Err(DiamondRelocationError::UnsupportedPair);
        }
        if coupled(member_instruction, crossed) {
            return Err(DiamondRelocationError::UnsupportedPair);
        }
    }
    // A settlement positioned past the member's index observed it inside
    // the source block's executed prefix; a settlement positioned past the
    // landing index observes it inside the join's. Both refuse; positions
    // at or before either boundary keep the executed set they always had.
    // Arm blocks are unaffected: the member never enters an arm's body, so
    // no arm prefix ever contained or loses it.
    if function.boundary_settlements.iter().any(|settlement| {
        (settlement.block == block.id && settlement.instruction_index as usize > member_index)
            || (settlement.block == target.id
                && settlement.instruction_index as usize > landing_index)
    }) {
        return Err(DiamondRelocationError::UnsupportedPair);
    }
    // The scan walks every block body and terminator instruction once to
    // locate the member, and again with successor edges to count
    // predecessor edges; the window audit walks the member's surface
    // against each crossed position's, plus the function's three rosters
    // and every crossed edge's binding roster.
    let mut crossed_instructions = block.instructions[member_index + 1..]
        .iter()
        .chain(arm_indices.iter().flat_map(|&arm| {
            function.blocks[arm]
                .instructions
                .iter()
                .chain(std::iter::once(terminator_instruction(
                    &function.blocks[arm],
                )))
        }))
        .chain(target.instructions[..landing_index].iter())
        .chain(std::iter::once(terminator));
    let crossed_surfaces = crossed_instructions
        .try_fold(0usize, |total, crossed| {
            total
                .checked_add(surface(member_instruction))?
                .checked_add(surface(crossed))
        })
        .ok_or(DiamondRelocationError::IdentityOverflow)?;
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
        .and_then(|total| total.checked_add(crossed_surfaces))
        .and_then(|total| {
            total
                .checked_add(function.memory_accesses.len())?
                .checked_add(function.calls.len())?
                .checked_add(function.boundary_settlements.len())
        })
        .and_then(|total| {
            branch_edges
                .iter()
                .chain(arm_edges.iter())
                .try_fold(total, |total, edge| total.checked_add(edge.bindings.len()))
        })
        .ok_or(DiamondRelocationError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| DiamondRelocationError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(DiamondRelocationError::WorkBudgetExceeded);
    }
    Ok(Admission {
        function,
        block_index,
        member_index,
        target_index,
        landing_index,
    })
}
