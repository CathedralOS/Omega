//! Translate an established integer bound into an equivalent mathematical
//! arrangement. Search produces ordinary kernel rules, never a new assumption:
//! both endpoints receive the same total addend, then closed ring identities
//! restore the requested endpoints. Unmatched subjects and selected wrapping
//! policies cannot cancel and therefore establish nothing.
use semantic_vocabulary::{IntegerMathTerm, Proposition, PropositionContext};
use terminal_psi::{PrimitiveJudgment, ProofNode, ProofRule};

pub(super) fn derive(
    context: &PropositionContext,
    goal: &Proposition,
    premise: &ProofNode,
    assumptions: usize,
) -> Option<ProofNode> {
    let Proposition::IntegerMathLessOrEqual(goal_left, goal_right) = goal else {
        return None;
    };
    proof_admission::check_predicate_evaluation_size([goal, &premise.conclusion]).ok()?;
    let mut relation = match &premise.conclusion {
        Proposition::IntegerMathLessOrEqual(..) => premise.clone(),
        Proposition::LessOrEqual(..) => super::bridge(
            proof_admission::lift_fixed_integer_relation(&premise.conclusion)?,
            premise.clone(),
            assumptions,
        ),
        _ => return None,
    };
    // The opaque embedding bridge preserves a scalar operation. Its expansion
    // is a separate, checked equality and is permitted only for Exact rings.
    if let Some(Proposition::IntegerMathLessOrEqual(left, right)) =
        proof_admission::lift_exact_integer_order(&premise.conclusion)
    {
        relation = replace_endpoint(context, relation, left, 0)?;
        relation = replace_endpoint(context, relation, right, 1)?;
    }
    let Proposition::IntegerMathLessOrEqual(left, right) = &relation.conclusion else {
        return None;
    };
    let addend = IntegerMathTerm::Subtract(Box::new(goal_left.clone()), Box::new(left.clone()));
    let translated_left = IntegerMathTerm::Add(Box::new(left.clone()), Box::new(addend.clone()));
    let translated_right = IntegerMathTerm::Add(Box::new(right.clone()), Box::new(addend.clone()));
    // Test the only non-tautological obligation before constructing the proof.
    // This also bounds every generated term before recursive kernel work.
    let upper = primitive(
        context,
        Proposition::IntegerMathLessOrEqual(translated_right.clone(), goal_right.clone()),
    )?;
    relation = ProofNode {
        conclusion: Proposition::IntegerMathLessOrEqual(translated_left, translated_right),
        rule: ProofRule::IntegerMathOrderTranslation {
            relation: Box::new(relation),
            addend,
        },
    };
    relation = replace_endpoint(context, relation, goal_left.clone(), 0)?;
    Some(ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::IntegerLessOrEqualTransitivity {
            left_less_or_equal_middle: Box::new(relation),
            middle_less_or_equal_right: Box::new(upper),
        },
    })
}

fn replace_endpoint(
    context: &PropositionContext,
    relation: ProofNode,
    replacement: IntegerMathTerm,
    endpoint: usize,
) -> Option<ProofNode> {
    let Proposition::IntegerMathLessOrEqual(left, right) = &relation.conclusion else {
        return None;
    };
    let original = if endpoint == 0 { left } else { right };
    if *original == replacement {
        return Some(relation);
    }
    let (equality_left, equality_right) = if original <= &replacement {
        (original.clone(), replacement.clone())
    } else {
        (replacement.clone(), original.clone())
    };
    let equality = primitive(
        context,
        Proposition::IntegerMathEqual(equality_left, equality_right),
    )?;
    let conclusion = if endpoint == 0 {
        Proposition::IntegerMathLessOrEqual(replacement, right.clone())
    } else {
        Proposition::IntegerMathLessOrEqual(left.clone(), replacement)
    };
    Some(ProofNode {
        conclusion,
        rule: ProofRule::IntegerOrderSubstitution {
            relation: Box::new(relation),
            equality: Box::new(equality),
            endpoint,
        },
    })
}

fn primitive(context: &PropositionContext, conclusion: Proposition) -> Option<ProofNode> {
    proof_admission::decide_primitive(
        context,
        &conclusion,
        PrimitiveJudgment::ClosedIntegerRelation,
    )
    .ok()?;
    Some(ProofNode {
        conclusion,
        rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{
        IntegerSign, IntegerType, IntegerValue, ScalarTerm, ScalarType, ValueId,
    };

    #[test]
    fn exact_range_rearrangement_retains_subject_policy_and_premise() {
        let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
        let context = PropositionContext::from_value_types((1..=3).map(|identity| {
            (
                ValueId::new(identity).unwrap(),
                ScalarType::Integer(integer),
            )
        }))
        .unwrap();
        let value = |identity| {
            ScalarTerm::value(
                ValueId::new(identity).unwrap(),
                ScalarType::Integer(integer),
            )
        };
        let maximum =
            ScalarTerm::integer(integer, IntegerValue::Unsigned(u64::MAX.into())).unwrap();
        let difference = ScalarTerm::ExactIntegerSubtract {
            scalar_type: integer,
            left: Box::new(maximum.clone()),
            right: Box::new(value(2)),
        };
        let premise = Proposition::LessOrEqual(value(1), difference);
        let goal = Proposition::IntegerMathLessOrEqual(
            IntegerMathTerm::Add(
                Box::new(IntegerMathTerm::embed(value(1)).unwrap()),
                Box::new(IntegerMathTerm::embed(value(2)).unwrap()),
            ),
            IntegerMathTerm::embed(maximum.clone()).unwrap(),
        );
        let proof = super::super::prove(&context, &goal, &[premise.clone()], 0).unwrap();
        proof_admission::check_certificate(&context, &goal, &[premise.clone()], &[], &proof)
            .unwrap();
        proof_admission::verify_bounded_certificate(
            &context,
            &goal,
            &[premise.clone()],
            &[],
            &proof,
            &mut proof_admission::Budget::default(),
        )
        .unwrap();
        assert!(proof_admission::check_certificate(&context, &goal, &[], &[], &proof).is_err());
        let wrong_subject = Proposition::LessOrEqual(
            value(3),
            match &premise {
                Proposition::LessOrEqual(_, right) => right.clone(),
                _ => unreachable!(),
            },
        );
        assert!(super::super::prove(&context, &goal, &[wrong_subject], 0).is_none());
        let wrapping = Proposition::LessOrEqual(
            value(1),
            ScalarTerm::WrappingIntegerSubtract {
                scalar_type: integer,
                left: Box::new(maximum),
                right: Box::new(value(2)),
            },
        );
        assert!(super::super::prove(&context, &goal, &[wrapping], 0).is_none());
    }
}
