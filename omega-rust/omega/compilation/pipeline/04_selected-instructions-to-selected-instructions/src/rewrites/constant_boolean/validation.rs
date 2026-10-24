//! Independent validation of constant condition materialization.
//!
//! The validator never calls [`super::admission`] and never reads the
//! `pair` descriptor table the producer consults: it re-derives the
//! fold's legality from the source records — the named instruction's
//! `MaterializeBoolean*` kind and emitted `[def result]` shape, the flag
//! units it reads partitioned against the environment's own compare rows
//! and resolving through the shared condition-state walk to one compare's
//! published definitions, that compare's compile-time constant operands,
//! and the target's own materialize row the rewritten instruction is
//! built from — then rebuilds the instruction the contract demands and
//! requires the proposal to carry it at the materialization's position.
//! The catalog effect surface is restated here too: the consumer's row
//! must encode an effect-isolated fall-through whose implicit uses stay
//! inside the flag universe, and the materialize row an isolated
//! fall-through with no implicit unit traffic at all — read off the bound
//! catalog directly. Reinserting the source boolean must restore the
//! complete source by content: every other instruction, register, call,
//! and function included. A producer admission error therefore fails
//! validation even when the proposal is exactly what that producer
//! emitted.
use std::collections::BTreeSet;

use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::RegisterUnitId;
use selected_instructions::{
    MachineSemanticKind, SelectedFunction, SelectedInstructionId, SelectedInstructionPlan,
};

use super::{ConstantBooleanError, ConstantBooleanReceipt, ValidatedConstantBoolean};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::sealed;

/// The flag-universe and materialize-surface sizes the measured-step
/// contract charges — the union of the target's three compare rows'
/// implicit definitions and the materialize row's implicit surface, under
/// the same deduplication the audit's own sets apply. Discovery charges
/// these once per pass, and the reached compare's published definitions
/// stay bounded by the flag universe they are drawn from.
pub(crate) fn surface_sizes(
    environment: &ValidatedTargetRegisterEnvironment,
) -> Result<(usize, usize), ConstantBooleanError> {
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
                .ok_or(ConstantBooleanError::ConstraintMismatch)?
                .implicit_defs
                .iter()
                .copied(),
        );
    }
    let materialize_row = environment
        .constraint(
            keys.for_semantic(MachineSemanticKind::MaterializeI64)
                .ok_or(ConstantBooleanError::ConstraintMismatch)?,
        )
        .ok_or(ConstantBooleanError::ConstraintMismatch)?;
    let materialize_surface: BTreeSet<RegisterUnitId> = materialize_row
        .implicit_uses
        .iter()
        .chain(materialize_row.implicit_defs.iter())
        .chain(materialize_row.clobbers.iter())
        .copied()
        .collect();
    Ok((flag_universe.len(), materialize_surface.len()))
}

/// The validation work this audit performs, in the measured-step contract
/// the family publishes: the shared rewrite scan (one step per block
/// and instruction of the function, one per plan function), two scans of the reconstructed function's blocks for
/// the producer audit, and the shared walk's setup and fixpoint bound at
/// one per used unit — flag or not — so the bound stays independent of
/// the partition split. The `flag_universe`, `materialize_surface`, and
/// `producer_defs` surface sizes are caller-supplied so discovery can
/// charge the contract before admission runs; a pre-admission caller
/// bounds `producer_defs` by `flag_universe`, which covers the published
/// definitions of every compare row the union was built from.
pub(crate) fn measured_steps(
    plan: &SelectedInstructionPlan,
    function: &SelectedFunction,
    successors: &[Vec<usize>],
    implicit_uses: usize,
    flag_universe: usize,
    materialize_surface: usize,
    producer_defs: usize,
) -> Result<u64, ConstantBooleanError> {
    let steps = crate::rewrites::constant_fold_steps(
        plan,
        function,
        successors,
        implicit_uses,
        flag_universe,
        materialize_surface,
        producer_defs,
        || ConstantBooleanError::IdentityOverflow,
    )?;
    u64::try_from(steps).map_err(|_| ConstantBooleanError::IdentityOverflow)
}

/// Admit a freshly produced plan with its receipt; the producer calls this
/// instead of replaying its own output.
pub(crate) fn seal_constant_boolean_fold(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    materialization: SelectedInstructionId,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedConstantBoolean, ConstantBooleanError> {
    Ok(
        sealed!(ValidatedConstantBoolean, ConstantBooleanReceipt, source, proposed, function_index: function_index, materialization: materialization),
    )
}
