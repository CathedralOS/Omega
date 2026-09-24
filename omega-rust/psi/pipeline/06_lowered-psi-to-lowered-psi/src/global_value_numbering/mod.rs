//! Optimizer module role: stage group. Global value numbering.
//!
//! An unconditionally-total scalar operation that repeats an earlier identical
//! operation is a redundant computation: the earlier result is available
//! everywhere the duplicate is when the earlier block dominates the
//! duplicate's block, and both results are the same value. The rewrite removes
//! the duplicate and substitutes the surviving result at every direct scalar
//! use. Operands compare after resolving already-collapsed duplicates, so a
//! chain of equal computations converges on one canonical survivor: the first
//! matching operation in reverse postorder.
//!
//! Propositions, ranking rows, and custody sidecars name value identities
//! without listing direct uses: every value they can mention is retained, and
//! a ranked machine keeps its covered cyclic components' contents exact.
//! Inside a proof-bearing module a collapse survives only while the
//! reconstructed proof question is unchanged; a substitution the question
//! cannot carry verbatim refuses, leaving the complete closure as authored
//! until proof-context transport lets the question move with the rewrite.
//!
//! The scan proposes removals; the independent verifier re-derives the
//! canonical survivor for each removed operation and checks the exact
//! before/after relation and proof questions.

mod equivalents;

use crate::PsiOptimizationStageError;
use lowered_psi::LoweredPsi;
use std::collections::BTreeSet;
use terminal_psi::DebugSubject;
use terminal_verifier::{
    reconstruct_optimizable_terminal_obligations, validate_global_value_numbering,
    validate_module_for_optimization,
};

pub(super) fn number(before: LoweredPsi) -> Result<LoweredPsi, PsiOptimizationStageError> {
    let validated = validate_module_for_optimization(&before.semantic_module)
        .map_err(PsiOptimizationStageError::InvalidModule)?;
    let questions = reconstruct_optimizable_terminal_obligations(validated)
        .map_err(PsiOptimizationStageError::InvalidModule)?;
    // Call composition can include a callee's axioms in another machine's
    // obligation: the verifier's reconstructed-question check is the refusal
    // boundary for the complete closure, not just the obligation owner.
    let proof_bearing = !questions.obligations().is_empty();
    let mut after = before.clone();
    // These are semantic uses even when no executable operand reads the
    // value: proof projections and retained suspension frontiers survive
    // publication unchanged.
    let mut retained_values = BTreeSet::new();
    for projection in &before.semantic_module.float_meaning_projections {
        match &projection.source {
            terminal_psi::FloatMeaningSource::DirectOperationResult(result) => {
                retained_values.insert(result.result);
            }
            terminal_psi::FloatMeaningSource::DirectCallResult(result) => {
                retained_values.insert(result.result);
            }
            terminal_psi::FloatMeaningSource::DirectBlockParameter(parameter) => {
                retained_values.insert(parameter.parameter);
            }
            terminal_psi::FloatMeaningSource::TransitionalInput(_)
            | terminal_psi::FloatMeaningSource::DirectMachineParameter(_)
            | terminal_psi::FloatMeaningSource::DirectMachineResult(_)
            | terminal_psi::FloatMeaningSource::DirectStructuralLeaf(_)
            | terminal_psi::FloatMeaningSource::ExactBinary32Literal(_)
            | terminal_psi::FloatMeaningSource::ExactBinary64Literal(_)
            | terminal_psi::FloatMeaningSource::SemanticApplication(_) => {}
        }
    }
    for plan in &before.semantic_module.suspension_call_plans {
        for value in &plan.live_values {
            if let terminal_psi::TerminalSuspensionPlace::Scalar(value) = value.place {
                retained_values.insert(value);
            }
        }
    }
    for machine in &mut after.semantic_module.machines {
        equivalents::deduplicate(machine, &before.source_call_occurrences, &retained_values);
    }
    if let Err(error) =
        validate_global_value_numbering(&before.semantic_module, &after.semantic_module)
    {
        // Without proof-context transport a proof-bearing input keeps only a
        // rewrite the reconstructed question survives unchanged; a refused
        // rewrite leaves the module as authored.
        if proof_bearing
            && matches!(
                error,
                terminal_verifier::GlobalValueNumberingRewriteError::InvalidModule(_)
                    | terminal_verifier::GlobalValueNumberingRewriteError::ChangedProofQuestion
            )
        {
            return Ok(before);
        }
        return Err(PsiOptimizationStageError::InvalidGlobalValueNumberingRewrite(error));
    }
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
