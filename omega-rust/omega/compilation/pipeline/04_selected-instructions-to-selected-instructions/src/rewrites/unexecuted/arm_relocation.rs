//! Instruction relocation out of a branch arm, upstream through the fork
//! edge that reaches it, on the selected CFG.
//!
//! The landed cross-boundary families move a member downstream across a
//! `Jump` edge (`rewrites/edge_relocation`), upstream across the sole
//! `Jump` edge reaching its block (`rewrites/predecessor_relocation`),
//! through a complete diamond (`rewrites/diamond_relocation` sinks,
//! `rewrites/join_relocation` rises), and into the one arm a fork edge
//! selects (`rewrites/fork_relocation`). This family is the upstream
//! counterpart of the fork move: the named `member` leaves a conditional
//! arm's body, crosses the head's plain edge into it, and takes the named
//! `destination` instruction's position inside the fork head's body. The
//! destination and every later position keep their relative order one
//! slot later, so the member lands exactly where the destination sat.
//!
//! Rising out of the arm flips the soundness burden the sink carries.
//! `fork_relocation` removes the member's execution from every traversal
//! leaving through the branch's other edges, so its written locations
//! must be dead there. This move *adds* the member's execution to those
//! traversals — speculation — so the member must be pure register and
//! condition-state work that can never fault: no barrier kind, no
//! call-roster entry, no memory roster rows, and no memory-capable or
//! potentially-faulting kind, because an access or trap that ran only on
//! the arm's path would newly run on every traversal of the head. And
//! every register and condition-state unit the member writes must be
//! dead — unread until rewritten — along every path leaving the head's
//! non-source edges: the forward dead-path audit walks the successor
//! region from each such edge, reading each crossed edge's register,
//! structural-binding, and case-payload transports as boundary positions,
//! and refuses the moment a still-live member definition meets a reader.
//! On those paths the member's own new position is transparent: its reads
//! belong to the added execution whose outputs must die anyway, and its
//! writes republish member locations that stay subject to the die-unread
//! rule — a re-executed member publishes values foreign to the path, not
//! the values a reader there observed before.
//!
//! The source side needs no region structure at all: the arm is any
//! source block whose every predecessor edge leaves the member's new
//! block, so each traversal reaching the arm is immediately preceded by
//! a head traversal that ran the member — the same count of one it had
//! inside the arm. A second predecessor into the arm would hand the arm's
//! stream a member that ran an extra time on that path, and a head
//! terminator that is not a two-successor conditional branch is the
//! single-edge family's case or no relocation at all. The crossed edges
//! must be plain semantic successors: case custody, fuel, and structural
//! transfers are boundary effects the move does not cross.
//!
//! Between the member's old position and its new one lie only the crossed
//! positions the window audits: the head's body instructions at and after
//! the landing index, the branch terminator instruction, each crossed
//! edge's register transports, and the arm's body instructions before the
//! member's index. Positions before the landing index, at or after the
//! member's index, and in every other block keep the member on the side
//! they always had, so they are never crossed. The non-source edges are
//! never traversed by the member's new position — a traversal takes them
//! after the member ran — so their transports join only the dead-path
//! audit, where an argument reading a still-live member definition
//! refuses and a parameter writing one retires it.
//!
//! The hazard audit is the in-block relocation's applied across the
//! boundary in the upstream direction: a register or condition-state unit
//! the member writes and a crossed instruction reads or writes, or the
//! member reads and a crossed instruction writes, refuses in either
//! direction — which also pins the member's inputs, so the execution at
//! the landing index computes the values the arm position computed. The
//! edge's register transports join the audit directly: a member defining
//! the transported argument would hand the binding a new value where the
//! source bound the old, a member defining the parameter would be
//! overwritten before the arm observes it, and a member reading the
//! parameter would observe the pre-transport value after the move. Calls,
//! hosted effects, terminator kinds, and call-roster entries are barriers
//! anywhere in the window — the branch terminator itself is the crossed
//! edges' position, not a window member — and a boundary settlement
//! refuses exactly where an executed prefix changes: past the member's
//! index in the arm, or past the landing index in the head.
//!
//! No instruction, register, roster row, call, settlement, successor, or
//! terminator record changes: the member keeps its own id, kind, operands,
//! provenance, and implicit surface while only its position in the program
//! moves. Proposal and independent replay share only the admission
//! predicates. Validation consumes the proposed program, requires the
//! member to sit at the landing index, and restores the complete source
//! by content — every other block, instruction, register, roster row,
//! call, and settlement is retained bit-identical.

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
use crate::rewrites::block_edges::{
    CrossingDirection, RelocationCrossing, all_edges, crossed_window, member_position,
    predecessor_source_index,
};
use crate::rewrites::unexecuted::dead_path;
use crate::rewrites::window_hazards::{
    RunRelocationRejection, admit_run_relocation, schedulable, speculatable,
};

pub(crate) use validation::validate_arm_relocation;

/// An accepted relocation out of the branch arm with its replay receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ValidatedArmRelocation {
    transformed: Arc<SelectedInstructionPlan>,
    receipt: ArmRelocationReceipt,
}

validated_transformed_receipt!(ValidatedArmRelocation, ArmRelocationReceipt);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ArmRelocationReceipt {
    source_selected: SelectedInstructionPlanIdentity,
    transformed_selected: SelectedInstructionPlanIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
}

receipt_selected_identities!(ArmRelocationReceipt);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ArmRelocationError {
    SourceMismatch,
    /// The named member can never speculate: a barrier kind, a call-roster
    /// entry, a roster-carrying or unaccounted memory-capable kind — an
    /// access or fault that ran only on the arm's path would newly run on
    /// every head traversal — or a barrier kind, call, or unaccounted
    /// memory-capable instruction at a position the move would cross.
    UnsupportedInstruction,
    /// The named pair does not bound an admissible arm window: the
    /// member's block is the entry block, a non-source implementation
    /// block, or reached by edges leaving more than one block — or by
    /// none; the head is the member's own block or a non-source block,
    /// or lacks the two-successor conditional terminator; the destination
    /// names no position in the head; a crossed edge is not a plain
    /// semantic successor or carries case, fuel, or structural transfers;
    /// a register or condition-state hazard couples the member with a
    /// crossed position or a crossed edge's transports; a boundary
    /// settlement observes a changed executed prefix; or the dead-path
    /// audit finds a register or unit the member writes still live at a
    /// reader on a path the member never executed on before.
    UnsupportedPair,
    WorkBudgetExceeded,
    IdentityOverflow,
    ReplayMismatch,
}

impl std::fmt::Display for ArmRelocationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid arm relocation: {self:?}")
    }
}

impl std::error::Error for ArmRelocationError {}

/// The shared derivation of the member relocation both the admission and
/// the validator prove: the member's arm block and index, the two-branch
/// fork head it rises to and the landing index in its body, the plain
/// edges the member physically crosses and the head's remaining edges it
/// newly executes on, and the shared window audit over them. Step
/// accounting stays with each side.
struct ArmRelocationDerivation<'source> {
    plan: &'source SelectedInstructionPlan,
    function: &'source SelectedFunction,
    block_index: usize,
    member_index: usize,
    member_instruction: &'source SelectedInstruction,
    target_index: usize,
    landing_index: usize,
    edge_limit: usize,
    crossing: RelocationCrossing<'source>,
}

/// Re-derive the arm relocation's legality from the source records: the
/// member's arm, the fork head dominating it, the destination's landing
/// position, the crossed window's shared audit, and the dead-path audit
/// over the head's skipped edges.
#[allow(clippy::too_many_arguments)]
fn derive_arm_relocation<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
) -> Result<ArmRelocationDerivation<'source>, ArmRelocationError> {
    let (plan, function) =
        crate::rewrites::plan_function(source, function_index, environment, || {
            ArmRelocationError::SourceMismatch
        })?;
    let (block_index, member_index) =
        member_position(function, member).ok_or(ArmRelocationError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    let member_instruction = &block.instructions[member_index];
    // The member's block must be a conditional arm of one fork head: a
    // plain source block, never the entry block — an edge into the entry
    // would feed it a traversal entry never saw — and reached only by
    // edges leaving that head. A predecessor leaving any other block
    // would hand the arm's stream a member that ran an extra time on that
    // path, and an arm with no predecessors is unreachable: hoisting the
    // member would start its execution.
    if block.id == function.entry_block || !matches!(block.origin, SelectedBlockOrigin::Source(_)) {
        return Err(ArmRelocationError::UnsupportedPair);
    }
    let target_index =
        predecessor_source_index(function, block.id).ok_or(ArmRelocationError::UnsupportedPair)?;
    let target = &function.blocks[target_index];
    // The head must not collapse back into the member's own block — an
    // arm that heads its own fork is the in-block family's case with a
    // back-edge reading — and an implementation block's origin carries
    // edge or case work the bounded audit does not cross.
    if target.id == block.id || !matches!(target.origin, SelectedBlockOrigin::Source(_)) {
        return Err(ArmRelocationError::UnsupportedPair);
    }
    // Only a two-successor conditional terminator gives the landing block
    // the fork this step rises through: an unconditional `Jump` is the
    // sole-predecessor family's case, and a terminator naming no
    // successors could never have reached the arm.
    let branch_edges: Vec<&SelectedSuccessor> = match &target.terminator {
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
            return Err(ArmRelocationError::UnsupportedPair);
        }
    };
    let terminator = terminator_instruction(&target.terminator);
    // The destination names the landing position directly: a body
    // instruction's own index, or the head's terminator-carried
    // instruction landing the member at the body end.
    let landing_index = instruction_position(&target.instructions, destination)
        .or_else(|| (terminator.id == destination).then_some(target.instructions.len()))
        .ok_or(ArmRelocationError::UnsupportedPair)?;
    // The member physically crosses every head edge into the arm: each
    // must be a plain semantic successor, and its register transports sit
    // between the member's old and new positions. The head's remaining
    // edges are the speculative side the member's new position adds it
    // to — they are never crossed positions.
    let mut skipped_edges: Vec<&SelectedSuccessor> = Vec::new();
    for edge in &branch_edges {
        if edge.block != block.id {
            skipped_edges.push(*edge);
        }
    }
    // The member's execution becomes unconditional: it newly runs on every
    // traversal leaving through the head's other edges, so only pure
    // register and condition-state work may rise — a roster-carrying or
    // unaccounted memory access or a potentially-faulting kind that ran
    // only on the arm's path would run on every head traversal after the
    // move.
    if schedulable(function, member_instruction) != Some(false) || !speculatable(member_instruction)
    {
        return Err(ArmRelocationError::UnsupportedInstruction);
    }
    // The crossed window is the shared derivation rather than this
    // family's own enumeration: the member is the one-member run, and the
    // gates above leave only head-to-arm acyclic paths; the walk may also
    // explore the skipped edges' subtrees before they dead-end, so the
    // function's edge roster bounds it. The shared audit applies the
    // hazard, transport, memory-roster, and settlement checks once: the
    // branch terminator rides each crossed edge's record — exempt from
    // the barrier-kind rule but inside the call and hazard audit — and
    // the head's skipped edges are never crossed positions. A boundary
    // settlement positioned past the member's index in the arm or past
    // the landing index in the head observed a changed executed prefix
    // and refuses.
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
    .ok_or(ArmRelocationError::WorkBudgetExceeded)?;
    admit_run_relocation(function, &[member_instruction], &crossing).map_err(rejection)?;
    // The dead-path audit: every location the member writes must be dead —
    // unread until rewritten — along every path leaving the head's other
    // edges, where the member's execution is new.
    if !skipped_edges.is_empty()
        && !dead_path::dead(
            function,
            dead_path::Relocation {
                members: &[member_instruction],
                vacated_block: block_index,
                vacated_first: member_index,
                vacated_last: member_index,
                landing_block: target_index,
                landing_index,
                landing: dead_path::Landing::Speculated,
                // The vacated index stays silent: every walked path
                // reaching the arm crossed the head's new position first,
                // so the member's write is never missing there.
                vacated: dead_path::Vacated::Silent,
            },
            dead_path::Start::Edges(&skipped_edges),
        )
    {
        return Err(ArmRelocationError::UnsupportedPair);
    }

    Ok(ArmRelocationDerivation {
        plan,
        function,
        block_index,
        member_index,
        member_instruction,
        target_index,
        landing_index,
        edge_limit,
        crossing,
    })
}

/// Keeps the family's typed rejection vocabulary over the shared audit's
/// rejection kinds: an unschedulable member or crossed position is the
/// instruction-level refusal and every window-level refusal is the pair
/// kind.
fn rejection(rejection: RunRelocationRejection) -> ArmRelocationError {
    match rejection {
        RunRelocationRejection::Unschedulable => ArmRelocationError::UnsupportedInstruction,
        RunRelocationRejection::UnreachableDestination
        | RunRelocationRejection::Coupled
        | RunRelocationRejection::MemoryOrdering
        | RunRelocationRejection::NonPlainEdge
        | RunRelocationRejection::Settlement
        | RunRelocationRejection::TransportConflict => ArmRelocationError::UnsupportedPair,
    }
}
