//! Member-run relocation through one shared admission on the selected CFG.
//!
//! The per-shape scheduling families — `edge`, `predecessor`, `diamond`,
//! `join`, `fork`, `arm`, `bypass`, `triangle`, `confluence`, `inflow` and
//! their run variants — each enumerate the window their own shape crosses
//! and spell the independence audit by hand. This module is the general
//! mechanism the execution board's scheduling flag names: one admission
//! over a contiguous member run and a destination point. Given
//! `first_member`..`last_member` in one block's body and a `destination`
//! instruction anywhere in the function, admission derives the window the
//! move crosses itself — every acyclic edge path between the run's block
//! and the destination block contributes its terminator-carried
//! instruction and successor row, every intermediate block contributes
//! its whole body, the run's own block contributes the tail behind the
//! run, and the destination block contributes the positions before the
//! landing index — then proves the move changes no traversal: every
//! predecessor edge into the destination lies on a crossed path (no
//! traversal gains the run) and every exit of every block any path
//! leaves reaches the destination (no traversal loses the run). The
//! shared audit then runs once: every member schedulable, no member
//! coupled with a crossed position or a crossed edge's terminator
//! instruction, every crossed edge a plain semantic successor free of
//! register-transport interference, roster-carrying runs crossing only
//! unaccounted positions, and no boundary settlement whose observed
//! executed prefix changes — including settlements inside fully crossed
//! intermediate bodies.
//!
//! A run named inside the same block it lands in is the in-block move the
//! local families own: the window is the span between the run and the
//! landing index and the traversal proof is vacuous. Moves whose gained
//! or lost traversal set is nonempty but still sound — the fork-head
//! hoists and join sinks — are the `dead_path` audit's families and stay
//! with them; this admission proves the preservation case.
//!
//! No instruction, register, roster row, call, settlement, successor, or
//! terminator record changes: every member keeps its own id, kind,
//! operands, provenance, and implicit surface while only the run's
//! position in the program moves. Proposal and independent replay share
//! only the admission predicates. Validation consumes the proposed program,
//! requires the run to sit at the landing index in its original order,
//! and restores the complete source by content — every other block,
//! instruction, register, roster row, call, and settlement is retained
//! bit-identical.

mod admission;
mod rewrite;
mod validation;

use std::collections::BTreeSet;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{
    SelectedBlockOrigin, SelectedFunction, SelectedInstructionId, instruction_position,
    terminator_instruction,
};

use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::{
    CrossingDirection, PATH_EDGE_LIMIT, all_edges, crossed_window, edge_surface, member_position,
    terminator_successors,
};
use crate::rewrites::window_hazards::{RunRelocationRejection, admit_run_relocation, surface};

use std::sync::Arc;

use optimization_core::OptimizationUnitIdentity;
use selected_instructions::{SelectedInstructionPlan, SelectedInstructionPlanIdentity};
use semantic_vocabulary::FuelScheduleIdentity;

pub(crate) use validation::validate_member_run_relocation;

/// An accepted member-run relocation with its replay receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ValidatedMemberRunRelocation {
    transformed: Arc<SelectedInstructionPlan>,
    receipt: MemberRunRelocationReceipt,
}

validated_transformed_receipt!(ValidatedMemberRunRelocation, MemberRunRelocationReceipt);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MemberRunRelocationReceipt {
    source_selected: SelectedInstructionPlanIdentity,
    transformed_selected: SelectedInstructionPlanIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
}

receipt_selected_identities!(MemberRunRelocationReceipt);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MemberRunRelocationError {
    SourceMismatch,
    /// A member or a crossed position can never trade order: a barrier
    /// kind, a call-roster entry, or an unaccounted memory reach.
    UnsupportedInstruction,
    /// The named span or destination does not bound an admissible
    /// relocation: the last member does not close a contiguous run after
    /// the first in one block, the destination names no position, the
    /// move crosses no acyclic path to the destination, the destination
    /// is the entry block or not a plain source block, a traversal would
    /// gain or lose the run (a destination predecessor outside the
    /// crossed paths, or an exit of a crossed block that never reaches
    /// the destination), the run would land inside its own span, a
    /// crossed edge carries boundary effects, a register or
    /// condition-state hazard couples a member with a crossed position,
    /// a member interferes with a crossed edge's register transports, a
    /// second memory-access actor shares the window, or a boundary
    /// settlement observes a changed executed prefix.
    UnsupportedPair,
    WorkBudgetExceeded,
    IdentityOverflow,
    ReplayMismatch,
}

impl std::fmt::Display for MemberRunRelocationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid member-run relocation: {self:?}")
    }
}

impl std::error::Error for MemberRunRelocationError {}

/// The validator's own reconstruction of the relocation the contract
/// permits: the run's block and bounding positions, the destination
/// block, and the landing index the move takes there. It shares no state
/// with the producer's `admission` record.
struct Reconstructed<'source> {
    function: &'source SelectedFunction,
    /// The run's own block, as an index into `function.blocks`.
    run_block: usize,
    /// First body index the run occupies.
    run_start: usize,
    /// Last body index the run occupies.
    run_end: usize,
    /// Index into `function.blocks` of the block the run lands in —
    /// `run_block` itself for the in-block case.
    destination_block: usize,
    /// The body index the run lands at, in the destination block's
    /// source-order positions.
    landing_index: usize,
}

/// Map the shared audit's rejection onto this module's public error: an
/// unschedulable member or crossed position is `UnsupportedInstruction`;
/// every shape-level refusal — unreachable destination, non-plain edge,
/// transport conflict, coupling, memory ordering, settlement — is
/// `UnsupportedPair`.
fn reject(rejection: RunRelocationRejection) -> MemberRunRelocationError {
    match rejection {
        RunRelocationRejection::Unschedulable => MemberRunRelocationError::UnsupportedInstruction,
        RunRelocationRejection::UnreachableDestination
        | RunRelocationRejection::Coupled
        | RunRelocationRejection::MemoryOrdering
        | RunRelocationRejection::TransportConflict
        | RunRelocationRejection::NonPlainEdge
        | RunRelocationRejection::Settlement => MemberRunRelocationError::UnsupportedPair,
    }
}

/// Reconstruct the legality of moving the run `first_member`..`last_member`
/// onto `destination` from the source records: locate the run as the
/// contiguous span the two members bound in one block in the named order,
/// resolve the destination to a landing position in any block's body or
/// terminator-carried instruction, derive the window the move crosses
/// over every acyclic path between the two blocks, re-prove the traversal
/// preservation — every predecessor edge into the destination lies on a
/// crossed path and every exit of every block a crossed path leaves
/// reaches the destination — then apply the shared schedulable, hazard,
/// memory-roster, transport, and settlement audit once and account the
/// family's measured steps against the budget. Nothing in this audit
/// reads the producer's admission decision, so a producer-side legality
/// error fails here even when the proposal matches the emitted edit.
#[allow(clippy::too_many_arguments)]
fn reconstruct<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    first_member: SelectedInstructionId,
    last_member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Reconstructed<'source>, MemberRunRelocationError> {
    let (plan, function) =
        crate::rewrites::plan_function(source, function_index, environment, || {
            MemberRunRelocationError::SourceMismatch
        })?;
    let (run_block, run_start) =
        member_position(function, first_member).ok_or(MemberRunRelocationError::SourceMismatch)?;
    // The run is the contiguous span the two named members bound in this
    // block, in the named order; one member is the member-level move. A
    // last member absent from the same body, or earlier than the first,
    // names no run.
    let block = &function.blocks[run_block];
    let run_end = block
        .instruction_index(last_member)
        .filter(|position| *position >= run_start)
        .ok_or(MemberRunRelocationError::UnsupportedPair)?;
    // The destination names a body position in any block — the run lands
    // at its index — or a terminator-carried instruction, landing the run
    // at that block's body end.
    let (destination_block, landing_index) = function
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block_index, candidate)| {
            instruction_position(&candidate.instructions, destination)
                .map(|index| (block_index, index))
                .or_else(|| {
                    (terminator_instruction(&candidate.terminator).id == destination)
                        .then_some((block_index, candidate.instructions.len()))
                })
        })
        .ok_or(MemberRunRelocationError::UnsupportedPair)?;
    let in_block = run_block == destination_block;
    if in_block && landing_index >= run_start && landing_index <= run_end {
        // Landing inside the run's own span is a degenerate move.
        return Err(MemberRunRelocationError::UnsupportedPair);
    }
    // The move's direction is whichever one has acyclic paths: downstream
    // when the destination is reachable from the run's block, upstream when
    // the run's block is reachable from the destination. Blocks on a common
    // cycle reach each other both ways, where a move in either direction
    // changes how often the run executes, so that case refuses rather than
    // picking a direction.
    let window = |direction| {
        crossed_window(
            function,
            run_block,
            run_start,
            run_end,
            destination_block,
            landing_index,
            direction,
            PATH_EDGE_LIMIT,
        )
        .ok_or(MemberRunRelocationError::WorkBudgetExceeded)
    };
    let mut crossing = window(CrossingDirection::Forward)?;
    if !in_block {
        let upstream = window(CrossingDirection::Backward)?;
        // The block every traversal of the run must now arrive through: the
        // destination downstream, the run's own block upstream. Its
        // predecessor edges carry the direction's whole traversal claim.
        let arrival_block = match (crossing.reachable, upstream.reachable) {
            (true, true) => return Err(MemberRunRelocationError::UnsupportedPair),
            (false, true) => {
                crossing = upstream;
                run_block
            }
            _ => destination_block,
        };
        let destination = &function.blocks[destination_block];
        let arrival = &function.blocks[arrival_block];
        // An implementation block's origin carries edge or case work the
        // audit does not cross, and the run lands in the destination in
        // either direction.
        if !matches!(destination.origin, SelectedBlockOrigin::Source(_)) {
            return Err(MemberRunRelocationError::UnsupportedPair);
        }
        if !crossing.reachable {
            return Err(MemberRunRelocationError::UnsupportedPair);
        }
        // Traversals gained downstream, lost upstream: an edge into the
        // arrival block that no crossed path covers reaches the run's new
        // position without passing the run's old one. An arrival block
        // nothing arrives at — the entry block of an acyclic function, a
        // detached block — makes that audit vacuous, so it must have at
        // least one predecessor for the claim to mean anything.
        let crossed_edges: BTreeSet<_> = crossing
            .edges
            .iter()
            .map(|edge| (edge.block, edge.successor.psi_edge))
            .collect();
        let mut arrivals = 0usize;
        for (from, predecessor) in all_edges(function) {
            if predecessor.block == arrival.id {
                arrivals += 1;
                if !crossed_edges.contains(&(from, predecessor.psi_edge)) {
                    return Err(MemberRunRelocationError::UnsupportedPair);
                }
            }
        }
        if arrivals == 0 {
            return Err(MemberRunRelocationError::UnsupportedPair);
        }
        // The mirrored half, in both directions: every exit of every block a
        // path leaves must itself lie on a complete crossed path — otherwise
        // some traversal leaves the crossed region without reaching the run's
        // new position.
        for source_block in crossing
            .edges
            .iter()
            .map(|edge| edge.block)
            .collect::<BTreeSet<_>>()
        {
            let source = function
                .block(source_block)
                .ok_or(MemberRunRelocationError::SourceMismatch)?;
            if terminator_successors(&source.terminator)
                .iter()
                .any(|exit| !crossed_edges.contains(&(source_block, exit.psi_edge)))
            {
                return Err(MemberRunRelocationError::UnsupportedPair);
            }
        }
    }
    let members: Vec<_> = block.instructions[run_start..=run_end].iter().collect();
    admit_run_relocation(function, &members, &crossing).map_err(reject)?;
    // The validator's own audit walks the same surfaces the family
    // publishes: every block body and terminator instruction once to
    // locate the run and the destination; the path walk bounded by
    // PATH_EDGE_LIMIT; the gained/lost audit reading the predecessor and
    // per-block successor rosters once; the window audit walking every
    // member-against-crossed-position and member-against-crossed-edge
    // surface, plus the function's three rosters once.
    let crossed_positions = crossing
        .positions
        .iter()
        .flat_map(|(_, positions)| positions.iter());
    let steps = crate::rewrites::rewrite_scan(plan, function)
        .and_then(|total| total.checked_add(PATH_EDGE_LIMIT.saturating_mul(2)))
        .and_then(|total| {
            function.blocks.iter().try_fold(total, |total, block| {
                total.checked_add(terminator_successors(&block.terminator).len())
            })
        })
        .and_then(|total| {
            members.iter().try_fold(total, |total, member| {
                crossed_positions
                    .clone()
                    .try_fold(total, |total, _| total.checked_add(surface(member)))
            })
        })
        .and_then(|total| {
            crossing
                .positions
                .iter()
                .try_fold(total, |total, (block_index, positions)| {
                    positions.iter().try_fold(total, |total, position| {
                        total.checked_add(surface(
                            &function.blocks[*block_index].instructions[*position],
                        ))
                    })
                })
        })
        .and_then(|total| {
            crossing.edges.iter().try_fold(total, |total, edge| {
                total
                    .checked_add(surface(edge.instruction).saturating_mul(members.len()))?
                    .checked_add(edge_surface(edge.successor).saturating_mul(members.len()))
            })
        })
        .and_then(|total| {
            total
                .checked_add(function.memory_accesses.len())?
                .checked_add(function.calls.len())?
                .checked_add(function.boundary_settlements.len())
        })
        .ok_or(MemberRunRelocationError::IdentityOverflow)?;
    crate::rewrites::charge_steps(
        steps,
        budget,
        || MemberRunRelocationError::IdentityOverflow,
        || MemberRunRelocationError::WorkBudgetExceeded,
    )?;
    Ok(Reconstructed {
        function,
        run_block,
        run_start,
        run_end,
        destination_block,
        landing_index,
    })
}
