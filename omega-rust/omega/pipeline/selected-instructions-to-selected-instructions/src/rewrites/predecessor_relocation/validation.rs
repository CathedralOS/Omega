use std::sync::Arc;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{
    SelectedBlockOrigin, SelectedFunction, SelectedInstructionId, SelectedInstructionPlan,
    SelectedTerminator,
};
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{
    PredecessorRelocationError, PredecessorRelocationReceipt, ValidatedPredecessorRelocation,
};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::{
    CrossingDirection, all_edges, crossed_window, plain_edge, terminator_instruction,
    terminator_successors,
};
use crate::rewrites::window_hazards::{RunRelocationRejection, admit_run_relocation, surface};

/// The validator's own reconstruction of the relocation the contract
/// permits: the member's and the sole predecessor's blocks, the member's
/// position in its block, and the landing position the move takes in the
/// predecessor body. It shares no state with the producer's `admission`
/// record.
struct Reconstructed<'source> {
    function: &'source SelectedFunction,
    /// The member's own block — the crossed edge's successor.
    block_index: usize,
    /// The member's index inside that block's body.
    member_index: usize,
    /// The sole predecessor's index in `function.blocks` — the block the
    /// crossed `Jump` edge leaves.
    target_index: usize,
    /// The body index the member lands at in the predecessor: the named
    /// destination instruction's own index, or the body end when the
    /// destination names the predecessor's terminator-carried `Jump`.
    landing_index: usize,
}

/// Keeps the family's typed rejection vocabulary over the shared audit's
/// rejection kinds: an unschedulable member or crossed position is the
/// instruction-level refusal and every window-level refusal is the pair
/// kind.
fn reject(rejection: RunRelocationRejection) -> PredecessorRelocationError {
    match rejection {
        RunRelocationRejection::Unschedulable => PredecessorRelocationError::UnsupportedInstruction,
        RunRelocationRejection::UnreachableDestination
        | RunRelocationRejection::Coupled
        | RunRelocationRejection::MemoryOrdering
        | RunRelocationRejection::TransportConflict
        | RunRelocationRejection::NonPlainEdge
        | RunRelocationRejection::Settlement => PredecessorRelocationError::UnsupportedPair,
    }
}

/// Reconstruct the legality of moving `member` onto `destination` from the
/// source records: locate the member by identity, require its block to be
/// reached by exactly one edge — the unconditional `Jump` the predecessor
/// ends in on a plain semantic edge — resolve the landing index in the
/// predecessor body, and re-derive the window's independence audit through
/// the shared `crossed_window`/`admit_run_relocation` contract — no
/// register or condition-state hazard between the member and any crossed
/// position, no interference with the edge's register transports, no
/// roster-carrying member sharing the window with a second memory-access
/// actor, no barrier, call, hosted effect, or call-roster entry inside the
/// window, and no boundary settlement whose observed executed prefix
/// changes — then account the family's measured steps against the budget.
/// Nothing in this audit reads the producer's admission decision, so a
/// producer-side legality error fails here even when the proposal matches
/// the emitted edit.
fn reconstruct<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Reconstructed<'source>, PredecessorRelocationError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(PredecessorRelocationError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(PredecessorRelocationError::SourceMismatch)?;
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
        .ok_or(PredecessorRelocationError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    let member_instruction = &block.instructions[member_index];
    // Every edge into the member's block must be the one crossed edge: a
    // second predecessor gives the block a path the member would stop
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
        return Err(PredecessorRelocationError::UnsupportedPair);
    };
    if incoming.next().is_some() {
        return Err(PredecessorRelocationError::UnsupportedPair);
    }
    let target = &function.blocks[target_index];
    // Only an unconditional jump keeps the member's execution count: every
    // traversal of the predecessor leaves through it into the member's
    // block, so the member executes once per predecessor traversal on
    // either side of the move. A conditional predecessor keeps a second
    // exit the member would newly execute on after relocating. The found
    // edge is this terminator's one successor, so `successor` names the
    // crossed edge directly.
    let SelectedTerminator::Jump {
        instruction: terminator,
        successor,
    } = &target.terminator
    else {
        return Err(PredecessorRelocationError::UnsupportedPair);
    };
    // The crossed edge must be a plain semantic successor: case dispatch,
    // continuation, structural transfer, and per-edge fuel all carry
    // boundary effects this step does not cross. Structural bindings may
    // remain only while every transport is `Unused`, which moves nothing.
    if !plain_edge(successor) {
        return Err(PredecessorRelocationError::UnsupportedPair);
    }
    // A self-edge is the in-block family's case with a back-edge transport
    // reading, not a cross-edge window. The entry block is reached with no
    // predecessor at all — an edge naming it would feed it a traversal
    // entry never saw — and an implementation block's origin carries edge
    // or case work the bounded audit does not cross.
    if target.id == block.id
        || block.id == function.entry_block
        || !matches!(target.origin, SelectedBlockOrigin::Source(_))
    {
        return Err(PredecessorRelocationError::UnsupportedPair);
    }
    // The destination names a position in the predecessor body — the
    // member lands at its index — or the predecessor's terminator-carried
    // `Jump` instruction, landing the member at the body end.
    let landing_index = target
        .instructions
        .iter()
        .position(|instruction| instruction.id == destination)
        .or_else(|| {
            (terminator_instruction(&target.terminator).id == destination)
                .then_some(target.instructions.len())
        })
        .ok_or(PredecessorRelocationError::UnsupportedPair)?;
    // The crossed window is the shared derivation rather than this family's
    // own enumeration: the gates above leave exactly one acyclic path —
    // this `Jump` edge — so the backward path walk is bounded by the
    // function's edge roster alone. The shared audit applies the hazard,
    // memory-roster, transport, and settlement checks once.
    let edge_limit = all_edges(function).count();
    let crossing = crossed_window(
        function,
        block_index,
        member_index,
        member_index,
        target_index,
        landing_index,
        CrossingDirection::Backward,
        edge_limit,
    )
    .ok_or(PredecessorRelocationError::WorkBudgetExceeded)?;
    admit_run_relocation(function, &[member_instruction], &crossing).map_err(reject)?;
    // The validator's own audit walks the same surfaces the family
    // publishes: every block body and terminator instruction once to
    // locate the member and count the block's predecessor edges, each path
    // edge once, and the member's surface against each crossed position's,
    // plus the function's three rosters and the edge's binding roster.
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
            target.instructions[landing_index..]
                .iter()
                .chain(block.instructions[..member_index].iter())
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
        .ok_or(PredecessorRelocationError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| PredecessorRelocationError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(PredecessorRelocationError::WorkBudgetExceeded);
    }
    Ok(Reconstructed {
        function,
        block_index,
        member_index,
        target_index,
        landing_index,
    })
}

/// Independently consume the proposed program: the validator re-derives
/// the admitted member, edge, and landing index from the source without
/// the producer's admission routine, the proposal must place exactly the
/// member's instruction on the landing index in the predecessor block,
/// and moving it back must restore the complete source by content —
/// every crossed instruction, every other block and instruction,
/// register, roster row, call, settlement, and edge included.
pub fn validate_predecessor_relocation(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedPredecessorRelocation, PredecessorRelocationError> {
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
        return Err(PredecessorRelocationError::ReplayMismatch);
    }
    let mut restored = proposed.clone();
    let member_instruction = restored.functions[function_index].blocks[reconstructed.target_index]
        .instructions
        .remove(reconstructed.landing_index);
    restored.functions[function_index].blocks[reconstructed.block_index]
        .instructions
        .insert(reconstructed.member_index, member_instruction);
    if restored != *source.selected_plan() {
        return Err(PredecessorRelocationError::ReplayMismatch);
    }
    Ok(ValidatedPredecessorRelocation {
        receipt: PredecessorRelocationReceipt {
            source_selected: source.selected_identity(),
            transformed_selected: selected_instruction_plan_identity(&proposed),
            optimization_unit: source.optimization_unit_identity(),
            fuel_schedule: source.fuel_schedule_identity(),
        },
        transformed: Arc::new(proposed),
    })
}
