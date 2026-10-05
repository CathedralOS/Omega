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
//! A retained join invariant is carried instead of frozen: when a copy
//! parameter its predicate names collapses, the predicate is rewritten with
//! the same resolved source, which is exactly the substitution the verifier's
//! reconstructed-question transport applies on the other side. If that
//! transport fails anyway, the stage retries with the named values retained —
//! the earlier unconditional boundary — before recording a refusal.
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
    // One proposal, two treatments of the retained join invariants: the
    // rewriting attempt lets a copy parameter a predicate names collapse
    // while rewriting the predicate to the resolved source; the retained
    // attempt keeps every named value, which is the boundary this stage held
    // before join transport existed. The retained attempt only runs when
    // transport cannot carry a rewritten predicate, so a rewrite that was
    // clean before cannot regress into a refusal.
    let propose = |rewrite_invariants: bool| {
        let mut after = before.clone();
        // These are semantic uses even when no executable operand reads the
        // value: proof projections, retained suspension frontiers, and
        // recorded source-call operand joins keep their exact identities.
        let mut retained_values = BTreeSet::new();
        for projection in &after.semantic_module.float_meaning_projections {
            if let terminal_psi::FloatMeaningSource::DirectBlockParameter(parameter) =
                &projection.source
            {
                retained_values.insert(parameter.parameter);
            }
        }
        for plan in &after.semantic_module.suspension_call_plans {
            for value in &plan.live_values {
                if let terminal_psi::TerminalSuspensionPlace::Scalar(value) = value.place {
                    retained_values.insert(value);
                }
            }
        }
        if !rewrite_invariants {
            for invariant in &after.semantic_module.scalar_block_invariants {
                invariant.predicate.visit_value_ids(|value| {
                    retained_values.insert(value);
                });
            }
        }
        let module = &mut after.semantic_module;
        let invariants = &mut module.scalar_block_invariants;
        for machine in &mut module.machines {
            copies::propagate(
                machine,
                invariants,
                &before.source_call_occurrences,
                &mut retained_values,
            );
        }
        after
    };
    let after = propose(true);
    let invariants_rewritten = after.semantic_module.scalar_block_invariants
        != before.semantic_module.scalar_block_invariants;
    let first = transport_copy_propagation_evidence(
        &before.semantic_module,
        &after.semantic_module,
        &before.proof_bundle,
    );
    // A rewritten predicate must also remain publishable: remapped identities
    // can reorder the conjunction's canonical member list, and a module whose
    // stored proposition no longer encodes is not a candidate. When the
    // rewritten attempt fails either check — and only an invariant rewrite
    // separates it from the retained proposal — retry that boundary rather
    // than spending a refusal on the join.
    let publishable = first.is_ok()
        && (!invariants_rewritten
            || terminal_codec::terminal_psi_identity(&after.semantic_module).is_ok());
    let (after, transported) = if publishable {
        (
            after,
            first.expect("publishable implies the transport succeeded"),
        )
    } else if invariants_rewritten {
        let retained_after = propose(false);
        match transport_copy_propagation_evidence(
            &before.semantic_module,
            &retained_after.semantic_module,
            &before.proof_bundle,
        ) {
            Ok(proof) => (retained_after, proof),
            Err(error) => {
                withhold_or_fail(error, proof_bearing, refusals)?;
                return Ok(before);
            }
        }
    } else {
        withhold_or_fail(
            first.expect_err("not publishable with unchanged invariants"),
            proof_bearing,
            refusals,
        )?;
        return Ok(before);
    };
    let mut after = after;
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

/// Map one transport failure onto the recorded refusal surface. A refusal
/// reason exists only for the two boundaries that preserve the complete
/// input — a changed proof question or a proposal that left proof context
/// dangling — and only a proof-bearing module can owe one. Anything else is
/// a stage error, and a refusal pushes onto `refusals` for the caller to
/// return the input module with.
fn withhold_or_fail(
    error: terminal_verifier::CopyPropagationRewriteError,
    proof_bearing: bool,
    refusals: &mut Vec<PsiRewriteRefusal>,
) -> Result<(), PsiOptimizationStageError> {
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
        return Ok(());
    }
    Err(PsiOptimizationStageError::InvalidCopyPropagationRewrite(
        error,
    ))
}
