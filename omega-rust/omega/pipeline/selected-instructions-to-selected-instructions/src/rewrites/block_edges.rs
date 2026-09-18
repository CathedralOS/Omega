//! The control-flow surface of one selected block, shared by every rewrite
//! that walks a function: the successor edges a terminator names, the
//! instruction it carries, the whole-function edge roster, and the per-edge
//! checks and surfaces the relocation admissions apply to an edge a member
//! crosses. Each rewrite locates its own blocks and edges; reading them is
//! one owner.
use selected_instructions::{
    SelectedBlock, SelectedBlockId, SelectedFunction, SelectedInstruction,
    SelectedMemoryAccessOrigin, SelectedStructuralTransport, SelectedSuccessor,
    SelectedSuccessorRole, SelectedTerminator, SelectedValueTransport,
};

use crate::rewrites::window_hazards::{has_memory_rows, register_reads, register_writes};

/// The instruction a terminator carries — a position every traversal of
/// its block observes, and the body-end landing position a destination can
/// name.
pub(super) fn terminator_instruction(terminator: &SelectedTerminator) -> &SelectedInstruction {
    match terminator {
        SelectedTerminator::HostedExitProcess { instruction, .. }
        | SelectedTerminator::Jump { instruction, .. }
        | SelectedTerminator::ConditionalBranch { instruction, .. }
        | SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. }
        | SelectedTerminator::ConditionalBranchI64LessThan { instruction, .. }
        | SelectedTerminator::Return { instruction, .. } => instruction,
    }
}

/// Every successor edge a terminator names, in its physical record order;
/// `HostedExitProcess` and `Return` name none.
pub(super) fn terminator_successors(terminator: &SelectedTerminator) -> Vec<&SelectedSuccessor> {
    match terminator {
        SelectedTerminator::Jump { successor, .. } => vec![successor],
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
        SelectedTerminator::HostedExitProcess { .. } | SelectedTerminator::Return { .. } => {
            Vec::new()
        }
    }
}

/// Every position of a block: its body instructions, then the instruction
/// its terminator carries.
pub(super) fn block_instructions(
    block: &SelectedBlock,
) -> impl Iterator<Item = &SelectedInstruction> {
    block
        .instructions
        .iter()
        .chain(std::iter::once(terminator_instruction(&block.terminator)))
}

/// Every successor edge in the function paired with the block it leaves,
/// for predecessor-count audits.
pub(super) fn all_edges(
    function: &SelectedFunction,
) -> impl Iterator<Item = (SelectedBlockId, &SelectedSuccessor)> {
    function.blocks.iter().flat_map(|block| {
        terminator_successors(&block.terminator)
            .into_iter()
            .map(move |edge| (block.id, edge))
    })
}

/// A plain semantic successor edge: case dispatch, continuation,
/// structural transfer, and per-edge fuel all carry boundary effects a
/// member would physically cross — a relocation does not cross them. This
/// applies to the edges the member lands through; edges it never traverses
/// join only the dead-path audit, which reads their complete register
/// surface directly.
pub(super) fn plain_edge(successor: &SelectedSuccessor) -> bool {
    successor.role == SelectedSuccessorRole::Semantic
        && successor.structural_case.is_none()
        && successor.fuel.is_empty()
        && successor
            .structural_bindings
            .iter()
            .all(|binding| binding.transport == SelectedStructuralTransport::Unused)
}

/// The member must not interfere with one crossed edge's register
/// transports: a member defining the transported argument would hand the
/// binding a stale value, a member defining the parameter would be
/// overwritten by it, and a member reading the parameter would observe the
/// transported value only after the move. Reading the argument is harmless
/// — the binding never writes it.
pub(super) fn transport_conflict(
    member: &SelectedInstruction,
    successor: &SelectedSuccessor,
) -> bool {
    successor.bindings.iter().any(|binding| {
        matches!(
            binding.transport,
            SelectedValueTransport::Registers { argument, parameter }
                if register_writes(member)
                    .any(|register| register == argument || register == parameter)
                    || register_reads(member).any(|register| register == parameter))
    })
}

/// The roster surface one crossed edge position carries: rows the
/// terminator instruction itself records plus any rows the roster logs
/// with the edge's own origin.
pub(super) fn edge_accounted(
    function: &SelectedFunction,
    terminator: &SelectedInstruction,
    successor: &SelectedSuccessor,
) -> bool {
    has_memory_rows(function, terminator.id)
        || function
            .memory_accesses
            .iter()
            .any(|access| access.origin == SelectedMemoryAccessOrigin::Edge(successor.psi_edge))
}

/// The register-plus-unit transport surface one edge carries — the
/// dead-path audit's per-edge scan cost.
pub(super) fn edge_surface(successor: &SelectedSuccessor) -> usize {
    successor.bindings.len()
        + successor.structural_bindings.len()
        + successor
            .structural_case
            .as_ref()
            .map_or(0, |case| case.payloads.len())
}
