//! Shared admission for cross-diamond run relocation: locate the named
//! `first_member` and `last_member` bounding one contiguous run inside a
//! block's body, require that block to end in a two-successor
//! conditional branch whose distinct targets — the arms — are each
//! reached by that branch's edges alone and each end in an unconditional
//! `Jump` to one common join reached by arm edges alone, and prove the
//! window the move crosses independent — no register or condition-state
//! hazard between any member and any crossed position, no member
//! interference with any crossed edge's register transports, no
//! roster-carrying run sharing the window with a second memory-access
//! actor, no barrier, call, hosted effect, or call-roster entry inside
//! the window, and no boundary settlement whose observed executed prefix
//! changes.
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{
    SelectedBlockOrigin, SelectedFunction, SelectedInstruction, SelectedInstructionId,
    SelectedSuccessor, SelectedTerminator,
};

use super::DiamondRunRelocationError;
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::{
    all_edges, edge_accounted, plain_edge, terminator_instruction, transport_conflict,
};
use crate::rewrites::window_hazards::{coupled, has_call_contract, schedulable, surface};

pub(super) struct Admission<'source> {
    pub function: &'source SelectedFunction,
    /// The run's own block: the diamond's branching head.
    pub block_index: usize,
    /// The run's first member index in the block body.
    pub first_index: usize,
    /// The run's last member index; the run is the contiguous span
    /// `first_index..=last_index` of at least two members.
    pub last_index: usize,
    /// The join block's index in `function.blocks`.
    pub target_index: usize,
    /// The destination instruction's position in the join body: the run
    /// lands at this index. Naming the join's terminator-carried
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
) -> Result<Admission<'source>, DiamondRunRelocationError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(DiamondRunRelocationError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(DiamondRunRelocationError::SourceMismatch)?;
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
        .ok_or(DiamondRunRelocationError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    // The run is the contiguous span the two named members bound in this
    // block, in the named order; a repeated id or a last member that does
    // not follow the first names no multi-member run. A single member is
    // the one-instruction cross-diamond relocation the sibling family
    // already proves.
    let last_index = block
        .instructions
        .iter()
        .position(|instruction| instruction.id == last_member)
        .filter(|position| *position > first_index)
        .ok_or(DiamondRunRelocationError::UnsupportedPair)?;
    let run = &block.instructions[first_index..=last_index];
    // Only a two-successor conditional terminator gives the run's block
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
            return Err(DiamondRunRelocationError::UnsupportedPair);
        }
    };
    let terminator = terminator_instruction(&block.terminator);
    for edge in &branch_edges {
        if !plain_edge(edge) {
            return Err(DiamondRunRelocationError::UnsupportedPair);
        }
    }
    // The arms are the branch's distinct targets: both edges naming one
    // block is a degenerate diamond with a single arm. Each arm must be a
    // plain source block the branch alone reaches — a second predecessor
    // would give the join a path the run never executed on, the run's own
    // block would make the move self-referential, the entry block is
    // reached with no predecessor at all, and an implementation block's
    // origin carries edge or case work the bounded audit does not cross.
    let mut arm_indices: Vec<usize> = Vec::new();
    for edge in &branch_edges {
        let arm_index = function
            .blocks
            .iter()
            .position(|candidate| candidate.id == edge.block)
            .ok_or(DiamondRunRelocationError::SourceMismatch)?;
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
            return Err(DiamondRunRelocationError::UnsupportedPair);
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
            return Err(DiamondRunRelocationError::UnsupportedPair);
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
            return Err(DiamondRunRelocationError::UnsupportedPair);
        };
        if !plain_edge(arm_edge) {
            return Err(DiamondRunRelocationError::UnsupportedPair);
        }
        let join_index = function
            .blocks
            .iter()
            .position(|candidate| candidate.id == arm_edge.block)
            .ok_or(DiamondRunRelocationError::SourceMismatch)?;
        match target_index {
            Some(existing) if existing != join_index => {
                return Err(DiamondRunRelocationError::UnsupportedPair);
            }
            Some(_) => {}
            None => target_index = Some(join_index),
        }
        arm_edges.push(arm_edge);
        arm_terminators.push(arm_terminator);
    }
    let target_index = target_index.ok_or(DiamondRunRelocationError::UnsupportedPair)?;
    let target = &function.blocks[target_index];
    // The join must not collapse back into the diamond: landing inside the
    // run's own block or inside an arm is an in-block or self-loop move
    // this step does not cross, the entry block is reached with no
    // predecessor at all, and an implementation block's origin carries
    // edge or case work the bounded audit does not cross.
    if target.id == block.id
        || target.id == function.entry_block
        || arm_indices
            .iter()
            .any(|&arm| function.blocks[arm].id == target.id)
        || !matches!(target.origin, SelectedBlockOrigin::Source(_))
    {
        return Err(DiamondRunRelocationError::UnsupportedPair);
    }
    // Every edge into the join must leave an arm: a second predecessor
    // gives the join a path the run would newly execute on.
    let mut join_incoming = 0usize;
    for (source_block, _) in all_edges(function).filter(|(_, edge)| edge.block == target.id) {
        if !arm_indices
            .iter()
            .any(|&arm| function.blocks[arm].id == source_block)
        {
            return Err(DiamondRunRelocationError::UnsupportedPair);
        }
        join_incoming += 1;
    }
    if join_incoming != arm_indices.len() {
        return Err(DiamondRunRelocationError::UnsupportedPair);
    }
    // The destination names a position in the join body — the run lands
    // at its index — or the join's terminator-carried instruction,
    // landing the run at the body end.
    let landing_index = target
        .instructions
        .iter()
        .position(|instruction| instruction.id == destination)
        .or_else(|| {
            (terminator_instruction(&target.terminator).id == destination)
                .then_some(target.instructions.len())
        })
        .ok_or(DiamondRunRelocationError::UnsupportedPair)?;
    // Every member meets the schedulable bar itself; the run's memory
    // accounting is the union of its members' roster rows.
    let mut run_accounted = false;
    for member in run {
        run_accounted |= schedulable(function, member)
            .ok_or(DiamondRunRelocationError::UnsupportedInstruction)?;
    }
    // Every crossed edge's register transports sit between the run's old
    // and new positions — the two branch edges and each arm's jump edge.
    for edge in branch_edges
        .iter()
        .copied()
        .chain(arm_edges.iter().copied())
    {
        for member in run {
            if transport_conflict(member, edge) {
                return Err(DiamondRunRelocationError::UnsupportedPair);
            }
        }
    }
    // The branch terminator plus its two edges is the first crossed
    // position: it is exempt from the barrier-kind rule but not from the
    // call, hazard, or memory accounting.
    if has_call_contract(function, terminator.id) {
        return Err(DiamondRunRelocationError::UnsupportedInstruction);
    }
    let branch_accounted = branch_edges
        .iter()
        .copied()
        .any(|edge| edge_accounted(function, terminator, edge));
    if run_accounted && branch_accounted {
        return Err(DiamondRunRelocationError::UnsupportedPair);
    }
    for member in run {
        if coupled(member, terminator) {
            return Err(DiamondRunRelocationError::UnsupportedPair);
        }
    }
    // Each arm's whole body sits inside the window, then its `Jump`
    // terminator and outgoing edge form that arm's boundary position.
    for position in 0..arm_indices.len() {
        let arm = &function.blocks[arm_indices[position]];
        let arm_edge = arm_edges[position];
        let arm_terminator = arm_terminators[position];
        for crossed in &arm.instructions {
            let crossed_accounted = schedulable(function, crossed)
                .ok_or(DiamondRunRelocationError::UnsupportedInstruction)?;
            if run_accounted && crossed_accounted {
                return Err(DiamondRunRelocationError::UnsupportedPair);
            }
            for member in run {
                if coupled(member, crossed) {
                    return Err(DiamondRunRelocationError::UnsupportedPair);
                }
            }
        }
        if has_call_contract(function, arm_terminator.id) {
            return Err(DiamondRunRelocationError::UnsupportedInstruction);
        }
        let arm_boundary_accounted = edge_accounted(function, arm_terminator, arm_edge);
        if run_accounted && arm_boundary_accounted {
            return Err(DiamondRunRelocationError::UnsupportedPair);
        }
        for member in run {
            if coupled(member, arm_terminator) {
                return Err(DiamondRunRelocationError::UnsupportedPair);
            }
        }
    }
    // Every member trades order with the positions behind the run in its
    // own body and the positions before the landing index in the join
    // body. Every other position keeps the run on the side it always had.
    for crossed in block.instructions[last_index + 1..]
        .iter()
        .chain(target.instructions[..landing_index].iter())
    {
        let crossed_accounted = schedulable(function, crossed)
            .ok_or(DiamondRunRelocationError::UnsupportedInstruction)?;
        if run_accounted && crossed_accounted {
            return Err(DiamondRunRelocationError::UnsupportedPair);
        }
        for member in run {
            if coupled(member, crossed) {
                return Err(DiamondRunRelocationError::UnsupportedPair);
            }
        }
    }
    // A settlement positioned past the run's first index observed a
    // member inside the source block's executed prefix; a settlement
    // positioned past the landing index observes the run inside the
    // join's. Both refuse; positions at or before either boundary keep
    // the executed set they always had. Arm blocks are unaffected: the
    // run never enters an arm's body, so no arm prefix ever contained or
    // loses it.
    if function.boundary_settlements.iter().any(|settlement| {
        (settlement.block == block.id && settlement.instruction_index as usize > first_index)
            || (settlement.block == target.id
                && settlement.instruction_index as usize > landing_index)
    }) {
        return Err(DiamondRunRelocationError::UnsupportedPair);
    }
    // The scan walks every block body and terminator instruction once to
    // locate the run's first member, and again with successor edges to
    // count predecessor edges; the window audit walks every
    // member-against-crossed operand and unit surface, plus the
    // function's three rosters and every crossed edge's binding roster
    // once per member.
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
                    .chain(arm_indices.iter().flat_map(|&arm| {
                        function.blocks[arm]
                            .instructions
                            .iter()
                            .chain(std::iter::once(terminator_instruction(
                                &function.blocks[arm].terminator,
                            )))
                    }))
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
                .checked_add(function.boundary_settlements.len())
        })
        .and_then(|total| {
            branch_edges
                .iter()
                .chain(arm_edges.iter())
                .try_fold(total, |total, edge| {
                    total.checked_add(edge.bindings.len().saturating_mul(run.len()))
                })
        })
        .ok_or(DiamondRunRelocationError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| DiamondRunRelocationError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(DiamondRunRelocationError::WorkBudgetExceeded);
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
