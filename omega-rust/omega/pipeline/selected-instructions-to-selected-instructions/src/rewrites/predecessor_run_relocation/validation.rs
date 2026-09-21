use std::sync::Arc;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{
    SelectedBlockOrigin, SelectedFunction, SelectedInstructionId, SelectedInstructionPlan,
    SelectedTerminator,
};
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{
    PredecessorRunRelocationError, PredecessorRunRelocationReceipt,
    ValidatedPredecessorRunRelocation,
};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::{
    CrossingDirection, all_edges, crossed_window, plain_edge, terminator_instruction,
    terminator_successors,
};
use crate::rewrites::window_hazards::{RunRelocationRejection, admit_run_relocation, surface};

/// The validator's own reconstruction of the relocation the contract
/// permits: the run's and the sole predecessor's blocks, the run's
/// contiguous span in its block's body, and the landing position the
/// move takes in the predecessor body. It shares no state with the
/// producer's `admission` record.
struct Reconstructed<'source> {
    function: &'source SelectedFunction,
    /// The run's own block — the crossed edge's successor.
    block_index: usize,
    /// The run's first member index in the block body.
    first_index: usize,
    /// The run's last member index; the run is the contiguous span
    /// `first_index..=last_index` of at least two members.
    last_index: usize,
    /// The sole predecessor's index in `function.blocks` — the block the
    /// crossed `Jump` edge leaves.
    target_index: usize,
    /// The body index the run lands at in the predecessor: the named
    /// destination instruction's own index, or the body end when the
    /// destination names the predecessor's terminator-carried `Jump`.
    landing_index: usize,
}

/// Keeps the family's typed rejection vocabulary over the shared audit's
/// rejection kinds: an unschedulable member or crossed position is the
/// instruction-level refusal and every window-level refusal is the pair
/// kind.
fn reject(rejection: RunRelocationRejection) -> PredecessorRunRelocationError {
    match rejection {
        RunRelocationRejection::Unschedulable => {
            PredecessorRunRelocationError::UnsupportedInstruction
        }
        RunRelocationRejection::UnreachableDestination
        | RunRelocationRejection::Coupled
        | RunRelocationRejection::MemoryOrdering
        | RunRelocationRejection::TransportConflict
        | RunRelocationRejection::NonPlainEdge
        | RunRelocationRejection::Settlement => PredecessorRunRelocationError::UnsupportedPair,
    }
}

/// Reconstruct the legality of moving the run `first_member..=last_member`
/// onto `destination` from the source records: locate the bounding members
/// by identity inside one block's body, require that block to be reached
/// by exactly one edge — the unconditional `Jump` the predecessor ends in
/// on a plain semantic edge — resolve the landing index in the predecessor
/// body, and re-derive the window's independence audit through the shared
/// `crossed_window`/`admit_run_relocation` contract — no register or
/// condition-state hazard between any member and any crossed position, no
/// member interference with the edge's register transports, no
/// roster-carrying run sharing the window with a second memory-access
/// actor, no barrier, call, hosted effect, or call-roster entry inside the
/// window, and no boundary settlement whose observed executed prefix
/// changes — then account the family's measured steps against the budget.
/// Nothing in this audit reads the producer's admission decision, so a
/// producer-side legality error fails here even when the proposal matches
/// the emitted edit.
fn reconstruct<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    first_member: SelectedInstructionId,
    last_member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Reconstructed<'source>, PredecessorRunRelocationError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(PredecessorRunRelocationError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(PredecessorRunRelocationError::SourceMismatch)?;
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
        .ok_or(PredecessorRunRelocationError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    // The run is the contiguous span the two named members bound in this
    // block, in the named order; a repeated id or a last member that does
    // not follow the first names no multi-member run. A single member is
    // the one-instruction predecessor relocation the sibling family
    // already proves.
    let last_index = block
        .instructions
        .iter()
        .position(|instruction| instruction.id == last_member)
        .filter(|position| *position > first_index)
        .ok_or(PredecessorRunRelocationError::UnsupportedPair)?;
    let run = &block.instructions[first_index..=last_index];
    // Every edge into the run's block must be the one crossed edge: a
    // second predecessor gives the block a path the run would stop
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
        return Err(PredecessorRunRelocationError::UnsupportedPair);
    };
    if incoming.next().is_some() {
        return Err(PredecessorRunRelocationError::UnsupportedPair);
    }
    let target = &function.blocks[target_index];
    // Only an unconditional jump keeps the run's execution count: every
    // traversal of the predecessor leaves through it into the run's
    // block, so the run executes once per predecessor traversal on either
    // side of the move. A conditional predecessor keeps a second exit the
    // run would newly execute on after relocating. The found edge is this
    // terminator's one successor, so `successor` names the crossed edge
    // directly.
    let SelectedTerminator::Jump {
        instruction: terminator,
        successor,
    } = &target.terminator
    else {
        return Err(PredecessorRunRelocationError::UnsupportedPair);
    };
    // The crossed edge must be a plain semantic successor: case dispatch,
    // continuation, structural transfer, and per-edge fuel all carry
    // boundary effects this step does not cross.
    if !plain_edge(successor) {
        return Err(PredecessorRunRelocationError::UnsupportedPair);
    }
    // A self-edge is the in-block run family's case with a back-edge
    // transport reading, not a cross-edge window. The entry block is
    // reached with no predecessor at all — an edge naming it would feed
    // it a traversal entry never saw — and an implementation block's
    // origin carries edge or case work the bounded audit does not cross.
    if target.id == block.id
        || block.id == function.entry_block
        || !matches!(target.origin, SelectedBlockOrigin::Source(_))
    {
        return Err(PredecessorRunRelocationError::UnsupportedPair);
    }
    // The destination names a position in the predecessor body — the run
    // lands at its index — or the predecessor's terminator-carried `Jump`
    // instruction, landing the run at the body end.
    let landing_index = target
        .instructions
        .iter()
        .position(|instruction| instruction.id == destination)
        .or_else(|| {
            (terminator_instruction(&target.terminator).id == destination)
                .then_some(target.instructions.len())
        })
        .ok_or(PredecessorRunRelocationError::UnsupportedPair)?;
    // The crossed window is the shared derivation rather than this family's
    // own enumeration: the gates above leave exactly one acyclic path —
    // this `Jump` edge — so the backward path walk is bounded by the
    // function's edge roster alone. The shared audit applies the hazard,
    // memory-roster, transport, and settlement checks once across the run's
    // members.
    let edge_limit = all_edges(function).count();
    let crossing = crossed_window(
        function,
        block_index,
        first_index,
        last_index,
        target_index,
        landing_index,
        CrossingDirection::Backward,
        edge_limit,
    )
    .ok_or(PredecessorRunRelocationError::WorkBudgetExceeded)?;
    let members: Vec<_> = run.iter().collect();
    admit_run_relocation(function, &members, &crossing).map_err(reject)?;
    // The validator's own audit walks the same surfaces the family
    // publishes: every block body and terminator instruction once to
    // locate the run's first member and count the block's predecessor
    // edges, each path edge once, and every member's surface against each
    // crossed position's, plus the function's three rosters and the edge's
    // binding roster once per member.
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
        .and_then(|total| total.checked_add(edge_limit))
        .and_then(|total| {
            run.iter().try_fold(total, |total, member| {
                target.instructions[landing_index..]
                    .iter()
                    .chain(block.instructions[..first_index].iter())
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
                .checked_add(function.boundary_settlements.len())?
                .checked_add(successor.bindings.len().saturating_mul(run.len()))
        })
        .ok_or(PredecessorRunRelocationError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| PredecessorRunRelocationError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(PredecessorRunRelocationError::WorkBudgetExceeded);
    }
    Ok(Reconstructed {
        function,
        block_index,
        first_index,
        last_index,
        target_index,
        landing_index,
    })
}

/// Independently consume the proposed program: the validator re-derives
/// the admitted run, edge, and landing index from the source without the
/// producer's admission routine, the proposal must place exactly the
/// run's members on the landing index in the predecessor block in their
/// original order, and moving the run back must restore the complete
/// source by content — every crossed instruction, every other block and
/// instruction, register, roster row, call, settlement, and edge
/// included.
pub fn validate_predecessor_run_relocation(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    first_member: SelectedInstructionId,
    last_member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedPredecessorRunRelocation, PredecessorRunRelocationError> {
    let reconstructed = reconstruct(
        source,
        function_index,
        first_member,
        last_member,
        destination,
        environment,
        budget,
    )?;
    let run_len = reconstructed.last_index - reconstructed.first_index + 1;
    if proposed
        .functions
        .get(function_index)
        .and_then(|function| function.blocks.get(reconstructed.target_index))
        .and_then(|block| {
            block
                .instructions
                .get(reconstructed.landing_index..reconstructed.landing_index + run_len)
        })
        != Some(
            &reconstructed.function.blocks[reconstructed.block_index].instructions
                [reconstructed.first_index..=reconstructed.last_index],
        )
    {
        return Err(PredecessorRunRelocationError::ReplayMismatch);
    }
    let mut restored = proposed.clone();
    let run: Vec<_> = restored.functions[function_index].blocks[reconstructed.target_index]
        .instructions
        .drain(reconstructed.landing_index..reconstructed.landing_index + run_len)
        .collect();
    restored.functions[function_index].blocks[reconstructed.block_index]
        .instructions
        .splice(reconstructed.first_index..reconstructed.first_index, run);
    if restored != *source.selected_plan() {
        return Err(PredecessorRunRelocationError::ReplayMismatch);
    }
    Ok(ValidatedPredecessorRunRelocation {
        receipt: PredecessorRunRelocationReceipt {
            source_selected: source.selected_identity(),
            transformed_selected: selected_instruction_plan_identity(&proposed),
            optimization_unit: source.optimization_unit_identity(),
            fuel_schedule: source.fuel_schedule_identity(),
        },
        transformed: Arc::new(proposed),
    })
}
