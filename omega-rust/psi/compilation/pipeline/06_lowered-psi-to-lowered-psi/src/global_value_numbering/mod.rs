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
mod validation;

pub(super) use validation::GlobalValueNumberingRewriteError;

use crate::PsiOptimizationStageError;
use lowered_psi::LoweredPsi;
use std::collections::BTreeSet;
use validation::validate_global_value_numbering;

pub(super) fn number(before: LoweredPsi) -> Result<LoweredPsi, PsiOptimizationStageError> {
    let mut after = before.clone();
    // These are semantic uses even when no executable operand reads the
    // value: proof projections and retained suspension frontiers survive
    // publication unchanged.
    let mut retained_values = BTreeSet::new();
    retained_values.extend(crate::retained_identities::quotient_call_results(
        &before.semantic_module,
    ));
    retained_values.extend(crate::retained_identities::boundary_application_results(
        &before.semantic_module,
    ));
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
        plan.visit_value_ids(|value| {
            retained_values.insert(value);
        });
    }
    for machine in &mut after.semantic_module.machines {
        equivalents::deduplicate(machine, &before.source_call_occurrences, &retained_values);
    }
    validate_global_value_numbering(&before.semantic_module, &after.semantic_module)
        .map_err(PsiOptimizationStageError::InvalidGlobalValueNumberingRewrite)?;
    crate::retained_identities::prune_debug_map(&before, &mut after)?;
    Ok(after)
}
