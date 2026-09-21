//! Shared admission for diamond relocation: locate the named `member` in
//! one block's body, require that block to end in a two-successor
//! conditional branch whose distinct targets — the arms — are each reached
//! by that branch's edges alone and each end in an unconditional `Jump` to
//! one common join reached by arm edges alone, then derive the window the
//! move crosses and prove it independent through the shared relocation
//! admission: no register or condition-state hazard between the member and
//! any crossed position, no interference with any crossed edge's register
//! transports, no roster-carrying member sharing the window with a second
//! memory-access actor, no barrier, call, hosted effect, or call-roster
//! entry inside the window, and no boundary settlement whose observed
//! executed prefix changes.
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{
    SelectedBlockOrigin, SelectedFunction, SelectedInstructionId, SelectedSuccessor,
    SelectedTerminator,
};

use super::DiamondRelocationError;
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::{
    CrossingDirection, PATH_EDGE_LIMIT, all_edges, crossed_window, plain_edge,
    terminator_instruction,
};
use crate::rewrites::window_hazards::{RunRelocationRejection, admit_run_relocation, surface};

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
        | SelectedTerminator::Crash { .. }
        | SelectedTerminator::HostedExitProcess { .. }
        | SelectedTerminator::Return { .. } => {
            return Err(DiamondRelocationError::UnsupportedPair);
        }
    };
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
    for &arm_index in &arm_indices {
        let arm = &function.blocks[arm_index];
        let SelectedTerminator::Jump {
            successor: arm_edge,
            ..
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
            (terminator_instruction(&target.terminator).id == destination)
                .then_some(target.instructions.len())
        })
        .ok_or(DiamondRelocationError::UnsupportedPair)?;
    // The window the member's move crosses derives through the shared
    // relocation rule once: the member's own tail behind it, every arm's
    // whole body, the join's head before the landing index, and the two
    // branch edges plus each arm's `Jump` edge — the same set this family
    // enumerated by hand. The shared audit applies the schedulable,
    // coupling, memory-ordering, transport, and settlement refusals
    // unchanged; the topology gates above already bound every traversal.
    let crossing = crossed_window(
        function,
        block_index,
        member_index,
        member_index,
        target_index,
        landing_index,
        CrossingDirection::Forward,
        PATH_EDGE_LIMIT,
    )
    .ok_or(DiamondRelocationError::WorkBudgetExceeded)?;
    admit_run_relocation(function, &[member_instruction], &crossing).map_err(|rejection| {
        match rejection {
            RunRelocationRejection::Unschedulable => DiamondRelocationError::UnsupportedInstruction,
            RunRelocationRejection::UnreachableDestination
            | RunRelocationRejection::Coupled
            | RunRelocationRejection::MemoryOrdering
            | RunRelocationRejection::TransportConflict
            | RunRelocationRejection::NonPlainEdge
            | RunRelocationRejection::Settlement => DiamondRelocationError::UnsupportedPair,
        }
    })?;
    // The scan walks every block body and terminator instruction once to
    // locate the member, and again with successor edges to count
    // predecessor edges; the window audit walks the member's surface
    // against each crossed position's, plus the function's three rosters
    // and every crossed edge's binding roster.
    let mut terminator_ids = std::collections::BTreeSet::new();
    let crossed_surfaces = crossing
        .positions
        .iter()
        .flat_map(|(block_index, positions)| {
            positions
                .iter()
                .map(|position| &function.blocks[*block_index].instructions[*position])
        })
        .chain(
            crossing
                .edges
                .iter()
                .map(|edge| edge.instruction)
                .filter(|instruction| terminator_ids.insert(instruction.id)),
        )
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
                    .chain(std::iter::once(terminator_instruction(
                        &candidate.terminator,
                    )))
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
            crossing.edges.iter().try_fold(total, |total, edge| {
                total.checked_add(edge.successor.bindings.len())
            })
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
