//! Preserve a guarded live endpoint across an independently checked cast word.
//!
//! A byte index is widened after its mixed-width source comparison. The
//! original comparison and later storage reads must first rejoin through
//! retained equalities; the cast witness then moves just that endpoint. No
//! guessed length or source-shape bound substitutes for the original guard.

use proof_admission::{PrimitiveJudgment, ProofNode, ProofRule, lift_fixed_integer_relation};
use semantic_vocabulary::{
    IntegerMathTerm, Proposition, PropositionContext, ScalarTerm, ScalarType,
};

use super::super::{cast_custody, integer_evidence::projected_facts, integer_selection};

pub(in super::super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    let (strict, left, right) = match goal {
        Proposition::LessThan(left, right) => (true, left, right),
        Proposition::LessOrEqual(left, right) => (false, left, right),
        _ => return None,
    };
    for (endpoint, target) in [left, right].into_iter().enumerate() {
        let Some((root, _)) = cast_custody::source_root(target, semantic_axioms) else {
            continue;
        };
        let (root_left, root_right) = if endpoint == 0 {
            (root.clone(), right.clone())
        } else {
            (left.clone(), root.clone())
        };
        let scalar_shape = if strict {
            Proposition::LessThan(root_left, root_right)
        } else {
            Proposition::LessOrEqual(root_left, root_right)
        };
        // Lift without pretending the mixed-width scalar shape is a valid
        // scalar proposition. Only the mathematical relation is published.
        let Some(root_goal) = lift_fixed_integer_relation(&scalar_shape) else {
            continue;
        };
        let Some(root_bound) = rejoin_guard(context, &root_goal, assumptions, semantic_axioms)
        else {
            continue;
        };
        if let Some(proof) = cast_custody::prove_from_root(
            context,
            goal,
            assumptions,
            semantic_axioms,
            &root,
            root_bound,
        ) {
            return Some(proof);
        }
    }
    None
}

fn rejoin_guard(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    let (strict, goal_left, goal_right) = order(goal)?;
    for fact in projected_facts(assumptions, semantic_axioms) {
        let Some((fact_strict, _, _)) = order(fact.proposition) else {
            continue;
        };
        if strict != fact_strict {
            continue;
        }
        let mut proof = fact.proof();
        for (endpoint, target) in [goal_left, goal_right].into_iter().enumerate() {
            let (_, left, right) = order(&proof.conclusion)?;
            let old = if endpoint == 0 { left } else { right };
            if old == target {
                continue;
            }
            let (Some(old_scalar), Some(target_scalar)) = (scalar_value(old), scalar_value(target))
            else {
                break;
            };
            let equality_goal = Proposition::Equal(old_scalar, target_scalar.clone());
            let Some(equality) =
                integer_selection::build(context, &equality_goal, assumptions, semantic_axioms)
            else {
                break;
            };
            let Some(mathematical_equality) = lift_fixed_integer_relation(&equality_goal) else {
                break;
            };
            // A checked transitivity node lifts the scalar equality without
            // relabeling an assumption or conjunction-projection certificate.
            let equality = ProofNode {
                conclusion: mathematical_equality,
                rule: ProofRule::EqualityTransitivity {
                    left_equals_middle: Box::new(equality),
                    middle_equals_right: Box::new(ProofNode {
                        conclusion: Proposition::Equal(target_scalar.clone(), target_scalar),
                        rule: ProofRule::Primitive(PrimitiveJudgment::ReflexiveEquality),
                    }),
                },
            };
            let (left, right) = if endpoint == 0 {
                (target.clone(), right.clone())
            } else {
                (left.clone(), target.clone())
            };
            let conclusion = if strict {
                Proposition::IntegerMathLessThan(left, right)
            } else {
                Proposition::IntegerMathLessOrEqual(left, right)
            };
            proof = ProofNode {
                conclusion,
                rule: ProofRule::IntegerOrderSubstitution {
                    relation: Box::new(proof),
                    equality: Box::new(equality),
                    endpoint,
                },
            };
        }
        if proof.conclusion == *goal {
            return Some(proof);
        }
    }
    None
}

fn order(proposition: &Proposition) -> Option<(bool, &IntegerMathTerm, &IntegerMathTerm)> {
    match proposition {
        Proposition::IntegerMathLessThan(left, right) => Some((true, left, right)),
        Proposition::IntegerMathLessOrEqual(left, right) => Some((false, left, right)),
        _ => None,
    }
}

fn scalar_value(term: &IntegerMathTerm) -> Option<ScalarTerm> {
    let IntegerMathTerm::MathValue { source_type, value } = term else {
        return None;
    };
    Some(ScalarTerm::value(*value, ScalarType::Integer(*source_type)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use proof_admission::check_certificate;
    use semantic_vocabulary::{IntegerSign, IntegerType, ValueId};

    #[test]
    fn mixed_width_guard_rejoins_widened_index_and_reloaded_length() {
        let narrow = IntegerType::new(IntegerSign::Unsigned, 32).unwrap();
        let wide = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
        let integer_type = |identity| if identity <= 3 { narrow } else { wide };
        let value = |identity| {
            ScalarTerm::value(
                ValueId::new(identity).unwrap(),
                ScalarType::Integer(integer_type(identity)),
            )
        };
        let mathematical = |identity| IntegerMathTerm::MathValue {
            source_type: integer_type(identity),
            value: ValueId::new(identity).unwrap(),
        };
        let context = PropositionContext::from_value_types((1..=7).map(|identity| {
            (
                ValueId::new(identity).unwrap(),
                ScalarType::Integer(integer_type(identity)),
            )
        }))
        .unwrap();
        let axioms = vec![
            Proposition::Equal(value(1), value(3)),
            Proposition::Equal(value(2), value(3)),
            Proposition::Equal(value(4), value(7)),
            Proposition::Equal(value(5), value(7)),
            Proposition::IntegerMathLessThan(mathematical(1), mathematical(4)),
            Proposition::Equal(
                value(6),
                ScalarTerm::integer_widen(narrow, wide, value(2)).unwrap(),
            ),
        ];
        let goal = Proposition::LessThan(value(6), value(5));
        let produce = |facts: &[Proposition]| {
            crate::proofs::nonzero_divisor_certificate::prove_canonical_integer_proposition(
                &context,
                &goal,
                &[],
                facts,
            )
        };
        let proof = produce(&axioms)
            .expect("guarded index preserves both reloaded endpoints through widening");
        check_certificate(&context, &goal, &[], &axioms, &proof).unwrap();
        for missing in 0..axioms.len() {
            let mut incomplete = axioms.clone();
            incomplete[missing] = Proposition::Truth;
            assert!(produce(&incomplete).is_none(), "missing citation {missing}");
            assert!(check_certificate(&context, &goal, &[], &incomplete, &proof).is_err());
        }
        let mut inclusive = axioms.clone();
        inclusive[4] = Proposition::IntegerMathLessOrEqual(mathematical(1), mathematical(4));
        assert!(produce(&inclusive).is_none());
        assert!(check_certificate(&context, &goal, &[], &inclusive, &proof).is_err());
        let mut conditional = axioms.clone();
        conditional[4] = Proposition::Implication {
            premise: Box::new(Proposition::Falsehood),
            conclusion: Box::new(axioms[4].clone()),
        };
        assert!(produce(&conditional).is_none());
    }
}
