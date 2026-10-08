//! Shared admission for inflow relocation: locate the named `member` in
//! one block's body, locate the named `destination` in a block that ends
//! in the lone unconditional `Jump` reaching the member's block — a plain
//! semantic successor edge — require the member's block to be a join at
//! least one other predecessor's edge also reaches, and prove the move
//! sound in both directions — the crossed window independent, handed to
//! the shared run audit (`crossed_window` derives the positions and edges
//! every acyclic path between the inflow and the join crosses — exactly
//! this `Jump` edge under the gates below — and `admit_run_relocation`
//! proves the window independent once: no register or condition-state
//! hazard between the member and any crossed position, no interference
//! with the crossed edge's register transports, no barrier, call, hosted
//! effect, or call-roster entry inside the window, and no boundary
//! settlement whose observed executed prefix changes) and every
//! member-written location dead on the paths the move removes, where the
//! member's vacated index publishes them on arrivals that no longer run
//! it.
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::SelectedInstructionId;

use super::InflowRelocationError;
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::{
    CrossingDirection, all_edges, crossed_window, terminator_successors,
};
use crate::rewrites::window_hazards::{admit_run_relocation, relocation_scan_steps};

pub(super) struct Admission {
    /// The member's own block: the join the inflow feeds.
    pub block_index: usize,
    /// The member's index inside that block's body.
    pub member_index: usize,
    /// The inflow block's index in `function.blocks`: the predecessor the
    /// destination names.
    pub target_index: usize,
    /// The destination instruction's position in the inflow body: the
    /// member lands at this index. Naming the inflow's terminator-carried
    /// instruction lands the member at the body end, index
    /// `target.instructions.len()`.
    pub landing_index: usize,
}

pub(super) fn admit(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Admission, InflowRelocationError> {
    let derived = super::derive_inflow_span(
        source,
        function_index,
        member,
        destination,
        environment,
    )?;
    let plan = derived.plan;
    let function = derived.function;
    let block_index = derived.block_index;
    let member_index = derived.member_index;
    let member_instruction = derived.member_instruction;
    let target_index = derived.target_index;
    let landing_index = derived.landing_index;
    // The crossed window is the shared derivation rather than this
    // family's own enumeration: the member is the one-member run, and the
    // gates above leave exactly one acyclic path — the inflow's `Jump`
    // edge is the block's lone successor — so the backward path walk is
    // bounded by the function's edge roster alone. The shared audit
    // applies the hazard, memory-roster, transport, and settlement
    // checks once: the `Jump` instruction is the crossed edge's own
    // position, exempt from the barrier-kind rule but not from the call
    // or hazard audit, and a settlement positioned past the member's
    // index in the join or past the landing index in the inflow observed
    // a changed executed prefix and refuses. The other inflow blocks are
    // unaffected: the member never enters their streams.
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
    .ok_or(InflowRelocationError::WorkBudgetExceeded)?;
    admit_run_relocation(function, &[member_instruction], &crossing)
        .map_err(super::rejection)?;
    // The dead-path audit: every location the member writes must be dead
    // — unread until rewritten — on every arrival through the join's
    // other inflows.
    if !super::member_writes_dead(function, &derived) {
        return Err(InflowRelocationError::UnsupportedPair);
    }
    // The scan walks every block body and terminator instruction once to
    // locate the member, and again with successor edges to locate the
    // destination and find the join's other inflows; the path walk touches
    // each edge once; the window audit walks the member's surface against
    // each crossed position's and each crossed edge's own surface; the
    // dead-path audit rescans a block's stream and edge surfaces only
    // while its entry set grows — at most once per member location per
    // block. The inflow scan also touches each block's successor edges.
    let steps = relocation_scan_steps(
        plan,
        function,
        member_instruction,
        &crossing,
        edge_limit,
        &|candidate| terminator_successors(&candidate.terminator).len(),
    )
    .ok_or(InflowRelocationError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| InflowRelocationError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(InflowRelocationError::WorkBudgetExceeded);
    }
    Ok(Admission {
        block_index: derived.block_index,
        member_index: derived.member_index,
        target_index: derived.target_index,
        landing_index: derived.landing_index,
    })
}
