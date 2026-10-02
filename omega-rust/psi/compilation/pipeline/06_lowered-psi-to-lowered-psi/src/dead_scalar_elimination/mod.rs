//! Optimizer module role: stage group. Dead total scalar elimination.
//!
//! Liveness proposes removals; the independent verifier checks the exact
//! before/after relation and proof questions. Goals, ordered requirements and
//! retained invariants stay exact; citations in existing certificates move by
//! reconstructed axiom identity. Both bundles replay independently. A removed
//! material premise refuses, leaving the complete closure as authored rather
//! than silently searching for another proof. The refusal is recorded on the
//! stage result.
//! Unused scalar block parameters are removed together with their dead
//! results, dropping the matching edge-argument positions. Ranking evidence
//! names exact values and covered member parameter tables: those stay live
//! while dead work outside the covered components still leaves.

mod liveness;

use crate::PsiOptimizationStageError;
use crate::rewrite_refusal::{PsiRewriteRefusal, PsiRewriteRefusalReason};
use lowered_psi::LoweredPsi;
use optimization::PsiOptimization;
use std::collections::BTreeSet;
use terminal_psi::DebugSubject;
use terminal_verifier::{
    reconstruct_optimizable_terminal_obligations, transport_dead_scalar_elimination_evidence,
    validate_module_for_optimization,
};

pub(super) fn eliminate(
    before: LoweredPsi,
    refusals: &mut Vec<PsiRewriteRefusal>,
) -> Result<LoweredPsi, PsiOptimizationStageError> {
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
    let mut retained_values = Vec::new();
    for projection in &before.semantic_module.float_meaning_projections {
        match &projection.source {
            terminal_psi::FloatMeaningSource::DirectOperationResult(result) => {
                retained_values.push(result.result)
            }
            terminal_psi::FloatMeaningSource::DirectCallResult(result) => {
                retained_values.push(result.result)
            }
            terminal_psi::FloatMeaningSource::DirectBlockParameter(parameter) => {
                retained_values.push(parameter.parameter)
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
            match value.place {
                terminal_psi::TerminalSuspensionPlace::Scalar(value) => retained_values.push(value),
                terminal_psi::TerminalSuspensionPlace::Structural { .. } => {}
            }
        }
    }
    // Join predicates remain the same proof questions. A value explicitly
    // named by a retained header is a semantic use, even when no executable
    // operation reads it after the selected rewrite.
    for invariant in &before.semantic_module.scalar_block_invariants {
        invariant.predicate.visit_value_ids(|value| {
            retained_values.push(value);
        });
    }
    for machine in &mut after.semantic_module.machines {
        liveness::eliminate(machine, &before.source_call_occurrences, &retained_values);
    }
    let transported = match transport_dead_scalar_elimination_evidence(
        &before.semantic_module,
        &after.semantic_module,
        &before.proof_bundle,
    ) {
        Ok(proof) => proof,
        Err(error) => {
            // The proposal cannot carry the existing evidence through its exact
            // reconstructed question. Preserve the complete input and record
            // the refusal; a new proof search must not turn this into a rewrite.
            let reason = match error {
                terminal_verifier::DeadScalarRewriteError::ChangedProofQuestion => {
                    Some(PsiRewriteRefusalReason::ChangedProofQuestion)
                }
                terminal_verifier::DeadScalarRewriteError::InvalidModule(_) => {
                    Some(PsiRewriteRefusalReason::ProposalLeftProofContextDangling)
                }
                _ => None,
            };
            if let Some(reason) = reason.filter(|_| proof_bearing) {
                refusals.push(PsiRewriteRefusal::new(
                    PsiOptimization::DeadPureScalarElimination,
                    reason,
                ));
                return Ok(before);
            }
            return Err(PsiOptimizationStageError::InvalidDeadScalarRewrite(error));
        }
    };
    after.proof_bundle = transported;
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
            .zip(&after.semantic_module.machines)
            .flat_map(|(old_machine, new_machine)| {
                let removed_results = old_machine
                    .blocks
                    .iter()
                    .flat_map(|block| &block.operations)
                    .filter(|operation| !operations.contains(&operation.id))
                    .filter_map(|operation| operation.result.scalar().map(|value| value.id));
                let removed_parameters =
                    old_machine.blocks.iter().zip(&new_machine.blocks).flat_map(
                        |(old_block, new_block)| {
                            let retained = new_block
                                .parameters
                                .iter()
                                .map(|parameter| parameter.id)
                                .collect::<BTreeSet<_>>();
                            old_block
                                .parameters
                                .iter()
                                .filter(move |parameter| !retained.contains(&parameter.id))
                                .map(|parameter| parameter.id)
                        },
                    );
                removed_results.chain(removed_parameters)
            })
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
