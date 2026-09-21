//! The path-derived relocation window: every simple path from the run's
//! block to the destination's, and the crossed positions, crossed edges,
//! and skipped edges those paths induce — the audits apply to the derived
//! sets, not to a named shape.
use std::collections::{BTreeSet, VecDeque};

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{
    SelectedBlock, SelectedBlockId, SelectedBlockOrigin, SelectedFunction, SelectedInstruction,
    SelectedInstructionId,
};

use super::ScheduledRelocationError;
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::{
    block_instructions, edge_surface, plain_edge, terminator_instruction, terminator_successors,
    transport_conflict,
};
use crate::rewrites::dead_path;
use crate::rewrites::window_hazards::{
    coupled, has_call_contract, register_writes, schedulable, surface,
};

/// The admitted run relocation: where the run leaves and where it lands.
pub(super) struct Admission {
    /// The block the contiguous run vacates.
    pub(super) block_index: usize,
    /// The first body index the run occupies.
    pub(super) member_first: usize,
    /// The last body index the run occupies — `member_first` for a
    /// one-member run.
    pub(super) member_last: usize,
    /// The block the run lands in.
    pub(super) target_index: usize,
    /// The body index the run's first member takes; naming the
    /// destination's terminator-carried instruction lands the run at the
    /// body end.
    pub(super) landing_index: usize,
}

/// Whether the member's effect is pure register and condition-state work
/// that cannot observe or abandon the execution it leaves behind.
/// `schedulable` already cleared barrier kinds, call contracts, and
/// unaccounted memory-capable kinds, but a row-less load or private-slot
/// `Store64` still performs a memory access: sinking it would remove the
/// access — and any fault or slot write it carried — from every path the
/// move abandons. The same holds for kinds whose target encoding may
/// architecturally fault: their proof obligations establish definedness
/// for the source operation, but this audit runs at the selected level
/// where the encoded trap behavior is the honest bound — an execution
/// that could fault must still run on every path that ran it before.
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
            | SaturatingDivide { .. }
            | SaturatingRemainder { .. }
    )
}

/// Whether every execution leaving `from` reaches `to` before it can exit
/// or revisit a block — the hoist's once-per-traversal bound. A block
/// reachable from `from` without passing `to` that terminates, or sits on
/// a cycle, would let a traversal pass `from` yet abandon or repeat the
/// run's execution. The returned count is the depth-first walk's
/// expansions for the work-budget fold.
fn continuations_reach(
    function: &SelectedFunction,
    from: usize,
    to: usize,
) -> Result<(bool, usize), ScheduledRelocationError> {
    const UNVISITED: u8 = 0;
    const IN_STACK: u8 = 1;
    const DONE: u8 = 2;
    let mut marks = vec![UNVISITED; function.blocks.len()];
    let mut expansions = 0usize;
    let mut stack: Vec<(usize, usize)> = vec![(from, 0)];
    marks[from] = IN_STACK;
    while let Some((block, edge_index)) = stack.last_mut() {
        let successors = terminator_successors(&function.blocks[*block].terminator);
        if successors.is_empty() {
            return Ok((false, expansions));
        }
        if *edge_index == successors.len() {
            marks[*block] = DONE;
            stack.pop();
            continue;
        }
        let edge = successors[*edge_index];
        *edge_index += 1;
        let next =
            block_index_of(function, edge.block).ok_or(ScheduledRelocationError::SourceMismatch)?;
        if next == to {
            continue;
        }
        match marks[next] {
            IN_STACK => return Ok((false, expansions)),
            DONE => continue,
            _ => {
                marks[next] = IN_STACK;
                expansions += 1;
                stack.push((next, 0));
            }
        }
    }
    Ok((true, expansions))
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

fn block_index_of(function: &SelectedFunction, id: SelectedBlockId) -> Option<usize> {
    function.blocks.iter().position(|block| block.id == id)
}

/// Whether `target` is reachable from `from` while `avoid` is never
/// expanded — the dominator question asked concretely: `avoid` dominates
/// `target` exactly when no such route exists. `None` expands every block.
fn reaches_avoiding(
    function: &SelectedFunction,
    from: usize,
    avoid: Option<usize>,
    target: usize,
) -> Result<bool, ScheduledRelocationError> {
    let mut seen = vec![false; function.blocks.len()];
    let mut pending = VecDeque::from([from]);
    seen[from] = true;
    while let Some(current) = pending.pop_front() {
        if current == target {
            return Ok(true);
        }
        if Some(current) == avoid {
            continue;
        }
        for edge in terminator_successors(&function.blocks[current].terminator) {
            let next = block_index_of(function, edge.block)
                .ok_or(ScheduledRelocationError::SourceMismatch)?;
            if !seen[next] {
                seen[next] = true;
                pending.push_back(next);
            }
        }
    }
    Ok(false)
}

/// The path-derived region: the blocks lying on at least one simple path
/// from `from` to `to`, and the successor edges those paths use —
/// enumerated only until `exploration` exceeds the work budget, since the
/// number of simple paths can grow exponentially in a cyclic graph.
struct Derived {
    /// Blocks on at least one path — the member's block, the destination's,
    /// and every intermediate the run physically passes.
    region: BTreeSet<usize>,
    /// Edges on at least one path, keyed by source block index and the
    /// edge's position in its terminator's successor list.
    used_edges: BTreeSet<(usize, usize)>,
    /// Stack expansions the enumeration spent, folded into the budget.
    exploration: usize,
}

fn derive_paths(
    function: &SelectedFunction,
    from: usize,
    to: usize,
    budget: OptimizationWorkBudget,
) -> Result<Derived, ScheduledRelocationError> {
    let mut derived = Derived {
        region: BTreeSet::new(),
        used_edges: BTreeSet::new(),
        exploration: 0,
    };
    // Each stack entry is one simple path prefix: the block sequence and
    // the edge identity that joined each block to its predecessor.
    let mut pending: Vec<(usize, Vec<usize>, Vec<(usize, usize)>)> =
        vec![(from, vec![from], vec![])];
    while let Some((current, path, edges)) = pending.pop() {
        derived.exploration += 1;
        if u64::try_from(derived.exploration)
            .map_err(|_| ScheduledRelocationError::IdentityOverflow)?
            > budget.validation_steps()
        {
            return Err(ScheduledRelocationError::WorkBudgetExceeded);
        }
        if current == to {
            derived.region.extend(path.iter().copied());
            derived.used_edges.extend(edges.iter().copied());
            continue;
        }
        for (edge_position, edge) in terminator_successors(&function.blocks[current].terminator)
            .iter()
            .enumerate()
        {
            let next = block_index_of(function, edge.block)
                .ok_or(ScheduledRelocationError::SourceMismatch)?;
            // A path revisiting a block runs its stream twice while the run
            // moves once; only simple paths bound the window.
            if path.contains(&next) {
                continue;
            }
            let mut next_path = path.clone();
            next_path.push(next);
            let mut next_edges = edges.clone();
            next_edges.push((current, edge_position));
            pending.push((next, next_path, next_edges));
        }
    }
    Ok(derived)
}

/// Admit `members` — a contiguous run of body instructions named in their
/// own order — to leave their block and land on `destination`'s position
/// in a block their block dominates (a sink), or land on a position in a
/// block dominating theirs whose continuations all reach theirs (a
/// hoist), when the shared audits clear the derived window: every path
/// between them simple and through plain `Source` blocks, every crossed
/// edge plain and free of member interference, every crossed position
/// schedulable and uncoupled, no boundary settlement observing a changed
/// prefix, and — for a sink, which abandons traversals — every location
/// the run writes dead on every traversal the run no longer executes on.
pub(super) fn admit<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    members: &[SelectedInstructionId],
    destination: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Admission, ScheduledRelocationError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(ScheduledRelocationError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(ScheduledRelocationError::SourceMismatch)?;
    let [first_member, ..] = members else {
        return Err(ScheduledRelocationError::UnsupportedPair);
    };
    // The run's first member bounds the block and span; the remaining
    // members must occupy the consecutive body positions in order.
    let (block_index, member_first) = function
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block_index, block)| {
            block
                .instructions
                .iter()
                .position(|instruction| instruction.id == *first_member)
                .map(|member_index| (block_index, member_index))
        })
        .ok_or(ScheduledRelocationError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    let member_last = member_first + members.len() - 1;
    if members.iter().enumerate().any(|(offset, member)| {
        block
            .instructions
            .get(member_first + offset)
            .map(|instruction| instruction.id)
            != Some(*member)
    }) {
        return Err(ScheduledRelocationError::UnsupportedPair);
    }
    let run: Vec<&SelectedInstruction> = block.instructions[member_first..=member_last]
        .iter()
        .collect();
    // The destination names exactly one position: a body instruction's
    // index in its block, or the body end for a terminator-carried
    // instruction.
    let mut destination_position = None;
    for (target_index, target) in function.blocks.iter().enumerate() {
        if let Some(index) = landing_position(target, destination) {
            if destination_position.is_some() {
                return Err(ScheduledRelocationError::SourceMismatch);
            }
            destination_position = Some((target_index, index));
        }
    }
    let Some((target_index, landing_index)) = destination_position else {
        return Err(ScheduledRelocationError::UnsupportedPair);
    };
    // An in-block move is the adjacent-interchange family's case; this
    // admission bounds only moves across a block boundary.
    if target_index == block_index {
        return Err(ScheduledRelocationError::UnsupportedPair);
    }
    let entry_index = block_index_of(function, function.entry_block)
        .ok_or(ScheduledRelocationError::SourceMismatch)?;
    // The direction binds the once-per-traversal argument. A sink keeps
    // the run once-per-traversal because every destination traversal ran
    // the run's block first; a hoist mirrors that — the destination's
    // block must dominate the run's, so no traversal reaches the run
    // without having executed it at the landing. Any other topology moves
    // the run across an inflow or a branch where some traversal would
    // gain or lose it.
    enum Direction {
        Sink,
        Hoist,
    }
    let direction = if !reaches_avoiding(function, entry_index, Some(block_index), target_index)? {
        Direction::Sink
    } else if !reaches_avoiding(function, entry_index, Some(target_index), block_index)? {
        Direction::Hoist
    } else {
        return Err(ScheduledRelocationError::UnsupportedPair);
    };
    let mut search_steps = 0usize;
    match direction {
        Direction::Sink => {
            // The run lands in the destination's stream and runs on every
            // traversal of it; a destination re-enterable from its own
            // successors would run it again where the source ran it once.
            for edge in terminator_successors(&function.blocks[target_index].terminator) {
                let next = block_index_of(function, edge.block)
                    .ok_or(ScheduledRelocationError::SourceMismatch)?;
                if next == target_index || reaches_avoiding(function, next, None, target_index)? {
                    return Err(ScheduledRelocationError::UnsupportedPair);
                }
            }
        }
        Direction::Hoist => {
            // A run block re-enterable from its own successors without
            // crossing the destination again would execute the run where
            // the relocated program already executed it once at the
            // landing.
            for edge in terminator_successors(&function.blocks[block_index].terminator) {
                let next = block_index_of(function, edge.block)
                    .ok_or(ScheduledRelocationError::SourceMismatch)?;
                if next == block_index
                    || reaches_avoiding(function, next, Some(target_index), block_index)?
                {
                    return Err(ScheduledRelocationError::UnsupportedPair);
                }
            }
            // Every traversal leaving the destination must reach the
            // run's block before it exits or revisits a block: an escape
            // would execute the relocated run on a traversal that never
            // ran it, and a cycle would run it again before the source
            // position arrived once. Since the search certifies that
            // continuation acyclic, every execution between them is a
            // simple path — so the derived window's edge audit covers
            // every edge a real traversal can cross.
            let (reaches, expansions) = continuations_reach(function, target_index, block_index)?;
            search_steps = expansions;
            if !reaches {
                return Err(ScheduledRelocationError::UnsupportedPair);
            }
        }
    }
    // Derive the region: every simple path between the run's block and
    // the destination's, in the direction execution traverses it — the
    // destination trails a sink, leads a hoist. No path at all means the
    // run would only be deleted, not relocated.
    let (window_from, window_to) = match direction {
        Direction::Sink => (block_index, target_index),
        Direction::Hoist => (target_index, block_index),
    };
    let derived = derive_paths(function, window_from, window_to, budget)?;
    if derived.region.is_empty() {
        return Err(ScheduledRelocationError::UnsupportedPair);
    }
    // Every block a path crosses is plain `Source` control flow: an
    // implementation block's origin carries edge or case work the bounded
    // audit does not cross.
    for index in &derived.region {
        if !matches!(
            function.blocks[*index].origin,
            SelectedBlockOrigin::Source(_)
        ) {
            return Err(ScheduledRelocationError::UnsupportedPair);
        }
    }
    // Every edge a path crosses must be a plain semantic successor the
    // run's transports do not interfere with.
    for &(block, edge_position) in &derived.used_edges {
        let edge = terminator_successors(&function.blocks[block].terminator)[edge_position];
        if !plain_edge(edge) || run.iter().any(|member| transport_conflict(member, edge)) {
            return Err(ScheduledRelocationError::UnsupportedPair);
        }
    }
    // The run's own effect bound: the move makes its execution conditional
    // on reaching the destination, so only pure register and
    // condition-state work may relocate.
    for member in &run {
        if schedulable(function, member) != Some(false) || !sinkable(member) {
            return Err(ScheduledRelocationError::UnsupportedInstruction);
        }
    }
    // The run trades order with the positions that switch sides of it:
    // for a sink, the tail of its own body, the whole stream of every
    // block a path crosses, and the positions before the landing index in
    // the destination block; for a hoist, the mirror — the head of its
    // own body, the intermediates' streams, and the positions from the
    // landing index on. Every other position keeps the run on the side it
    // always had.
    let intermediates: Vec<usize> = derived
        .region
        .iter()
        .copied()
        .filter(|index| *index != block_index && *index != target_index)
        .collect();
    let mut crossed_body: Vec<&SelectedInstruction> = Vec::new();
    let crossed_terminators: Vec<usize> = match direction {
        Direction::Sink => {
            crossed_body.extend(block.instructions[member_last + 1..].iter());
            for &index in &intermediates {
                crossed_body.extend(function.blocks[index].instructions.iter());
            }
            crossed_body.extend(function.blocks[target_index].instructions[..landing_index].iter());
            std::iter::once(block_index)
                .chain(intermediates.iter().copied())
                .collect()
        }
        Direction::Hoist => {
            crossed_body.extend(block.instructions[..member_first].iter());
            for &index in &intermediates {
                crossed_body.extend(function.blocks[index].instructions.iter());
            }
            crossed_body.extend(function.blocks[target_index].instructions[landing_index..].iter());
            std::iter::once(target_index)
                .chain(intermediates.iter().copied())
                .collect()
        }
    };
    for crossed in &crossed_body {
        schedulable(function, crossed).ok_or(ScheduledRelocationError::UnsupportedInstruction)?;
        if run.iter().any(|member| coupled(member, crossed)) {
            return Err(ScheduledRelocationError::UnsupportedPair);
        }
    }
    // The terminator at each crossed block's boundary is a crossed
    // position: exempt from the barrier-kind rule but not from the call
    // or hazard audit. For a sink the run's own block's terminator and
    // the intermediates' are crossed while the destination's stays behind
    // the landing index; a hoist crosses the destination's and the
    // intermediates' while the run's block's terminator stays ahead.
    for index in crossed_terminators {
        let terminator = terminator_instruction(&function.blocks[index].terminator);
        if has_call_contract(function, terminator.id) {
            return Err(ScheduledRelocationError::UnsupportedInstruction);
        }
        if run.iter().any(|member| coupled(member, terminator)) {
            return Err(ScheduledRelocationError::UnsupportedPair);
        }
    }
    // A settlement positioned past the run's first index observed the run
    // inside its block's executed prefix; a settlement positioned past
    // the landing index observes it inside the destination's. Both
    // refuse; positions at or before either boundary keep the executed
    // set they always had, and blocks on the skipped paths are
    // unaffected — the run was never in their streams.
    if function.boundary_settlements.iter().any(|settlement| {
        (settlement.block == block.id && settlement.instruction_index as usize > member_first)
            || (settlement.block == function.blocks[target_index].id
                && settlement.instruction_index as usize > landing_index)
    }) {
        return Err(ScheduledRelocationError::UnsupportedPair);
    }
    // A sink abandons traversals: the skipped edges — every edge leaving
    // a region block, other than the destination block, that no derived
    // path uses — begin the traversals the run no longer executes on. The
    // dead-path audit requires every location the run writes to die
    // there: the vacated span stays silent because every position behind
    // it that could observe the missing writes sits inside the crossed
    // window, and the landing is Executed because the dominance bound
    // makes every destination traversal run the run. A hoist abandons
    // nothing: its dominance and continuation bounds keep every traversal
    // that ran the run running it.
    if matches!(direction, Direction::Sink) {
        let mut skipped_edges = Vec::new();
        for &index in &derived.region {
            if index == target_index {
                continue;
            }
            for (edge_position, edge) in terminator_successors(&function.blocks[index].terminator)
                .iter()
                .enumerate()
            {
                if !derived.used_edges.contains(&(index, edge_position)) {
                    skipped_edges.push(*edge);
                }
            }
        }
        if !skipped_edges.is_empty()
            && !dead_path::dead(
                function,
                dead_path::Relocation {
                    members: &run,
                    vacated_block: block_index,
                    vacated_first: member_first,
                    vacated_last: member_last,
                    landing_block: target_index,
                    landing_index,
                    landing: dead_path::Landing::Executed,
                    vacated: dead_path::Vacated::Silent,
                },
                dead_path::Start::Edges(&skipped_edges),
            )
        {
            return Err(ScheduledRelocationError::UnsupportedPair);
        }
    }
    // The scan walks every block body, terminator, and edge once to
    // locate the run, the destination, and the paths between them; the
    // window audit walks each member's surface against each crossed
    // position's and each crossed edge's; the dead-path audit rescans a
    // block's stream and edge surfaces only while its entry set grows —
    // at most once per member location per block.
    let member_locations = run
        .iter()
        .map(|member| {
            register_writes(member).count() + member.implicit_defs.len() + member.clobbers.len()
        })
        .sum::<usize>();
    let member_surface = run.iter().map(|member| surface(member)).sum::<usize>();
    let block_scan: usize = function
        .blocks
        .iter()
        .map(|block| {
            block_instructions(block).map(surface).sum::<usize>()
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
            function.blocks.iter().try_fold(total, |total, block| {
                total.checked_add(block_instructions(block).count())
            })
        })
        .and_then(|total| total.checked_add(derived.exploration))
        .and_then(|total| total.checked_add(search_steps))
        .and_then(|total| {
            derived
                .used_edges
                .len()
                .checked_mul(member_surface)
                .and_then(|work| total.checked_add(work))
        })
        .and_then(|total| {
            crossed_body
                .len()
                .checked_add(intermediates.len().saturating_add(1))
                .and_then(|positions| positions.checked_mul(member_surface.saturating_add(1)))
                .and_then(|work| total.checked_add(work))
        })
        .and_then(|total| {
            total
                .checked_add(function.memory_accesses.len())?
                .checked_add(function.calls.len())?
                .checked_add(function.boundary_settlements.len())
        })
        .and_then(|total| {
            total.checked_add(block_scan.saturating_mul(member_locations.saturating_add(1)))
        })
        .ok_or(ScheduledRelocationError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| ScheduledRelocationError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(ScheduledRelocationError::WorkBudgetExceeded);
    }
    Ok(Admission {
        block_index,
        member_first,
        member_last,
        target_index,
        landing_index,
    })
}
