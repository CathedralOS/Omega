//! Shared admission for join relocation: locate the named `member` in one
//! block's body, require that block to be a converging join — every edge
//! into it leaving an arm whose terminator is a plain unconditional
//! `Jump` back — whose arms are reached by the edges of one common fork
//! head alone, require that head's two-successor conditional terminator
//! to name only arms, and prove the window the move crosses independent —
//! no register or condition-state hazard between the member and any
//! crossed position, no interference with any crossed edge's register
//! transports, no roster-carrying member sharing the window with a second
//! memory-access actor, no barrier, call, hosted effect, or call-roster
//! entry inside the window, and no boundary settlement whose observed
//! executed prefix changes.
//!
//! Where the sinking families prove a member's written locations dead on
//! the paths it stops traversing, this direction proves supply instead:
//! every edge into the join leaves an arm the head alone feeds, and every
//! head edge reaches an arm, so each traversal into the join passes the
//! member's new position and each traversal of the head reaches the join.
//! The member keeps its execution count of one and needs no dead-path
//! audit — the predecessor and successor bookkeeping is the totality
//! proof.
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{
    SelectedBlock, SelectedBlockId, SelectedBlockOrigin, SelectedFunction, SelectedInstruction,
    SelectedInstructionId, SelectedSuccessor, SelectedSuccessorRole, SelectedTerminator,
    SelectedValueTransport,
};

use super::JoinRelocationError;
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::window_hazards::{
    coupled, has_call_contract, has_memory_rows, register_reads, register_writes, schedulable,
    surface,
};

pub(super) struct Admission<'source> {
    pub function: &'source SelectedFunction,
    /// The member's own block: the diamond's converging join.
    pub block_index: usize,
    /// The member's index inside that block's body.
    pub member_index: usize,
    /// The fork head's index in `function.blocks`.
    pub target_index: usize,
    /// The destination instruction's position in the head body: the
    /// member lands at this index. Naming the head's terminator-carried
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
/// binding a value it never carried before the move, a member defining
/// the parameter would be overwritten by it, and a member reading the
/// parameter would observe the transported value only on one side of the
/// move. Reading the argument is harmless — the binding never writes it.
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
) -> Result<Admission<'source>, JoinRelocationError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(JoinRelocationError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(JoinRelocationError::SourceMismatch)?;
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
        .ok_or(JoinRelocationError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    let member_instruction = &block.instructions[member_index];
    // The member's block must be a converging join: every edge into it
    // leaves an arm — a plain source block, distinct from the join and the
    // entry, whose terminator is an unconditional `Jump` to the join on a
    // plain semantic edge. A predecessor shaped any other way hands the
    // join a path the member's new position cannot supply, and a join
    // with no predecessors is unreachable: hoisting the member would
    // start its execution.
    if block.id == function.entry_block || !matches!(block.origin, SelectedBlockOrigin::Source(_)) {
        return Err(JoinRelocationError::UnsupportedPair);
    }
    let mut arm_indices: Vec<usize> = Vec::new();
    let mut arm_edges: Vec<&SelectedSuccessor> = Vec::new();
    let mut arm_terminators: Vec<&SelectedInstruction> = Vec::new();
    for (source_block, _) in all_edges(function).filter(|(_, edge)| edge.block == block.id) {
        let arm_index = function
            .blocks
            .iter()
            .position(|candidate| candidate.id == source_block)
            .ok_or(JoinRelocationError::SourceMismatch)?;
        let arm = &function.blocks[arm_index];
        if arm.id == block.id
            || arm.id == function.entry_block
            || !matches!(arm.origin, SelectedBlockOrigin::Source(_))
        {
            return Err(JoinRelocationError::UnsupportedPair);
        }
        // The predecessor's edge into the join is its `Jump` successor
        // itself: a conditional or non-plain edge into the join is a
        // converging path this step does not cross.
        let SelectedTerminator::Jump {
            instruction: arm_terminator,
            successor: arm_edge,
        } = &arm.terminator
        else {
            return Err(JoinRelocationError::UnsupportedPair);
        };
        if !plain_edge(arm_edge) {
            return Err(JoinRelocationError::UnsupportedPair);
        }
        arm_indices.push(arm_index);
        arm_edges.push(arm_edge);
        arm_terminators.push(arm_terminator);
    }
    if arm_indices.is_empty() {
        return Err(JoinRelocationError::UnsupportedPair);
    }
    // The arms must descend from one fork head alone: every edge into
    // every arm leaves the same block. A second predecessor into an arm
    // would hand the join a path that never crossed the member's new
    // position — the partial-supply failure the totality rule refuses.
    let mut target_index: Option<usize> = None;
    for &arm_index in &arm_indices {
        for (source_block, _) in
            all_edges(function).filter(|(_, edge)| edge.block == function.blocks[arm_index].id)
        {
            let head_index = function
                .blocks
                .iter()
                .position(|candidate| candidate.id == source_block)
                .ok_or(JoinRelocationError::SourceMismatch)?;
            match target_index {
                Some(existing) if existing != head_index => {
                    return Err(JoinRelocationError::UnsupportedPair);
                }
                Some(_) => {}
                None => target_index = Some(head_index),
            }
        }
    }
    let target_index = target_index.ok_or(JoinRelocationError::UnsupportedPair)?;
    let target = &function.blocks[target_index];
    // The head must not collapse back into the region: landing inside the
    // join or inside an arm is an in-block or self-loop move this step
    // does not cross, and an implementation block's origin carries edge or
    // case work the bounded audit does not cross.
    if target.id == block.id
        || arm_indices
            .iter()
            .any(|&arm| function.blocks[arm].id == target.id)
        || !matches!(target.origin, SelectedBlockOrigin::Source(_))
    {
        return Err(JoinRelocationError::UnsupportedPair);
    }
    // Only a two-successor conditional terminator gives the landing block
    // the fork this step rises through; every other terminator shape is
    // the single-edge family's case or no relocation at all.
    let branch_edges: Vec<&SelectedSuccessor> = match &target.terminator {
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
            return Err(JoinRelocationError::UnsupportedPair);
        }
    };
    // Every edge the head names must reach an arm: an edge leaving the
    // region would run the member on a traversal the join never saw.
    for edge in &branch_edges {
        if !plain_edge(edge)
            || !arm_indices
                .iter()
                .any(|&arm| function.blocks[arm].id == edge.block)
        {
            return Err(JoinRelocationError::UnsupportedPair);
        }
    }
    let terminator = terminator_instruction(target);
    // The destination names a position in the head body — the member lands
    // at its index — or the head's terminator-carried instruction,
    // landing the member at the body end.
    let landing_index = target
        .instructions
        .iter()
        .position(|instruction| instruction.id == destination)
        .or_else(|| (terminator.id == destination).then_some(target.instructions.len()))
        .ok_or(JoinRelocationError::UnsupportedPair)?;
    let member_accounted = schedulable(function, member_instruction)
        .ok_or(JoinRelocationError::UnsupportedInstruction)?;
    // Every crossed edge's register transports sit between the member's
    // old and new positions — the arm jump edges and the two branch edges.
    for edge in arm_edges
        .iter()
        .copied()
        .chain(branch_edges.iter().copied())
    {
        if transport_conflict(member_instruction, edge) {
            return Err(JoinRelocationError::UnsupportedPair);
        }
    }
    // The branch terminator plus its two edges is the last crossed
    // position: it is exempt from the barrier-kind rule but not from the
    // call, hazard, or memory accounting.
    if has_call_contract(function, terminator.id) {
        return Err(JoinRelocationError::UnsupportedInstruction);
    }
    let branch_accounted = branch_edges
        .iter()
        .copied()
        .any(|edge| edge_accounted(function, terminator, edge));
    if member_accounted && branch_accounted {
        return Err(JoinRelocationError::UnsupportedPair);
    }
    if coupled(member_instruction, terminator) {
        return Err(JoinRelocationError::UnsupportedPair);
    }
    // Each arm's whole body sits inside the window, then its `Jump`
    // terminator and outgoing edge form that arm's boundary position.
    for position in 0..arm_indices.len() {
        let arm = &function.blocks[arm_indices[position]];
        let arm_edge = arm_edges[position];
        let arm_terminator = arm_terminators[position];
        for crossed in &arm.instructions {
            let crossed_accounted = schedulable(function, crossed)
                .ok_or(JoinRelocationError::UnsupportedInstruction)?;
            if member_accounted && crossed_accounted {
                return Err(JoinRelocationError::UnsupportedPair);
            }
            if coupled(member_instruction, crossed) {
                return Err(JoinRelocationError::UnsupportedPair);
            }
        }
        if has_call_contract(function, arm_terminator.id) {
            return Err(JoinRelocationError::UnsupportedInstruction);
        }
        let arm_boundary_accounted = edge_accounted(function, arm_terminator, arm_edge);
        if member_accounted && arm_boundary_accounted {
            return Err(JoinRelocationError::UnsupportedPair);
        }
        if coupled(member_instruction, arm_terminator) {
            return Err(JoinRelocationError::UnsupportedPair);
        }
    }
    // The member trades order with the positions ahead of it in its own
    // body and the positions behind the landing index in the head. Every
    // other position keeps the member on the side it always had.
    for crossed in block.instructions[..member_index]
        .iter()
        .chain(target.instructions[landing_index..].iter())
    {
        let crossed_accounted =
            schedulable(function, crossed).ok_or(JoinRelocationError::UnsupportedInstruction)?;
        if member_accounted && crossed_accounted {
            return Err(JoinRelocationError::UnsupportedPair);
        }
        if coupled(member_instruction, crossed) {
            return Err(JoinRelocationError::UnsupportedPair);
        }
    }
    // A settlement positioned past the member's index observed it inside
    // the join's executed prefix; a settlement positioned past the landing
    // index observes it inside the head's. Both refuse; positions at or
    // before either boundary keep the executed set they always had. Arm
    // blocks are unaffected: the member never enters an arm's body, so no
    // arm prefix ever contained or loses it.
    if function.boundary_settlements.iter().any(|settlement| {
        (settlement.block == block.id && settlement.instruction_index as usize > member_index)
            || (settlement.block == target.id
                && settlement.instruction_index as usize > landing_index)
    }) {
        return Err(JoinRelocationError::UnsupportedPair);
    }
    // The scan walks every block body and terminator instruction once to
    // locate the member, and again with successor edges to gather the
    // join's and the arms' predecessors; the window audit walks the
    // member's surface against each crossed position's, plus the
    // function's three rosters and every crossed edge's binding roster.
    let crossed_surfaces = block.instructions[..member_index]
        .iter()
        .chain(arm_indices.iter().flat_map(|&arm| {
            function.blocks[arm]
                .instructions
                .iter()
                .chain(std::iter::once(terminator_instruction(
                    &function.blocks[arm],
                )))
        }))
        .chain(target.instructions[landing_index..].iter())
        .chain(std::iter::once(terminator))
        .try_fold(0usize, |total, crossed| {
            total
                .checked_add(surface(member_instruction))?
                .checked_add(surface(crossed))
        })
        .ok_or(JoinRelocationError::IdentityOverflow)?;
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
            function.blocks.iter().try_fold(total, |total, candidate| {
                terminator_successors(candidate)
                    .iter()
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
        .ok_or(JoinRelocationError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| JoinRelocationError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(JoinRelocationError::WorkBudgetExceeded);
    }
    Ok(Admission {
        function,
        block_index,
        member_index,
        target_index,
        landing_index,
    })
}
