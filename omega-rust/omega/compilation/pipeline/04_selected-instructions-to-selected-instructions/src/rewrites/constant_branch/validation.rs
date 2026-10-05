//! Independent validation of constant condition branch folding.
//!
//! The validator never calls [`super::admission`]: it re-derives the
//! fold's legality from the source records — the terminator-carried
//! instruction's branch/kind pairing and zero-operand flag-reader shape,
//! the flag partition every implicit use must satisfy (a flag unit
//! resolving through the shared condition-state walk to the one compare
//! whose published definitions cover it, any other unit carried on the
//! jump row's own implicit surface), the jump row republishing exactly
//! the branch's implicit definitions and clobbers, and the compare's
//! compile-time constant operands — then decides the successor on its own
//! predicate table, rebuilds the `Jump` terminator the contract demands,
//! and requires the proposal to carry it in the branch's block. The
//! catalog control surface is restated here too: the branch row must
//! encode a conditional relative branch and the jump row an unconditional
//! one on every alternative, both otherwise effect-isolated — read off the
//! bound catalog directly, never through the `pair` descriptor table the
//! producer consults. Reinserting the source terminator must restore the complete source by
//! content: every other instruction, register, call, settlement, and
//! function included. A producer admission error therefore fails
//! validation even when the proposal is exactly what that producer
//! emitted.
use std::collections::BTreeSet;
use std::sync::Arc;

use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::RegisterUnitId;
use selected_instructions::{SelectedFunction, SelectedInstructionId, SelectedInstructionPlan};
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{ConstantBranchError, ConstantBranchReceipt, ValidatedConstantBranch};
use crate::ValidatedSelectedAnalysis;

/// The flag-universe and jump-surface sizes the measured-step contract
/// charges — the union of the target's three compare rows' implicit
/// definitions and the jump row's implicit surface, under the same
/// deduplication the audit's own sets apply. Discovery charges these once
/// per pass, and the reached compare's published definitions stay bounded
/// by the flag universe they are drawn from.
pub(crate) fn surface_sizes(
    environment: &ValidatedTargetRegisterEnvironment,
) -> Result<(usize, usize), ConstantBranchError> {
    let keys = environment.selected_keys();
    let mut flag_universe = BTreeSet::new();
    for key in [
        keys.compare_i64,
        keys.compare_i64_immediate,
        keys.compare_i64_zero,
    ] {
        flag_universe.extend(
            environment
                .constraint(key)
                .ok_or(ConstantBranchError::ConstraintMismatch)?
                .implicit_defs
                .iter()
                .copied(),
        );
    }
    let jump_row = environment
        .constraint(keys.jump)
        .ok_or(ConstantBranchError::ConstraintMismatch)?;
    let jump_surface: BTreeSet<RegisterUnitId> = jump_row
        .implicit_uses
        .iter()
        .chain(jump_row.implicit_defs.iter())
        .chain(jump_row.clobbers.iter())
        .copied()
        .collect();
    Ok((flag_universe.len(), jump_surface.len()))
}

/// The validation work this audit performs, in the measured-step contract
/// the family publishes: one step per block plus one per instruction
/// across the plan, two scans of the reconstructed function's blocks for
/// the producer audit, and the shared walk's setup and fixpoint bound at
/// one per used unit — flag or not — so the bound stays independent of
/// the partition split. The `flag_universe`, `jump_surface`, and
/// `compare_defs` surface sizes are caller-supplied so discovery can
/// charge the contract before admission runs; a pre-admission caller
/// bounds `compare_defs` by `flag_universe`, which covers the published
/// definitions of every compare row the union was built from.
pub(crate) fn measured_steps(
    plan: &SelectedInstructionPlan,
    function: &SelectedFunction,
    successors: &[Vec<usize>],
    implicit_uses: usize,
    flag_universe: usize,
    jump_surface: usize,
    compare_defs: usize,
) -> Result<u64, ConstantBranchError> {
    let function_scan = function
        .blocks
        .iter()
        .try_fold(0usize, |total, block| {
            total.checked_add(block.instructions.len())?.checked_add(1)
        })
        .ok_or(ConstantBranchError::IdentityOverflow)?;
    let edge_count = successors
        .iter()
        .try_fold(0usize, |total, targets| total.checked_add(targets.len()))
        .ok_or(ConstantBranchError::IdentityOverflow)?;
    let block_count = function.blocks.len();
    // Producer scans walk the whole function once per compared register —
    // two at most. The shared walk's setup resolves every edge's target
    // index, fills the predecessor lists, and marks the backward cone.
    // Each used unit then pays its partition checks — flag-universe and
    // jump-surface membership and the compare-definitions lookup — plus,
    // for a flag unit, the block-prefix scan and — on an in-block miss —
    // every block's stream for its last event, after which entry-set
    // propagation requeues a block only while its set grows: a set holds
    // at most one element per event site plus the unknown marker, so pops
    // stay under `blocks × (elements + 1)` and each pop visits its
    // out-edges — no more than the widest terminator's — at a bounded
    // union cost.
    let elements = function_scan
        .checked_add(1)
        .ok_or(ConstantBranchError::IdentityOverflow)?;
    let pops = block_count
        .checked_mul(
            elements
                .checked_add(1)
                .ok_or(ConstantBranchError::IdentityOverflow)?,
        )
        .ok_or(ConstantBranchError::IdentityOverflow)?;
    let widest_out = successors
        .iter()
        .map(|targets| targets.len())
        .max()
        .unwrap_or(0);
    let per_unit = function_scan
        .checked_add(block_count)
        .and_then(|total| total.checked_add(pops))
        .and_then(|total| total.checked_add(pops.checked_mul(widest_out)?.checked_mul(elements)?))
        .and_then(|total| {
            total.checked_add(
                flag_universe
                    .checked_add(jump_surface)?
                    .checked_add(compare_defs)?,
            )
        })
        .ok_or(ConstantBranchError::IdentityOverflow)?;
    let walk_setup = edge_count
        .checked_mul(
            block_count
                .checked_add(1)
                .ok_or(ConstantBranchError::IdentityOverflow)?,
        )
        .and_then(|total| total.checked_add(block_count))
        .and_then(|total| total.checked_add(edge_count))
        .and_then(|total| total.checked_add(flag_universe))
        .and_then(|total| total.checked_add(jump_surface))
        .ok_or(ConstantBranchError::IdentityOverflow)?;
    let reach_scan = implicit_uses
        .checked_mul(per_unit)
        .and_then(|total| total.checked_add(walk_setup))
        .ok_or(ConstantBranchError::IdentityOverflow)?;
    let steps = plan
        .functions
        .iter()
        .try_fold(0usize, |total, function| {
            function.blocks.iter().try_fold(total, |total, block| {
                total.checked_add(block.instructions.len())?.checked_add(1)
            })
        })
        .and_then(|total| {
            total
                .checked_add(function_scan.checked_mul(2)?)?
                .checked_add(reach_scan)
        })
        .ok_or(ConstantBranchError::IdentityOverflow)?;
    u64::try_from(steps).map_err(|_| ConstantBranchError::IdentityOverflow)
}

/// Admit a freshly produced plan with its receipt; the producer calls this
/// instead of replaying its own output.
pub(crate) fn seal_constant_branch_fold(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    branch: SelectedInstructionId,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedConstantBranch, ConstantBranchError> {
    Ok(ValidatedConstantBranch {
        receipt: ConstantBranchReceipt {
            source_selected: source.selected_identity(),
            transformed_selected: selected_instruction_plan_identity(&proposed),
            optimization_unit: source.optimization_unit_identity(),
            fuel_schedule: source.fuel_schedule_identity(),
            function_index,
            branch,
        },
        transformed: Arc::new(proposed),
    })
}
