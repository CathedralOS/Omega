//! Optimizer module role: stage group. Dead total scalar elimination.
//!
//! Liveness proposes removals; the independent verifier checks the exact
//! before/after relation and proof questions. Proof-bearing closures remain
//! unchanged until proof-context transport is implemented, not silently re-proved.

mod liveness;

use crate::PsiOptimizationStageError;
use lowered_psi::LoweredPsi;
use std::collections::BTreeSet;
use terminal_psi::DebugSubject;
use terminal_verifier::{
    reconstruct_optimizable_terminal_obligations, validate_dead_scalar_elimination,
    validate_module_for_optimization,
};

pub(super) fn eliminate(before: LoweredPsi) -> Result<LoweredPsi, PsiOptimizationStageError> {
    let validated = validate_module_for_optimization(&before.semantic_module)
        .map_err(PsiOptimizationStageError::InvalidModule)?;
    let questions = reconstruct_optimizable_terminal_obligations(validated)
        .map_err(PsiOptimizationStageError::InvalidModule)?;
    let mut after = before.clone();
    // Call composition can include a callee's axioms in another machine's
    // obligation. Protect the complete closure, not just the obligation owner.
    if questions.obligations().is_empty() {
        // These are semantic uses even when no executable operand reads the
        // value: proof projections and retained suspension frontiers survive
        // publication unchanged.
        let mut retained_values = Vec::new();
        for projection in &before.semantic_module.float_meaning_projections {
            match &projection.source {
                terminal_psi::FloatMeaningSource::DirectOperationResult(result) => {
                    retained_values.push(result.result)
                }
                terminal_psi::FloatMeaningSource::DirectCallResult(result) => {
                    retained_values.push(result.result)
                }
                terminal_psi::FloatMeaningSource::TransitionalInput(_)
                | terminal_psi::FloatMeaningSource::DirectMachineParameter(_)
                | terminal_psi::FloatMeaningSource::DirectMachineResult(_)
                | terminal_psi::FloatMeaningSource::DirectBlockParameter(_)
                | terminal_psi::FloatMeaningSource::DirectStructuralLeaf(_)
                | terminal_psi::FloatMeaningSource::ExactBinary32Literal(_)
                | terminal_psi::FloatMeaningSource::ExactBinary64Literal(_) => {}
            }
        }
        for plan in &before.semantic_module.suspension_call_plans {
            for value in &plan.live_values {
                match value.place {
                    terminal_psi::TerminalSuspensionPlace::Scalar(value) => {
                        retained_values.push(value)
                    }
                    terminal_psi::TerminalSuspensionPlace::Structural { .. } => {}
                }
            }
        }
        for machine in &mut after.semantic_module.machines {
            liveness::eliminate(machine, &before.source_call_occurrences, &retained_values);
        }
    }
    validate_dead_scalar_elimination(&before.semantic_module, &after.semantic_module)
        .map_err(PsiOptimizationStageError::InvalidDeadScalarRewrite)?;
    if let Some(debug) = after.debug_map.as_mut() {
        let operations = after
            .semantic_module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .map(|operation| operation.id)
            .collect::<BTreeSet<_>>();
        let removed_values = before
            .semantic_module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .filter(|operation| !operations.contains(&operation.id))
            .filter_map(|operation| operation.result.scalar().map(|value| value.id))
            .collect::<BTreeSet<_>>();
        debug.sites.retain(|site| match site.subject {
            DebugSubject::Operation(operation) => operations.contains(&operation),
            DebugSubject::Value(value) => !removed_values.contains(&value),
            _ => true,
        });
        debug.semantic = terminal_codec::terminal_psi_identity(&after.semantic_module)
            .map_err(PsiOptimizationStageError::InvalidSemantic)?;
    }
    Ok(after)
}
