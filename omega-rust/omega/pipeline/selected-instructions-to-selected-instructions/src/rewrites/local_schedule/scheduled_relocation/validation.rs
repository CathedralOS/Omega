use std::collections::{BTreeSet, VecDeque};
use std::sync::Arc;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{
    SelectedBlock, SelectedBlockId, SelectedBlockOrigin, SelectedFunction, SelectedInstruction,
    SelectedInstructionId, SelectedInstructionPlan,
};
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{ScheduledRelocationError, ScheduledRelocationReceipt, ValidatedScheduledRelocation};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::{
    block_instructions, edge_surface, plain_edge, terminator_instruction, terminator_successors,
    transport_conflict,
};
use crate::rewrites::dead_path;
use crate::rewrites::window_hazards::{
    coupled, has_call_contract, register_writes, schedulable, surface,
};

/// The validator's own reconstruction of the relocation the contract
/// permits: the block the contiguous run vacates, the body positions it
/// occupies, the block it lands in, and the landing index. It shares no
/// state with the producer's `admission` record.
struct Reconstructed<'source> {
    function: &'source SelectedFunction,
    block_index: usize,
    member_first: usize,
    member_last: usize,
    target_index: usize,
    landing_index: usize,
}

/// Whether the member's effect is pure register and condition-state work
/// that cannot observe or abandon the execution it leaves behind — the
/// validator's own sinkable bar, kept separate from the producer's.
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

/// The validator's own continuation search: every simple continuation
/// leaving `from` reaches `to` before it exits or revisits a block.
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

struct Derived {
    region: BTreeSet<usize>,
    used_edges: BTreeSet<(usize, usize)>,
    exploration: usize,
}

/// The validator's own path enumeration: every simple path between the
/// run's block and the destination's, deriving the region and used-edge
/// sets the window audit applies to.
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

/// Reconstruct the legality of moving the run `members` onto
/// `destination` from the source records: locate the run and the
/// destination position by identity, bind the sink/hoist direction from
/// the topology's own dominance and continuation bounds, re-derive the
/// path-derived window — every block on a path plain `Source`, every
/// crossed edge plain and free of member interference, every run member
/// schedulable and sinkable, every crossed body position and crossed
/// terminator uncoupled, no boundary settlement observing a changed
/// prefix, and every location the run writes dead on the traversals a
/// sink abandons — and account the family's measured steps against the
/// budget. Nothing in this audit reads the producer's admission
/// decision, so a producer-side legality error fails here even when the
/// proposal matches the emitted edit.
fn reconstruct<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    members: &[SelectedInstructionId],
    destination: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Reconstructed<'source>, ScheduledRelocationError> {
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
    if target_index == block_index {
        return Err(ScheduledRelocationError::UnsupportedPair);
    }
    let entry_index = block_index_of(function, function.entry_block)
        .ok_or(ScheduledRelocationError::SourceMismatch)?;
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
            for edge in terminator_successors(&function.blocks[target_index].terminator) {
                let next = block_index_of(function, edge.block)
                    .ok_or(ScheduledRelocationError::SourceMismatch)?;
                if next == target_index || reaches_avoiding(function, next, None, target_index)? {
                    return Err(ScheduledRelocationError::UnsupportedPair);
                }
            }
        }
        Direction::Hoist => {
            for edge in terminator_successors(&function.blocks[block_index].terminator) {
                let next = block_index_of(function, edge.block)
                    .ok_or(ScheduledRelocationError::SourceMismatch)?;
                if next == block_index
                    || reaches_avoiding(function, next, Some(target_index), block_index)?
                {
                    return Err(ScheduledRelocationError::UnsupportedPair);
                }
            }
            let (reaches, expansions) = continuations_reach(function, target_index, block_index)?;
            search_steps = expansions;
            if !reaches {
                return Err(ScheduledRelocationError::UnsupportedPair);
            }
        }
    }
    let (window_from, window_to) = match direction {
        Direction::Sink => (block_index, target_index),
        Direction::Hoist => (target_index, block_index),
    };
    let derived = derive_paths(function, window_from, window_to, budget)?;
    if derived.region.is_empty() {
        return Err(ScheduledRelocationError::UnsupportedPair);
    }
    for index in &derived.region {
        if !matches!(
            function.blocks[*index].origin,
            SelectedBlockOrigin::Source(_)
        ) {
            return Err(ScheduledRelocationError::UnsupportedPair);
        }
    }
    for &(block, edge_position) in &derived.used_edges {
        let edge = terminator_successors(&function.blocks[block].terminator)[edge_position];
        if !plain_edge(edge) || run.iter().any(|member| transport_conflict(member, edge)) {
            return Err(ScheduledRelocationError::UnsupportedPair);
        }
    }
    for member in &run {
        if schedulable(function, member) != Some(false) || !sinkable(member) {
            return Err(ScheduledRelocationError::UnsupportedInstruction);
        }
    }
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
    for index in crossed_terminators {
        let terminator = terminator_instruction(&function.blocks[index].terminator);
        if has_call_contract(function, terminator.id) {
            return Err(ScheduledRelocationError::UnsupportedInstruction);
        }
        if run.iter().any(|member| coupled(member, terminator)) {
            return Err(ScheduledRelocationError::UnsupportedPair);
        }
    }
    if function.boundary_settlements.iter().any(|settlement| {
        (settlement.block == block.id && settlement.instruction_index as usize > member_first)
            || (settlement.block == function.blocks[target_index].id
                && settlement.instruction_index as usize > landing_index)
    }) {
        return Err(ScheduledRelocationError::UnsupportedPair);
    }
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
    Ok(Reconstructed {
        function,
        block_index,
        member_first,
        member_last,
        target_index,
        landing_index,
    })
}

/// Independently consume the proposed program: the validator re-derives
/// the admitted run, derived region, and landing index from the source
/// without the producer's admission routine, the proposal must place
/// exactly the run's instructions, contiguous and in order, on the
/// landing index in the destination block, and moving them back must
/// restore the complete source by content — every crossed instruction,
/// every other block and instruction, register, roster row, call,
/// settlement, and edge included.
pub fn validate_scheduled_relocation(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    members: &[SelectedInstructionId],
    destination: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedScheduledRelocation, ScheduledRelocationError> {
    let reconstructed = reconstruct(
        source,
        function_index,
        members,
        destination,
        environment,
        budget,
    )?;
    let span = reconstructed.member_last - reconstructed.member_first + 1;
    let source_run: Vec<&_> = reconstructed.function.blocks[reconstructed.block_index].instructions
        [reconstructed.member_first..=reconstructed.member_last]
        .iter()
        .collect();
    let proposed_function = proposed
        .functions
        .get(function_index)
        .ok_or(ScheduledRelocationError::ReplayMismatch)?;
    let proposed_target = proposed_function
        .blocks
        .get(reconstructed.target_index)
        .ok_or(ScheduledRelocationError::ReplayMismatch)?;
    let landed = proposed_target
        .instructions
        .get(reconstructed.landing_index..);
    if landed.map(|tail| tail.len() < span).unwrap_or(true)
        || source_run.iter().enumerate().any(|(offset, member)| {
            proposed_target.instructions[reconstructed.landing_index + offset] != **member
        })
    {
        return Err(ScheduledRelocationError::ReplayMismatch);
    }
    let mut restored = proposed.clone();
    let vacated: Vec<_> = restored.functions[function_index].blocks[reconstructed.target_index]
        .instructions
        .drain(reconstructed.landing_index..reconstructed.landing_index + span)
        .collect();
    restored.functions[function_index].blocks[reconstructed.block_index]
        .instructions
        .splice(
            reconstructed.member_first..reconstructed.member_first,
            vacated,
        );
    if restored != *source.selected_plan() {
        return Err(ScheduledRelocationError::ReplayMismatch);
    }
    Ok(ValidatedScheduledRelocation {
        receipt: ScheduledRelocationReceipt {
            source_selected: source.selected_identity(),
            transformed_selected: selected_instruction_plan_identity(&proposed),
            optimization_unit: source.optimization_unit_identity(),
            fuel_schedule: source.fuel_schedule_identity(),
        },
        transformed: Arc::new(proposed),
    })
}
