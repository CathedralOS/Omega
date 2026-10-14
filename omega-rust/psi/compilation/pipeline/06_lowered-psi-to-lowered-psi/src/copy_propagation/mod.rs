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
use lowered_psi::LoweredPsi;
use std::collections::BTreeSet;

pub(super) fn propagate(before: LoweredPsi) -> Result<LoweredPsi, PsiOptimizationStageError> {
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
        retained_values.extend(crate::retained_identities::quotient_call_results(
            &after.semantic_module,
        ));
        for projection in &after.semantic_module.float_meaning_projections {
            if let terminal_psi::FloatMeaningSource::DirectBlockParameter(parameter) =
                &projection.source
            {
                retained_values.insert(parameter.parameter);
            }
        }
        for plan in &after.semantic_module.suspension_call_plans {
            plan.visit_value_ids(|value| {
                retained_values.insert(value);
            });
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
    let mut after = propose(false);
    crate::retained_identities::prune_debug_map(&before, &mut after)?;
    Ok(after)
}
