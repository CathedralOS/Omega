//! Optimizer module role: stage group. Scalar copy propagation.
//!
//! A block parameter bound to the same resolved value on every inventoried
//! incoming `Jump`/`Conditional` edge is a copy of that value. The rewrite
//! resolves copies transitively, substitutes the resolved source at every
//! direct scalar use, and removes the parameter with its edge-argument
//! positions. Propositions and ranking evidence name value identities without
//! listing direct uses: every value a proposition can mention is retained, and
//! a ranked machine keeps the covered cyclic components' parameter tables and
//! every use reaching into them exact while ordinary regions still collapse.
//!
//! Inside a proof-bearing module the independently checked copy relation
//! transports the same goals and ordered requirements, then remaps existing
//! certificate citations to reconstructed output axioms. Kernel replay of both
//! bundles is required; a vanished material premise refuses rather than being
//! re-proved. The refusal is recorded on the stage result.
//!
//! Resolution proposes substitutions; the independent verifier checks the
//! exact before/after relation and proof questions.

mod copies;

use crate::PsiOptimizationStageError;
use crate::rewrite_refusal::{PsiRewriteRefusal, PsiRewriteRefusalReason};
use lowered_psi::LoweredPsi;
use optimization::PsiOptimization;
use std::collections::BTreeSet;
use terminal_psi::DebugSubject;
use terminal_verifier::{
    reconstruct_optimizable_terminal_obligations, transport_copy_propagation_evidence,
    validate_module_for_optimization,
};

pub(super) fn propagate(
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
    // value: proof projections, retained suspension frontiers, and
    // recorded source-call operand joins keep their exact identities.
    let mut retained_values = BTreeSet::new();
    for projection in &before.semantic_module.float_meaning_projections {
        if let terminal_psi::FloatMeaningSource::DirectBlockParameter(parameter) =
            &projection.source
        {
            retained_values.insert(parameter.parameter);
        }
    }
    for plan in &before.semantic_module.suspension_call_plans {
        for value in &plan.live_values {
            if let terminal_psi::TerminalSuspensionPlace::Scalar(value) = value.place {
                retained_values.insert(value);
            }
        }
    }
    // Join predicates remain the same proof questions. A value explicitly
    // named by a retained header is a semantic use, even when no executable
    // operation reads it after the selected rewrite.
    for invariant in &before.semantic_module.scalar_block_invariants {
        invariant.predicate.visit_value_ids(|value| {
            retained_values.insert(value);
        });
    }
    for machine in &mut after.semantic_module.machines {
        copies::propagate(
            machine,
            &before.source_call_occurrences,
            &mut retained_values,
        );
    }
    let transported = match transport_copy_propagation_evidence(
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
                terminal_verifier::CopyPropagationRewriteError::ChangedProofQuestion => {
                    Some(PsiRewriteRefusalReason::ChangedProofQuestion)
                }
                terminal_verifier::CopyPropagationRewriteError::InvalidModule(_) => {
                    Some(PsiRewriteRefusalReason::ProposalLeftProofContextDangling)
                }
                _ => None,
            };
            if let Some(reason) = reason.filter(|_| proof_bearing) {
                refusals.push(PsiRewriteRefusal::new(
                    PsiOptimization::CopyPropagation,
                    reason,
                ));
                return Ok(before);
            }
            return Err(PsiOptimizationStageError::InvalidCopyPropagationRewrite(
                error,
            ));
        }
    };
    after.proof_bundle = transported;
    if let Some(debug) = after.debug_map.as_mut() {
        let removed_values = before
            .semantic_module
            .machines
            .iter()
            .zip(&after.semantic_module.machines)
            .flat_map(|(old_machine, new_machine)| {
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
                )
            })
            .collect::<BTreeSet<_>>();
        debug.sites.retain(|site| match site.subject {
            DebugSubject::Value(value) => !removed_values.contains(&value),
            _ => true,
        });
        debug.semantic = terminal_codec::terminal_psi_identity(&after.semantic_module)
            .map_err(PsiOptimizationStageError::InvalidSemantic)?;
    }
    Ok(after)
}
