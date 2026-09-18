//! Shared admission for cross-triangle run relocation: locate the named
//! `first_member` and `last_member` bounding one contiguous run inside a
//! block's body, require that block to end in a two-successor
//! conditional branch where at least one edge lands directly on the join
//! — the bypass — and every other distinct target is an arm reached by
//! that branch's edges alone and ending in an unconditional `Jump` to
//! the join, with every edge into the join leaving the head or an arm,
//! and prove the window the move crosses independent — no register or
//! condition-state hazard between any member and any crossed position,
//! no member interference with any crossed edge's register transports,
//! no roster-carrying run sharing the window with a second
//! memory-access actor, no barrier, call, hosted effect, or call-roster
//! entry inside the window, and no boundary settlement whose observed
//! executed prefix changes.
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{
    SelectedBlockId, SelectedBlockOrigin, SelectedFunction, SelectedInstruction,
    SelectedInstructionId, SelectedSuccessor, SelectedTerminator,
};

use super::BypassRunRelocationError;
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::{
    all_edges, edge_accounted, plain_edge, terminator_instruction, transport_conflict,
};
use crate::rewrites::window_hazards::{coupled, has_call_contract, schedulable, surface};

pub(super) struct Admission<'source> {
    pub function: &'source SelectedFunction,
    /// The run's own block: the triangle's branching head.
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

/// The index of the block an edge lands on.
fn block_index_of(function: &SelectedFunction, id: SelectedBlockId) -> Option<usize> {
    function
        .blocks
        .iter()
        .position(|candidate| candidate.id == id)
}

/// Whether `arm_index` is a plain arm of this bypassed triangle: a source
/// block — never the run's own block, which would make the move
/// self-referential, and never the entry block, which is reached with no
/// predecessor at all — that the branch's edges alone reach, ending in an
/// unconditional `Jump` to the join on a plain semantic edge. A second
/// predecessor into the arm would give the join a path the run never
/// executed on, and an implementation block's origin carries edge or
/// case work the bounded audit does not cross.
fn arm_into<'function>(
    function: &'function SelectedFunction,
    branch_edges: &[&SelectedSuccessor],
    head: SelectedBlockId,
    arm_index: usize,
    join: SelectedBlockId,
) -> Option<(&'function SelectedInstruction, &'function SelectedSuccessor)> {
    let arm = &function.blocks[arm_index];
    if arm.id == head
        || arm.id == function.entry_block
        || !matches!(arm.origin, SelectedBlockOrigin::Source(_))
    {
        return None;
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
        return None;
    }
    let SelectedTerminator::Jump {
        instruction: arm_terminator,
        successor: arm_edge,
    } = &arm.terminator
    else {
        return None;
    };
    if !plain_edge(arm_edge) || arm_edge.block != join {
        return None;
    }
    Some((arm_terminator, arm_edge))
}

pub(super) fn admit<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    first_member: SelectedInstructionId,
    last_member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Admission<'source>, BypassRunRelocationError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(BypassRunRelocationError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(BypassRunRelocationError::SourceMismatch)?;
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
        .ok_or(BypassRunRelocationError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    // The run is the contiguous span the two named members bound in this
    // block, in the named order; a repeated id or a last member that does
    // not follow the first names no multi-member run. A single member is
    // the one-instruction cross-triangle relocation the sibling family
    // already proves.
    let last_index = block
        .instructions
        .iter()
        .position(|instruction| instruction.id == last_member)
        .filter(|position| *position > first_index)
        .ok_or(BypassRunRelocationError::UnsupportedPair)?;
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
            return Err(BypassRunRelocationError::UnsupportedPair);
        }
    };
    let terminator = terminator_instruction(&block.terminator);
    for edge in &branch_edges {
        if !plain_edge(edge) {
            return Err(BypassRunRelocationError::UnsupportedPair);
        }
    }
    // The join is a branch target itself — the bypass edge lands on it
    // directly, which is exactly where this family parts from the
    // all-arms diamond. Each distinct edge target is tried in turn: a
    // candidate admits when every other distinct target is an arm ending
    // in a `Jump` to it and every edge into it leaves the head or an
    // arm. At most one candidate can admit — a second would make each an
    // arm of the other, and a block jumping to a candidate join is a
    // predecessor that candidate's arm check refuses — so the first and
    // only match stands; zero matches is the diamond family's shape or
    // no converging move at all.
    let mut candidates: Vec<usize> = Vec::new();
    for edge in &branch_edges {
        let index =
            block_index_of(function, edge.block).ok_or(BypassRunRelocationError::SourceMismatch)?;
        if !candidates.contains(&index) {
            candidates.push(index);
        }
    }
    let mut found: Option<(usize, Vec<usize>, Vec<&SelectedSuccessor>)> = None;
    for &join_index in &candidates {
        let join = &function.blocks[join_index];
        // The join must not collapse back into the triangle: landing
        // inside the run's own block is an in-block or self-loop move
        // this step does not cross, the entry block is reached with no
        // predecessor at all, and an implementation block's origin
        // carries edge or case work the bounded audit does not cross.
        if join.id == block.id
            || join.id == function.entry_block
            || !matches!(join.origin, SelectedBlockOrigin::Source(_))
        {
            continue;
        }
        let mut arm_indices: Vec<usize> = Vec::new();
        let mut arm_edges: Vec<&SelectedSuccessor> = Vec::new();
        let mut closes = true;
        for &arm_index in &candidates {
            if arm_index == join_index || arm_indices.contains(&arm_index) {
                continue;
            }
            match arm_into(function, &branch_edges, block.id, arm_index, join.id) {
                Some((_, arm_edge)) => {
                    arm_indices.push(arm_index);
                    arm_edges.push(arm_edge);
                }
                None => {
                    closes = false;
                    break;
                }
            }
        }
        if !closes {
            continue;
        }
        // Every edge into the join must leave the head or an arm: a
        // predecessor anywhere else — including the join's own self-loop —
        // gives the join a path the run would newly execute on or execute
        // a second time. Head edges and arm edges into the join are
        // already enumerated, so the per-source check is complete.
        for (source_block, _) in all_edges(function).filter(|(_, edge)| edge.block == join.id) {
            if source_block != block.id
                && !arm_indices
                    .iter()
                    .any(|&arm| function.blocks[arm].id == source_block)
            {
                closes = false;
                break;
            }
        }
        if !closes {
            continue;
        }
        if found.is_some() {
            return Err(BypassRunRelocationError::UnsupportedPair);
        }
        found = Some((join_index, arm_indices, arm_edges));
    }
    let (target_index, arm_indices, arm_edges) =
        found.ok_or(BypassRunRelocationError::UnsupportedPair)?;
    let target = &function.blocks[target_index];
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
        .ok_or(BypassRunRelocationError::UnsupportedPair)?;
    // Every member meets the schedulable bar itself; the run's memory
    // accounting is the union of its members' roster rows.
    let mut run_accounted = false;
    for member in run {
        run_accounted |= schedulable(function, member)
            .ok_or(BypassRunRelocationError::UnsupportedInstruction)?;
    }
    // Every crossed edge's register transports sit between the run's old
    // and new positions — the two branch edges, the bypass included, and
    // each arm's jump edge.
    for edge in branch_edges
        .iter()
        .copied()
        .chain(arm_edges.iter().copied())
    {
        for member in run {
            if transport_conflict(member, edge) {
                return Err(BypassRunRelocationError::UnsupportedPair);
            }
        }
    }
    // The branch terminator plus its two edges is the first crossed
    // position: it is exempt from the barrier-kind rule but not from the
    // call, hazard, or memory accounting.
    if has_call_contract(function, terminator.id) {
        return Err(BypassRunRelocationError::UnsupportedInstruction);
    }
    let branch_accounted = branch_edges
        .iter()
        .copied()
        .any(|edge| edge_accounted(function, terminator, edge));
    if run_accounted && branch_accounted {
        return Err(BypassRunRelocationError::UnsupportedPair);
    }
    for member in run {
        if coupled(member, terminator) {
            return Err(BypassRunRelocationError::UnsupportedPair);
        }
    }
    // Each arm's whole body sits inside the window, then its `Jump`
    // terminator and outgoing edge form that arm's boundary position.
    for position in 0..arm_indices.len() {
        let arm = &function.blocks[arm_indices[position]];
        let arm_edge = arm_edges[position];
        let arm_terminator = terminator_instruction(&arm.terminator);
        for crossed in &arm.instructions {
            let crossed_accounted = schedulable(function, crossed)
                .ok_or(BypassRunRelocationError::UnsupportedInstruction)?;
            if run_accounted && crossed_accounted {
                return Err(BypassRunRelocationError::UnsupportedPair);
            }
            for member in run {
                if coupled(member, crossed) {
                    return Err(BypassRunRelocationError::UnsupportedPair);
                }
            }
        }
        if has_call_contract(function, arm_terminator.id) {
            return Err(BypassRunRelocationError::UnsupportedInstruction);
        }
        let arm_boundary_accounted = edge_accounted(function, arm_terminator, arm_edge);
        if run_accounted && arm_boundary_accounted {
            return Err(BypassRunRelocationError::UnsupportedPair);
        }
        for member in run {
            if coupled(member, arm_terminator) {
                return Err(BypassRunRelocationError::UnsupportedPair);
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
            .ok_or(BypassRunRelocationError::UnsupportedInstruction)?;
        if run_accounted && crossed_accounted {
            return Err(BypassRunRelocationError::UnsupportedPair);
        }
        for member in run {
            if coupled(member, crossed) {
                return Err(BypassRunRelocationError::UnsupportedPair);
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
        return Err(BypassRunRelocationError::UnsupportedPair);
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
        .ok_or(BypassRunRelocationError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| BypassRunRelocationError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(BypassRunRelocationError::WorkBudgetExceeded);
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
