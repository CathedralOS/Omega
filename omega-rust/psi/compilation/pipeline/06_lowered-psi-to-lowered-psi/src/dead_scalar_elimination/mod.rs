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
use lowered_psi::LoweredPsi;

pub(super) fn eliminate(before: LoweredPsi) -> Result<LoweredPsi, PsiOptimizationStageError> {
    let mut after = before.clone();
    // These are semantic uses even when no executable operand reads the
    // value: proof projections, retained suspension frontiers, boundary
    // results and the quotient catalogue's correspondence endpoints survive
    // publication unchanged.
    let mut retained_values =
        crate::retained_identities::record_named_rows(&before.semantic_module).values;
    retained_values.extend(crate::retained_identities::boundary_application_results(
        &before.semantic_module,
    ));
    // Join predicates remain the same proof questions. A value explicitly
    // named by a retained header is a semantic use, even when no executable
    // operation reads it after the selected rewrite.
    for invariant in &before.semantic_module.scalar_block_invariants {
        invariant.predicate.visit_value_ids(|value| {
            retained_values.insert(value);
        });
    }
    for machine in &mut after.semantic_module.machines {
        liveness::eliminate(machine, &before.source_call_occurrences, &retained_values);
    }
    crate::retained_identities::prune_debug_map(&before, &mut after)?;
    Ok(after)
}
