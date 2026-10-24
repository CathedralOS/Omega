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

use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::RegisterUnitId;
use selected_instructions::{SelectedFunction, SelectedInstructionId, SelectedInstructionPlan};

use super::{ConstantBranchError, ConstantBranchReceipt, ValidatedConstantBranch};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::sealed;

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
/// the family publishes: the shared rewrite scan (one step per block
/// and instruction of the function, one per plan function), two scans of the reconstructed function's blocks for
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
    let steps = crate::rewrites::constant_fold_steps(
        plan,
        function,
        successors,
        implicit_uses,
        flag_universe,
        jump_surface,
        compare_defs,
        || ConstantBranchError::IdentityOverflow,
    )?;
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
    Ok(
        sealed!(ValidatedConstantBranch, ConstantBranchReceipt, source, proposed, function_index: function_index, branch: branch),
    )
}
