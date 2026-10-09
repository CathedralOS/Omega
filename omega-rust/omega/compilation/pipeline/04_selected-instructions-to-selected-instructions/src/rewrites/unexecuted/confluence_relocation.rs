//! Relocation of a member run into a confluence on the selected CFG.
//!
//! `super::relocation` carries a run across every shape whose traversal set
//! the move preserves, in either direction. This module takes the remaining
//! converging shape it refuses — the confluence, where the run starts
//! executing on traversals that reach the join by another edge: the named
//! `first_member` and `last_member` bound one contiguous run in a block's
//! body whose terminator is a lone `Jump` to a join at least one other
//! predecessor's edge also reaches, and the run leaves that body as a single
//! body — its members keeping their own order — and takes the named
//! `destination` instruction's position in the join. The destination and
//! every later position keep their relative order one run-width later. The
//! `dead_path` audit is what makes that sound.
//!
//! Naming one member as both bounds is the run of one, so the single-member
//! move is this admission's own granularity rather than a family of its own.
//! Moving several members as one body additionally admits the window a
//! one-member move refuses on internal coupling alone: a producer whose only
//! crossed reader is the run's own next member cannot leave its block — the member move
//! would starve the consumer it leaves behind — while the run carries
//! the consumer with it, so internally coupled members cross the
//! confluence together and only positions outside the run count as
//! crossed.
//!
//! Sinking a run into a confluence carries the member move's flipped
//! soundness burden at body granularity. The join's other inflows hand
//! the run's new position traversals it never executed on — speculation
//! downstream — so every member must be pure register and
//! condition-state work that can never fault: no barrier kind, no
//! call-roster entry, no memory roster rows, and no memory-capable or
//! potentially-faulting kind, because an access or trap that ran only on
//! this inflow would newly run on every arrival. And every register and
//! condition-state unit any member writes must be dead — unread until
//! rewritten — from the landing index forward: the join's tail, its
//! terminator, and every reachable successor position are shared by all
//! inflows, so a reader there observes the run's foreign definitions on
//! an arrival that never ran it. The forward dead-path audit walks the
//! join from the landing index, where the run's new position republishes
//! its members' locations on every arrival, and propagates them through
//! the successor region — reading each crossed edge's register,
//! structural-binding, and case-payload transports as boundary positions
//! — refusing the moment a still-live member definition meets a reader.
//! On this inflow's own path the relocated writes are the familiar ones
//! the source produced, but the shared region cannot tell the arrivals
//! apart, so the audit holds the die-unread rule on every continuation.
//!
//! The source side needs no region structure beyond the lone exit: the
//! run's block is any block ending in a plain `Jump` to the join — a
//! conditional terminator keeps a second exit the run would still
//! execute on, which is the fork, diamond, and triangle families'
//! shapes. The join needs only the in-edge count the single-edge family
//! excludes — at least one edge leaving another block — and the usual
//! boundary honesty: not the run's own block, which is the in-block
//! family's case with a back-edge reading, not the entry block, which is
//! reached with no predecessor at all, and a plain source block, because
//! an implementation block's origin carries edge or case work the
//! bounded audit does not cross. The crossed edge itself must be a plain
//! semantic successor: case custody, fuel, and structural transfers are
//! boundary effects the move does not cross.
//!
//! Between the run's old span and its new one lie only the crossed
//! positions the window audits: the run's own block tail behind it, the
//! `Jump` terminator instruction, the edge's register transports, and
//! the join's body instructions before the landing index. Positions
//! before the run, at or after the landing index, and in every other
//! block keep the run on the side they always had, so they are never
//! crossed. The join's other inflow edges are never traversed by the
//! run's old position — they run before the landing index on their own
//! arrivals — so their transports join only the dead-path audit's
//! boundary reading where loops carry the foreign set back around.
//!
//! The hazard audit is the shared run relocation's applied across the
//! boundary downstream — `block_edges::crossed_window` derives the lone
//! `Jump` edge's positions and `window_hazards::admit_run_relocation`
//! proves the window independent once at run granularity: a register or
//! condition-state unit any member writes and a crossed instruction
//! reads or writes, or any member reads and a crossed instruction
//! writes, refuses in either direction — which also
//! pins each member's inputs, so the run's execution at the landing
//! index on this inflow's path computes the values the old positions
//! computed. The edge's register transports join the audit per member
//! rather than through an instruction: a `Registers` transport reads its
//! argument and writes its parameter at the boundary, so a member
//! defining the transported argument would hand the binding the pre-move
//! value where the source bound the member's own write, a member
//! defining the parameter would be overwritten before the join observes
//! it, and a member reading the parameter would observe the transported
//! value only after the move. Calls, hosted effects, terminator kinds,
//! and call-roster entries are barriers anywhere in the window — the
//! `Jump` terminator itself is the crossed edge's position, not a window
//! member — and a boundary settlement refuses exactly where an executed
//! prefix changes: past the run's first index in its own block, or past
//! the landing index in the join block.
//!
//! No instruction, register, roster row, call, settlement, successor, or
//! terminator record changes: every member keeps its own id, kind,
//! operands, provenance, and implicit surface while only the run's
//! position in the program moves. Proposal and independent replay share
//! only the admission predicates. Validation consumes the proposed
//! program, requires the run to sit at the landing index in its original
//! order, and restores the complete source by content — every other
//! block, instruction, register, roster row, call, and settlement is
//! retained bit-identical.

mod admission;
mod rewrite;
mod validation;

use std::sync::Arc;

use optimization_core::OptimizationUnitIdentity;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{
    SelectedBlockOrigin, SelectedFunction, SelectedInstruction, SelectedInstructionId,
    SelectedInstructionPlan, SelectedInstructionPlanIdentity, SelectedSuccessor,
    SelectedTerminator, instruction_position, terminator_instruction,
};
use semantic_vocabulary::FuelScheduleIdentity;

use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::{all_edges, member_position, plain_edge};
use crate::rewrites::unexecuted::dead_path;
use crate::rewrites::window_hazards::{RunRelocationRejection, schedulable, speculatable};

pub(crate) use validation::validate_confluence_relocation;

/// An accepted cross-confluence run relocation with its replay receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ValidatedConfluenceRelocation {
    transformed: Arc<SelectedInstructionPlan>,
    receipt: ConfluenceRelocationReceipt,
}

validated_transformed_receipt!(ValidatedConfluenceRelocation, ConfluenceRelocationReceipt);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConfluenceRelocationReceipt {
    source_selected: SelectedInstructionPlanIdentity,
    transformed_selected: SelectedInstructionPlanIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
}

receipt_selected_identities!(ConfluenceRelocationReceipt);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConfluenceRelocationError {
    SourceMismatch,
    /// A barrier kind, a call-roster entry, a roster-carrying or
    /// unaccounted memory-capable kind, or a potentially-faulting kind
    /// sits at a position the move would cross — a run member included:
    /// an access or trap that ran only on this inflow would newly run on
    /// every arrival through the join's other inflows.
    UnsupportedInstruction,
    /// The named triple does not bound an admissible cross-confluence run
    /// window: the named members bound no contiguous run of at least two
    /// members in one block's body, the run's block does not end in a
    /// lone unconditional `Jump`, the crossed edge is not a plain
    /// semantic successor or carries case, fuel, or structural
    /// transfers, the join is the run's block, the entry block, a
    /// non-source block, or reached by no edge leaving another block —
    /// the sole-predecessor shape is the edge family's case — the
    /// destination names no body or terminator instruction there, a
    /// register or condition-state hazard couples a member with a
    /// crossed position or the crossed edge's transports, a boundary
    /// settlement observes a changed executed prefix, or the dead-path
    /// audit finds a register or unit a member writes still live at a
    /// reader on a shared continuation.
    UnsupportedPair,
    WorkBudgetExceeded,
    IdentityOverflow,
    ReplayMismatch,
}

impl std::fmt::Display for ConfluenceRelocationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid confluence run relocation: {self:?}")
    }
}

impl std::error::Error for ConfluenceRelocationError {}

/// The step base every confluence audit charges before its own surfaces:
/// the shared rewrite scan, one scan of the function's body and terminator
/// instructions to locate the run's first member and find the join's other inflow, and each member's
/// register, implicit-def, and clobber locations.
pub(super) fn confluence_scan_base(
    plan: &selected_instructions::SelectedInstructionPlan,
    function: &selected_instructions::SelectedFunction,
    run: &[selected_instructions::SelectedInstruction],
) -> Option<(usize, usize, usize)> {
    let run_locations: usize = run
        .iter()
        .map(|member| {
            crate::rewrites::window_hazards::register_writes(member).count()
                + member.implicit_defs.len()
                + member.clobbers.len()
        })
        .sum();
    let block_scan = crate::rewrites::window_hazards::block_scan_surface(function);
    let scan = crate::rewrites::rewrite_scan(plan, function).and_then(|total| {
        function.blocks.iter().try_fold(total, |total, candidate| {
            candidate
                .instructions
                .iter()
                .chain(std::iter::once(
                    selected_instructions::terminator_instruction(&candidate.terminator),
                ))
                .try_fold(total, |total, _| total.checked_add(1))
        })
    })?;
    Some((scan, run_locations, block_scan))
}

/// The tail of the family's measured step: the audit's own surfaces plus
/// the function's three rosters and the dead-path walk's rescans, charged
/// against the budget as the family's shared overflow and budget terms.
pub(super) fn confluence_settle_steps(
    total: usize,
    function: &selected_instructions::SelectedFunction,
    block_scan: usize,
    run_locations: usize,
    budget: optimization_core::OptimizationWorkBudget,
) -> Result<(), ConfluenceRelocationError> {
    let steps = total
        .checked_add(function.memory_accesses.len())
        .and_then(|total| total.checked_add(function.calls.len()))
        .and_then(|total| total.checked_add(function.boundary_settlements.len()))
        .and_then(|total| {
            total.checked_add(block_scan.saturating_mul(run_locations.saturating_add(1)))
        })
        .ok_or(ConfluenceRelocationError::IdentityOverflow)?;
    crate::rewrites::charge_steps(
        steps,
        budget,
        || ConfluenceRelocationError::IdentityOverflow,
        || ConfluenceRelocationError::WorkBudgetExceeded,
    )?;
    Ok(())
}

/// The shared derivation of the run's confluence legality both the
/// admission and the validator prove: the run's contiguous span in its
/// block, the lone `Jump` edge it leaves through, the join block it
/// sinks to, the landing position the destination names, and the
/// pure-work bar every member meets. The window audit and the step
/// accounting stay with each side.
struct ConfluenceDerivation<'source> {
    plan: &'source SelectedInstructionPlan,
    function: &'source SelectedFunction,
    block_index: usize,
    first_index: usize,
    last_index: usize,
    run: &'source [SelectedInstruction],
    target_index: usize,
    landing_index: usize,
    successor: &'source SelectedSuccessor,
    terminator: &'source SelectedInstruction,
}

/// Re-derive the confluence relocation's legality from the source
/// records: the run's span, the `Jump` edge's plain successor, the
/// join's origin and other-inflow reach, the landing index, and every
/// member's schedulable and speculatable bar.
fn derive_confluence_span<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    first_member: SelectedInstructionId,
    last_member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
) -> Result<ConfluenceDerivation<'source>, ConfluenceRelocationError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(ConfluenceRelocationError::SourceMismatch);
    }
    let function = function_at(
        plan,
        function_index,
        ConfluenceRelocationError::SourceMismatch,
    )?;
    let (block_index, first_index) =
        member_position(function, first_member).ok_or(ConfluenceRelocationError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    // The run is the contiguous span the two named members bound in this
    // block, in the named order; a repeated id or a last member that does
    // not follow the first names no multi-member run. A single member is
    // the one-instruction confluence relocation the sibling family
    // already proves.
    let last_index = block
        .instruction_index(last_member)
        .filter(|position| *position >= first_index)
        .ok_or(ConfluenceRelocationError::UnsupportedPair)?;
    let run = &block.instructions[first_index..=last_index];
    // Only an unconditional semantic jump edge keeps the run's execution
    // count on this inflow's path: every traversal of the run's block
    // leaves through it and reaches the join exactly once. A conditional
    // terminator keeps a second exit the run would still execute on —
    // the fork, diamond, and triangle families' shapes — and a
    // terminator naming no successors never reaches a join.
    let SelectedTerminator::Jump {
        instruction: terminator,
        successor,
    } = &block.terminator
    else {
        return Err(ConfluenceRelocationError::UnsupportedPair);
    };
    use crate::rewrites::block_edges::function_at;
    // The crossed edge must be a plain semantic successor: case dispatch,
    // continuation, structural transfer, and per-edge fuel all carry
    // boundary effects this step does not cross. Structural bindings may
    // remain only while every transport is `Unused`, which moves nothing.
    if !plain_edge(successor) {
        return Err(ConfluenceRelocationError::UnsupportedPair);
    }
    let target_index = function
        .block_index(successor.block)
        .ok_or(ConfluenceRelocationError::SourceMismatch)?;
    let target = &function.blocks[target_index];
    // A self-edge is the in-block family's case with a back-edge
    // transport reading, not a cross-edge window. The entry block is
    // reached with no predecessor at all, and an implementation block's
    // origin carries edge or case work the bounded audit does not cross.
    if target.id == block.id
        || target.id == function.entry_block
        || !matches!(target.origin, SelectedBlockOrigin::Source(_))
    {
        return Err(ConfluenceRelocationError::UnsupportedPair);
    }
    // At least one edge into the join must leave another block: the
    // single-predecessor shape is the edge family's case, where no
    // traversal speculates. Every other inflow — whatever it carries —
    // runs before the landing index on its own arrivals and never crosses
    // the run's window; the dead-path audit reads its register surface
    // where a loop reaches it.
    if !all_edges(function)
        .any(|(source_block, edge)| edge.block == target.id && source_block != block.id)
    {
        return Err(ConfluenceRelocationError::UnsupportedPair);
    }
    // The destination names a position in the join body — the run lands
    // at its index — or the join's terminator-carried instruction,
    // landing the run at the body end.
    let landing_index = instruction_position(&target.instructions, destination)
        .or_else(|| {
            (terminator_instruction(&target.terminator).id == destination)
                .then_some(target.instructions.len())
        })
        .ok_or(ConfluenceRelocationError::UnsupportedPair)?;
    // The run's execution newly runs on every arrival through the join's
    // other inflows, so only pure register and condition-state work may
    // sink — a roster-carrying or unaccounted memory access or a
    // potentially-faulting kind that ran only on this inflow would run on
    // every arrival after the move. Every member meets the bar itself;
    // the run then carries no roster rows at all, so it crosses any
    // accounted mix of positions without reordering a recorded access.
    for member in run {
        if schedulable(function, member) != Some(false) || !speculatable(member) {
            return Err(ConfluenceRelocationError::UnsupportedInstruction);
        }
    }

    Ok(ConfluenceDerivation {
        plan,
        function,
        block_index,
        first_index,
        last_index,
        run,
        target_index,
        landing_index,
        successor,
        terminator,
    })
}

/// The dead-path audit the move owes: every location any member writes
/// must be dead — unread until rewritten — from the landing index
/// forward, on the shared continuations every inflow reaches. The run
/// publishes the union of its members' locations.
fn run_writes_dead(function: &SelectedFunction, derived: &ConfluenceDerivation<'_>) -> bool {
    let members: Vec<&SelectedInstruction> = derived.run.iter().collect();
    dead_path::dead(
        function,
        dead_path::Relocation {
            members: &members,
            vacated_block: derived.block_index,
            vacated_first: derived.first_index,
            vacated_last: derived.last_index,
            landing_block: derived.target_index,
            landing_index: derived.landing_index,
            landing: dead_path::Landing::Speculated,
            // The vacated span stays silent: every position behind it in
            // the run's own block is a crossed window position the hazard
            // audit already owns, and no surviving position sits inside
            // the span.
            vacated: dead_path::Vacated::Silent,
        },
        dead_path::Start::Block(derived.target_index),
    )
}

/// Keeps the family's typed rejection vocabulary over the shared audit's
/// rejection kinds: an unschedulable member or crossed position is the
/// instruction-level refusal and every window-level refusal is the pair
/// kind.
fn rejection(rejection: RunRelocationRejection) -> ConfluenceRelocationError {
    match rejection {
        RunRelocationRejection::Unschedulable => ConfluenceRelocationError::UnsupportedInstruction,
        RunRelocationRejection::UnreachableDestination
        | RunRelocationRejection::Coupled
        | RunRelocationRejection::MemoryOrdering
        | RunRelocationRejection::TransportConflict
        | RunRelocationRejection::NonPlainEdge
        | RunRelocationRejection::Settlement => ConfluenceRelocationError::UnsupportedPair,
    }
}
