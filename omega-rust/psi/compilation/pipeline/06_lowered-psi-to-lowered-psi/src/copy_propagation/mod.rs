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
//! A join invariant's predicate is a proof question scoped to its destination
//! header, and a parameter the predicate names is retained like any other
//! record-named identity: the predicate keeps the exact question it asked.
//! Resolution proposes substitutions; the independent verifier checks the
//! exact before/after relation and proof questions.

mod copies;

use crate::PsiOptimizationStageError;
use lowered_psi::LoweredPsi;

pub(super) fn propagate(before: LoweredPsi) -> Result<LoweredPsi, PsiOptimizationStageError> {
    let mut after = before.clone();
    // These are semantic uses even when no executable operand reads the
    // value: proof projections, retained suspension frontiers, recorded
    // source-call operand joins, and the values a retained join invariant's
    // predicate names keep their exact identities.
    let mut retained_values =
        crate::retained_identities::record_named_rows(&after.semantic_module).values;
    for invariant in &after.semantic_module.scalar_block_invariants {
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
    crate::retained_identities::prune_debug_map(&before, &mut after)?;
    Ok(after)
}
