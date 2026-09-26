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
//! Inside a proof-bearing module a collapse survives only while the
//! reconstructed proof question is unchanged; a substitution the question
//! cannot carry verbatim refuses until proof-context transport lets the
//! question move with the rewrite.
//!
//! Resolution proposes substitutions; the independent verifier checks the
//! exact before/after relation and proof questions.

mod copies;

use crate::PsiOptimizationStageError;
use lowered_psi::LoweredPsi;
use std::collections::BTreeSet;
use terminal_psi::DebugSubject;
use terminal_verifier::{
    reconstruct_optimizable_terminal_obligations, validate_copy_propagation,
    validate_module_for_optimization,
};

pub(super) fn propagate(before: LoweredPsi) -> Result<LoweredPsi, PsiOptimizationStageError> {
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
    for machine in &mut after.semantic_module.machines {
        copies::propagate(
            machine,
            &before.source_call_occurrences,
            &mut retained_values,
        );
    }
    if let Err(error) = validate_copy_propagation(&before.semantic_module, &after.semantic_module) {
        // Without proof-context transport a proof-bearing input keeps only a
        // rewrite the reconstructed question survives unchanged; a refused
        // rewrite leaves the module as authored.
        if proof_bearing
            && matches!(
                error,
                terminal_verifier::CopyPropagationRewriteError::InvalidModule(_)
                    | terminal_verifier::CopyPropagationRewriteError::ChangedProofQuestion
            )
        {
            return Ok(before);
        }
        return Err(PsiOptimizationStageError::InvalidCopyPropagationRewrite(
            error,
        ));
    }
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
