//! Instruction relocation upstream onto one inflow of a confluence on the
//! selected CFG.
//!
//! The landed upstream families move a member out of its block across a
//! shape whose every traversal carries it: `rewrites/predecessor_relocation`
//! crosses the sole edge into a sole-predecessor block, and
//! `rewrites/arm_relocation`, `rewrites/join_relocation`, and
//! `rewrites/triangle_relocation` rise through a fork head whose edges
//! alone reach the member's region. The downstream `rewrites/confluence_relocation`
//! sinks a member through its block's lone `Jump` into a join at least one
//! other predecessor's edge also reaches, speculating the member onto
//! arrivals it never ran on. This family is that confluence move in the
//! upstream direction: the named `member` leaves its own block's body — a
//! block at least one other inflow edge also reaches — crosses the one
//! inflow edge the named `destination`'s block ends in, and takes the
//! destination's position inside that predecessor. The destination and
//! every later position keep their relative order one slot later, so the
//! member lands exactly where the destination sat and every position the
//! move crosses keeps its relative order.
//!
//! Hoisting onto one inflow flips the burden the downstream move carries.
//! `confluence_relocation` adds the member's execution to the join's other
//! arrivals; this move removes it: the member runs once per traversal of
//! the inflow block, whose lone `Jump` reaches the member's block exactly
//! once — so it still runs on every arrival through the crossed edge and
//! never again on an arrival through any other. No traversal gains an
//! execution, but a traversal that loses one must observe no difference:
//! the member must be pure register and condition-state work that cannot
//! fault, because a potentially-faulting kind or a memory access that ran
//! on every arrival would leave a trap or an access the other inflows'
//! continuations no longer perform. And every register and
//! condition-state unit the member writes must be dead — unread until
//! rewritten — along every path entering through the other inflows: the
//! member's write is missing where the source still ran it, so a reader
//! there meets a stale value where the source met the member's.
//!
//! The dead-path audit walks each other inflow edge into the member's
//! block, where the vacated index publishes the member's locations as
//! still-live — the divergence the missing write leaves behind — and
//! propagates them through the successor region, reading each crossed
//! edge's register, structural-binding, and case-payload transports as
//! boundary positions. A walked path that loops back through the inflow
//! block runs the member at its new position, where the landing is an
//! ordinary execution: the write republishes every location it carries
//! and clears the live set, matching what the source's member wrote on
//! the same traversal.
//!
//! The inflow side needs no region structure beyond the lone exit: the
//! destination's block is any plain source block ending in a `Jump` to
//! the member's block — a conditional terminator keeps a second exit the
//! member would newly execute on, which is the arm and join families'
//! burden, and a second edge out of the predecessor is the same shape.
//! The member's block needs only the inflow count the sole-predecessor
//! family excludes — at least one edge leaving another block — and the
//! usual boundary honesty: it may not be the entry block, whose first
//! traversal crosses no inflow edge at all, and the predecessor may not
//! be the member's own block, which is the in-block family's case with a
//! back-edge reading. The crossed edge itself must be a plain semantic
//! successor: case custody, fuel, and structural transfers are boundary
//! effects the move does not cross. The other inflow edges are never
//! crossed — whatever they carry runs before the member's old position on
//! their own arrivals — so they join only the dead-path audit's boundary
//! reading.
//!
//! Between the member's old position and its new one lie only the crossed
//! positions the window audits: the predecessor's body instructions at
//! and after the landing index, its `Jump` terminator instruction, the
//! edge's register transports, and the member's own block's body
//! instructions before its index. Positions after the member's index, at
//! or before the landing index, and in every other block keep the member
//! on the side they always had, so they are never crossed.
//!
//! The hazard audit is the in-block relocation's applied across the
//! boundary upstream: a register or condition-state unit the member
//! writes and a crossed instruction reads or writes, or the member reads
//! and a crossed instruction writes, refuses in either direction — which
//! also pins the member's inputs, so the execution at the landing index
//! on this inflow's path computes the values the old position computed.
//! The edge's register transports join the audit directly rather than
//! through an instruction: a `Registers` transport reads its argument and
//! writes its parameter at the boundary, so a member defining the
//! transported argument would hand the binding the pre-move value where
//! the source bound the member's own write, a member defining the
//! parameter would be overwritten before the join observes it, and a
//! member reading the parameter would observe the transported value only
//! after the move. Calls, hosted effects, terminator kinds, and
//! call-roster entries are barriers anywhere in the window — the `Jump`
//! terminator itself is the crossed edge's position, not a window
//! member — and a boundary settlement refuses exactly where an executed
//! prefix changes: past the member's index in its own block, or past the
//! landing index in the predecessor.
//!
//! No instruction, register, roster row, call, settlement, successor, or
//! terminator record changes: the member keeps its own id, kind,
//! operands, provenance, and implicit surface while only its position in
//! the program moves. Proposal and independent replay share only the
//! admission predicates. Validation consumes the proposed program,
//! requires the member to sit at the landing index, and restores the
//! complete source by content — every other block, instruction,
//! register, roster row, call, and settlement is retained bit-identical.

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

pub(crate) use validation::validate_inflow_relocation;

/// An accepted relocation onto the inflow with its replay receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ValidatedInflowRelocation {
    transformed: Arc<SelectedInstructionPlan>,
    receipt: InflowRelocationReceipt,
}

validated_transformed_receipt!(ValidatedInflowRelocation, InflowRelocationReceipt);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InflowRelocationReceipt {
    source_selected: SelectedInstructionPlanIdentity,
    transformed_selected: SelectedInstructionPlanIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
}

receipt_selected_identities!(InflowRelocationReceipt);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InflowRelocationError {
    SourceMismatch,
    /// The named member can never shed its other-inflow executions: a
    /// barrier kind, a call-roster entry, a roster-carrying or unaccounted
    /// memory-capable kind, or a potentially-faulting kind — an access or
    /// trap that ran on every arrival would silently vanish from the
    /// continuations the move removes — or a barrier kind, call, or
    /// unaccounted memory-capable instruction at a position the move
    /// would cross.
    UnsupportedInstruction,
    /// The named pair does not bound an admissible inflow window: the
    /// destination's block does not end in the lone unconditional `Jump`
    /// reaching the member's block, or its edge is not a plain semantic
    /// successor or carries case, fuel, or structural transfers; the
    /// member's block is the entry block, is reached by no edge leaving
    /// another block — the sole-inflow shape is the predecessor family's
    /// case — or is the destination's own block; the destination block is
    /// a non-source implementation block; the destination names no body
    /// or terminator instruction there; a register or condition-state
    /// hazard couples the member with a crossed position or the crossed
    /// edge's transports; or the dead-path audit finds a register or unit
    /// the member writes still live at a reader on an arrival the move
    /// removes.
    UnsupportedPair,
    WorkBudgetExceeded,
    IdentityOverflow,
    ReplayMismatch,
}

impl std::fmt::Display for InflowRelocationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid inflow relocation: {self:?}")
    }
}

impl std::error::Error for InflowRelocationError {}

/// The shared derivation of the member's inflow legality both the
/// admission and the validator prove: the member's join block and index,
/// the inflow predecessor the destination names, the landing position in
/// its body, and the pure-work bar the member meets. The window audit
/// and the step accounting stay with each side.
struct InflowDerivation<'source> {
    plan: &'source SelectedInstructionPlan,
    function: &'source SelectedFunction,
    block_index: usize,
    member_index: usize,
    member_instruction: &'source SelectedInstruction,
    target_index: usize,
    landing_index: usize,
    successor: &'source SelectedSuccessor,
    terminator: &'source SelectedInstruction,
}

/// Re-derive the inflow relocation's legality from the source records:
/// the member's join block, the inflow predecessor's lone `Jump` edge,
/// its plain successor, the join's origin and other-inflow reach, the
/// landing index, and the member's schedulable and speculatable bar.
fn derive_inflow_span<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
) -> Result<InflowDerivation<'source>, InflowRelocationError> {
    let (plan, function) =
        crate::rewrites::plan_function(source, function_index, environment, || {
            InflowRelocationError::SourceMismatch
        })?;
    let (block_index, member_index) =
        member_position(function, member).ok_or(InflowRelocationError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    let member_instruction = &block.instructions[member_index];
    // The destination names the landing position directly: the block
    // holding it must be the inflow — one predecessor ending in the lone
    // `Jump` that reaches the member's block. Naming a terminator-carried
    // instruction lands the member at the body end.
    let (target_index, landing_index) = function
        .blocks
        .iter()
        .enumerate()
        .find_map(|(index, candidate)| {
            instruction_position(&candidate.instructions, destination)
                .map(|position| (index, position))
                .or_else(|| {
                    (terminator_instruction(&candidate.terminator).id == destination)
                        .then_some((index, candidate.instructions.len()))
                })
        })
        .ok_or(InflowRelocationError::UnsupportedPair)?;
    let target = &function.blocks[target_index];
    // Only an unconditional semantic jump edge keeps the member's
    // execution count on this inflow's path: every traversal of the
    // inflow block leaves through it and reaches the member's block
    // exactly once, so the member still runs on every arrival through
    // the crossed edge. A conditional terminator keeps a second exit the
    // member would newly execute on — the arm and join families' burden
    // — and a terminator naming no successor reaches no join.
    let SelectedTerminator::Jump {
        instruction: terminator,
        successor,
    } = &target.terminator
    else {
        return Err(InflowRelocationError::UnsupportedPair);
    };
    if successor.block != block.id {
        return Err(InflowRelocationError::UnsupportedPair);
    }
    // The crossed edge must be a plain semantic successor: case dispatch,
    // continuation, structural transfer, and per-edge fuel all carry
    // boundary effects this step does not cross. Structural bindings may
    // remain only while every transport is `Unused`, which moves nothing.
    if !plain_edge(successor) {
        return Err(InflowRelocationError::UnsupportedPair);
    }
    // A destination in the member's own block is the in-block family's
    // case with a back-edge transport reading, not a cross-edge window.
    // The member's block may not be the entry block — the first
    // traversal crosses no inflow edge at all, so the move would remove
    // an execution the dead-path audit cannot see — and the inflow block
    // must be a plain source block: an implementation block's origin
    // carries edge or case work the bounded audit does not cross.
    if target.id == block.id
        || block.id == function.entry_block
        || !matches!(target.origin, SelectedBlockOrigin::Source(_))
    {
        return Err(InflowRelocationError::UnsupportedPair);
    }
    // At least one edge into the member's block must leave another block:
    // the single-predecessor shape is the predecessor family's case,
    // where no traversal loses the member. Every other inflow — whatever
    // it carries — runs before the member's old position on its own
    // arrivals and is never crossed; the dead-path audit reads its
    // register surface where the move removes the member's write.
    if !all_edges(function)
        .any(|(source_block, edge)| edge.block == block.id && source_block != target.id)
    {
        return Err(InflowRelocationError::UnsupportedPair);
    }
    // The member's execution leaves every arrival through the join's
    // other inflows, so only pure register and condition-state work may
    // rise — a roster-carrying or unaccounted memory access or a
    // potentially-faulting kind that ran on every arrival would silently
    // vanish from the arrivals the move removes.
    if schedulable(function, member_instruction) != Some(false) || !speculatable(member_instruction)
    {
        return Err(InflowRelocationError::UnsupportedInstruction);
    }

    Ok(InflowDerivation {
        plan,
        function,
        block_index,
        member_index,
        member_instruction,
        target_index,
        landing_index,
        successor,
        terminator,
    })
}

/// The dead-path audit the move owes: every location the member writes
/// must be dead — unread until rewritten — on every arrival through the
/// join's other inflows. The walk enters the member's block with nothing
/// live: the other inflow edges run before the member's old position on
/// their own arrivals, so their transports meet no divergence at the
/// boundary — it is the vacated index that publishes the member's
/// locations.
fn member_writes_dead(function: &SelectedFunction, derived: &InflowDerivation<'_>) -> bool {
    dead_path::dead(
        function,
        dead_path::Relocation {
            members: &[derived.member_instruction],
            vacated_block: derived.block_index,
            vacated_first: derived.member_index,
            vacated_last: derived.member_index,
            landing_block: derived.target_index,
            landing_index: derived.landing_index,
            landing: dead_path::Landing::Speculated,
            // The vacated index publishes the member's locations: the
            // move removes its write where the other inflows' arrivals
            // keep the block's old order edges, beside its new position
            // rather than through it, so the vacated index publishes its
            // locations.
            vacated: dead_path::Vacated::Removed,
        },
        dead_path::Start::Block(derived.block_index),
    )
}

/// Keeps the family's typed rejection vocabulary over the shared audit's
/// rejection kinds: an unschedulable member or crossed position is the
/// instruction-level refusal and every window-level refusal is the pair
/// kind.
fn rejection(rejection: RunRelocationRejection) -> InflowRelocationError {
    match rejection {
        RunRelocationRejection::Unschedulable => InflowRelocationError::UnsupportedInstruction,
        RunRelocationRejection::UnreachableDestination
        | RunRelocationRejection::Coupled
        | RunRelocationRejection::MemoryOrdering
        | RunRelocationRejection::NonPlainEdge
        | RunRelocationRejection::Settlement
        | RunRelocationRejection::TransportConflict => InflowRelocationError::UnsupportedPair,
    }
}
