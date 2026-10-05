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
use std::sync::Arc;

use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::RegisterUnitId;
use selected_instructions::{
    MachineSemanticKind, SelectedFunction, SelectedInstructionId, SelectedInstructionPlan,
};
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{ConstantBooleanError, ConstantBooleanReceipt, ValidatedConstantBoolean};
use crate::ValidatedSelectedAnalysis;

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
/// the family publishes: one step per block plus one per instruction
/// across the plan, two scans of the reconstructed function's blocks for
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
    let function_scan = function
        .blocks
        .iter()
        .try_fold(0usize, |total, block| {
            total.checked_add(block.instructions.len())?.checked_add(1)
        })
        .ok_or(ConstantBooleanError::IdentityOverflow)?;
    let edge_count = successors
        .iter()
        .try_fold(0usize, |total, targets| total.checked_add(targets.len()))
        .ok_or(ConstantBooleanError::IdentityOverflow)?;
    let block_count = function.blocks.len();
    // The shared walk's setup resolves every edge's target index, fills
    // the predecessor lists, and marks the backward cone. Each used unit
    // then pays its partition checks — flag-universe and
    // materialize-surface membership and the producer-definitions lookup —
    // plus, for a flag unit, the block-prefix scan and — on an in-block
    // miss — every block's stream for its last event, after which
    // entry-set propagation requeues a block only while its set grows:
    // a set holds at most one element per event site plus the unknown
    // marker, so pops stay under `blocks × (elements + 1)` and each pop
    // visits its out-edges at a bounded union cost.
    let elements = function_scan
        .checked_add(1)
        .ok_or(ConstantBooleanError::IdentityOverflow)?;
    let pops = block_count
        .checked_mul(
            elements
                .checked_add(1)
                .ok_or(ConstantBooleanError::IdentityOverflow)?,
        )
        .ok_or(ConstantBooleanError::IdentityOverflow)?;
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
                    .checked_add(materialize_surface)?
                    .checked_add(producer_defs)?,
            )
        })
        .ok_or(ConstantBooleanError::IdentityOverflow)?;
    let walk_setup = edge_count
        .checked_mul(
            block_count
                .checked_add(1)
                .ok_or(ConstantBooleanError::IdentityOverflow)?,
        )
        .and_then(|total| total.checked_add(block_count))
        .and_then(|total| total.checked_add(edge_count))
        .and_then(|total| total.checked_add(flag_universe))
        .and_then(|total| total.checked_add(materialize_surface))
        .ok_or(ConstantBooleanError::IdentityOverflow)?;
    let reach_scan = implicit_uses
        .checked_mul(per_unit)
        .and_then(|total| total.checked_add(walk_setup))
        .ok_or(ConstantBooleanError::IdentityOverflow)?;
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
        .ok_or(ConstantBooleanError::IdentityOverflow)?;
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
    Ok(ValidatedConstantBoolean {
        receipt: ConstantBooleanReceipt {
            source_selected: source.selected_identity(),
            transformed_selected: selected_instruction_plan_identity(&proposed),
            optimization_unit: source.optimization_unit_identity(),
            fuel_schedule: source.fuel_schedule_identity(),
            function_index,
            materialization,
        },
        transformed: Arc::new(proposed),
    })
}
