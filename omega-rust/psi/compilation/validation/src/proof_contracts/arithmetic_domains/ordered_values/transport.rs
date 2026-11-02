//! Project live integer observations into a state's delivered values.
//!
//! The effect owner has already retired relations invalidated by argument
//! evaluation. An undelivered scalar can still equal a delivered observation:
//! `limit == cursor.available` gives a meaning to `limit - start` after only
//! `cursor` and `start` cross the edge. Rebind its equal value inside the normal
//! operand tree; do not preserve an out-of-scope declaration or replay source.
//! Ordering alone cannot authorize this substitution. After equalities, the
//! existing difference walk projects consequences through forgotten operands.

use super::{Operand, Relation, RelationConstraint, ValueComparison, ordered_gaps_with_comparison};

pub(in crate::proof_contracts::arithmetic_domains) fn rebind_relations(
    relations: &[Relation],
    bindings: &[(Operand, Operand)],
) -> Vec<Relation> {
    if relations.is_empty() || bindings.is_empty() {
        return Vec::new();
    }
    let bindings = bind_equal_observations(relations, bindings);
    let mut rebound = Vec::new();
    for relation in relations
        .iter()
        .flat_map(|relation| relation.rebound(&bindings))
    {
        if !rebound.contains(&relation) {
            rebound.push(relation);
        }
    }
    let mut operands = Vec::new();
    for relation in relations {
        for operand in [&relation.left, &relation.right] {
            if !operands.contains(&operand) {
                operands.push(operand);
            }
        }
    }
    let delivered = operands
        .iter()
        .map(|operand| operand.rebound(&bindings))
        .collect::<Vec<_>>();
    if delivered.iter().all(|operands| !operands.is_empty()) {
        return rebound;
    }
    let mut comparison = ValueComparison::new(relations);
    for (operand, targets) in operands.iter().zip(&delivered) {
        if targets.is_empty() {
            continue;
        }
        for (ceiling, floor) in ordered_gaps_with_comparison(relations, operand, &mut comparison) {
            if ceiling == *operand {
                continue;
            }
            let Some(left_targets) = operands
                .iter()
                .position(|existing| *existing == ceiling)
                .map(|idx| &delivered[idx])
            else {
                continue;
            };
            if left_targets.is_empty() {
                continue;
            }
            for left in left_targets {
                for right in targets {
                    let relation = Relation {
                        left: left.clone(),
                        right: right.clone(),
                        constraint: RelationConstraint::MinimumDifference(floor),
                    };
                    if !rebound.contains(&relation) {
                        rebound.push(relation);
                    }
                }
            }
        }
    }
    rebound
}

fn bind_equal_observations(
    relations: &[Relation],
    bindings: &[(Operand, Operand)],
) -> Vec<(Operand, Operand)> {
    // The owner records equality as opposite non-strict orders. Both must
    // still be live here; one direction or an exclusion is not equality.
    let mut equalities = relations
        .iter()
        .filter(|relation| {
            relation.constraint == RelationConstraint::MinimumDifference(0)
                && relations.iter().any(|opposite| {
                    opposite.constraint == RelationConstraint::MinimumDifference(0)
                        && opposite.left == relation.right
                        && opposite.right == relation.left
                })
        })
        .collect::<Vec<_>>();
    if equalities.is_empty() {
        return bindings.to_vec();
    }
    let mut bindings = bindings.to_vec();
    loop {
        let previous_count = bindings.len();
        equalities.retain(|equality| {
            if !equality.left.rebound(&bindings).is_empty() {
                return false;
            }
            if let Some(value) = equality.right.rebound(&bindings).into_iter().next() {
                // One delivered representative suffices. Original bindings
                // retain every actual argument; this does not multiply equal
                // expression variants or create another declaration identity.
                bindings.push((equality.left.clone(), value));
                return false;
            }
            true
        });
        // Each iteration binds a previously unbound member of the finite
        // equality roster. Cyclic equalities without a delivered anchor add
        // nothing, and known mappings are never recursively expanded again.
        if bindings.len() == previous_count || equalities.is_empty() {
            return bindings;
        }
    }
}
