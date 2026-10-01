//! Select a remainder's exact divisor and prove it positive independently.
//! The existing affine rule checks the selected expression or SSA definition;
//! its presence never supplies its own nonzero premise. Restrict lookup to
//! definitions of the requested result or its proved value aliases. The shared
//! order machinery transports those equalities; no scan of unrelated arithmetic
//! rows or new cast authority is needed for a policy-erased remainder.

use proof_admission::{
    IntegerAffineWitness, ProofNode, ProofRule, check_integer_affine_witness,
    map_integer_affine_bound,
};
use semantic_vocabulary::{IntegerValue, Proposition, PropositionContext, ScalarTerm};

use super::super::super::super::affine_custody::DefinitionIndex;
use super::super::super::exact;

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> Option<ProofNode> {
    let Proposition::LessThan(goal_left, _) = goal else {
        return None;
    };
    let session = exact::session(context, assumptions, semantic_axioms);
    let aliases = session.value_aliases(goal_left);
    for target in std::iter::once(goal_left).chain(aliases.iter().map(|(alias, _)| alias)) {
        let positions = definitions.output_definitions_before(target, semantic_axioms.len());
        for position in std::iter::once(None).chain(positions.into_iter().map(Some)) {
            let expression = match position {
                None => target,
                Some(position) => {
                    let Proposition::Equal(result, expression) = &semantic_axioms[position] else {
                        continue;
                    };
                    if result != target {
                        continue;
                    }
                    expression
                }
            };
            let (integer_type, divisor) = match expression {
                ScalarTerm::ExactIntegerRemainder {
                    scalar_type, right, ..
                }
                | ScalarTerm::WrappingIntegerRemainder {
                    scalar_type, right, ..
                }
                | ScalarTerm::SaturatingIntegerRemainder {
                    scalar_type, right, ..
                } => (*scalar_type, right.as_ref()),
                _ => continue,
            };
            let witness = IntegerAffineWitness {
                root: divisor.clone(),
                target: target.clone(),
                definition_axioms: position.into_iter().collect(),
                literal_axioms: position.into_iter().map(|_| None).collect(),
            };
            let Ok(form) = check_integer_affine_witness(context, semantic_axioms, &witness) else {
                continue;
            };
            let zero = ScalarTerm::integer(integer_type, IntegerValue::Unsigned(0)).ok()?;
            // This leaf selector does not re-enter remainder production: an
            // expression cannot establish its own formation by recursive search.
            let Some(positive) = super::prove_without_subtract(
                context,
                &Proposition::LessThan(zero, divisor.clone()),
                assumptions,
                semantic_axioms,
            ) else {
                continue;
            };
            let Ok(conclusion) = map_integer_affine_bound(&form, &positive.conclusion) else {
                continue;
            };
            let proof = ProofNode {
                conclusion,
                rule: ProofRule::IntegerAffineBound {
                    root_bound: Box::new(positive),
                    witness,
                },
            };
            if let Some(proof) = super::complete(&session, goal, proof) {
                return Some(proof);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    use crate::proofs::nonzero_divisor_certificate::produce_checked_canonical_integer_proof;
    use proof_admission::{check_certificate, lift_fixed_integer_relation};
    use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType, ValueId};

    #[test]
    fn runtime_remainder_strict_bounds_replay_scalar_and_mathematical_goals() {
        let carrier = IntegerType::new(IntegerSign::Unsigned, 32).unwrap();
        let scalar_type = ScalarType::Integer(carrier);
        let value = |identity| ScalarTerm::value(ValueId::new(identity).unwrap(), scalar_type);
        let context = PropositionContext::from_value_types(
            (1..=4).map(|identity| (ValueId::new(identity).unwrap(), scalar_type)),
        )
        .unwrap();
        let zero = ScalarTerm::integer(carrier, IntegerValue::Unsigned(0)).unwrap();
        let assumptions = [Proposition::LessThan(zero, value(2))];
        for expression in [
            ScalarTerm::exact_integer_remainder(carrier, value(1), value(2)),
            ScalarTerm::wrapping_integer_remainder(carrier, value(1), value(2)),
            ScalarTerm::saturating_integer_remainder(carrier, value(1), value(2)),
        ] {
            let expression = expression.unwrap();
            let axioms = [
                Proposition::Equal(value(3), expression.clone()),
                Proposition::Equal(value(4), value(3)),
            ];
            let scalar_goal = Proposition::LessThan(value(3), value(2));
            let alias_goal = Proposition::LessThan(value(4), value(2));
            let direct_goal = Proposition::LessThan(expression, value(2));
            for goal in [
                direct_goal.clone(),
                scalar_goal.clone(),
                lift_fixed_integer_relation(&scalar_goal).unwrap(),
                alias_goal.clone(),
                lift_fixed_integer_relation(&alias_goal).unwrap(),
            ] {
                let produce = |assumptions: &[Proposition]| {
                    produce_checked_canonical_integer_proof(
                        &context,
                        &goal,
                        assumptions,
                        &axioms,
                        &BTreeSet::new(),
                        &mut crate::proofs::ProofSearchDeclines::default(),
                    )
                };
                let proof = produce(&assumptions)
                    .unwrap_or_else(|| panic!("positive runtime divisor must prove {goal:?}"));
                check_certificate(&context, &goal, &assumptions, &axioms, &proof).unwrap();
                assert!(produce(&[]).is_none());
                assert!(check_certificate(&context, &goal, &[], &axioms, &proof).is_err());
                let wrong_divisor = [
                    Proposition::Equal(
                        value(3),
                        ScalarTerm::wrapping_integer_remainder(carrier, value(1), value(1))
                            .unwrap(),
                    ),
                    axioms[1].clone(),
                ];
                if goal != direct_goal {
                    assert!(
                        check_certificate(&context, &goal, &assumptions, &wrong_divisor, &proof)
                            .is_err()
                    );
                }
                if goal == alias_goal || goal == lift_fixed_integer_relation(&alias_goal).unwrap() {
                    let changed_alias = [axioms[0].clone(), Proposition::Equal(value(4), value(1))];
                    assert!(
                        check_certificate(&context, &goal, &assumptions, &changed_alias, &proof)
                            .is_err()
                    );
                }
            }
        }
    }
}
