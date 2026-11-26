use semantic_vocabulary::{IntegerMathTerm, Proposition, PropositionContext};
use terminal_psi::{PrimitiveJudgment, ProofNode, ProofRule};

pub(crate) fn derive(
    context: &PropositionContext,
    goal: &Proposition,
    premise: &ProofNode,
    assumptions: usize,
) -> Option<ProofNode> {
    if matches!(
        (&premise.conclusion, goal),
        (Proposition::LessOrEqual(..), Proposition::LessThan(..))
            | (Proposition::LessThan(..), Proposition::LessOrEqual(..))
    ) {
        let conversion = ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::IntegerOrderDiscreteness {
                relation: Box::new(ProofNode {
                    conclusion: premise.conclusion.clone(),
                    rule: ProofRule::Assumption { index: 0 },
                }),
            },
        };
        if proof_admission::check_certificate(
            context,
            goal,
            std::slice::from_ref(&premise.conclusion),
            &[],
            &conversion,
        )
        .is_ok()
        {
            return Some(ProofNode {
                conclusion: goal.clone(),
                rule: ProofRule::IntegerOrderDiscreteness {
                    relation: Box::new(premise.clone()),
                },
            });
        }
    }
    let Proposition::IntegerMathLessOrEqual(goal_left, goal_right) = goal else {
        return None;
    };
    proof_admission::check_predicate_evaluation_size([goal, &premise.conclusion])
        .ok()?;
    let mut relation = match &premise.conclusion {
        Proposition::IntegerMathLessOrEqual(..) => premise.clone(),
        Proposition::LessOrEqual(..) => proof_admission::restate_integer_relation(
            proof_admission::lift_fixed_integer_relation(&premise.conclusion)?,
            premise.clone(),
            assumptions,
        ),
        _ => return None,
    };
    if let Some(Proposition::IntegerMathLessOrEqual(left, right)) =
        proof_admission::lift_exact_integer_order(&premise.conclusion)
    {
        relation = replace_endpoint(context, relation, left, 0)?;
        relation = replace_endpoint(context, relation, right, 1)?;
    }
    let Proposition::IntegerMathLessOrEqual(left, right) = &relation.conclusion else {
        return None;
    };
    let addend =
        IntegerMathTerm::Subtract(Box::new(goal_left.clone()), Box::new(left.clone()));
    let translated_left =
        IntegerMathTerm::Add(Box::new(left.clone()), Box::new(addend.clone()));
    let translated_right =
        IntegerMathTerm::Add(Box::new(right.clone()), Box::new(addend.clone()));
    let upper = primitive(
        context,
        Proposition::IntegerMathLessOrEqual(
            translated_right.clone(),
            goal_right.clone(),
        ),
    )?;
    relation = ProofNode {
        conclusion: Proposition::IntegerMathLessOrEqual(
            translated_left,
            translated_right,
        ),
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

fn primitive(
    context: &PropositionContext,
    conclusion: Proposition,
) -> Option<ProofNode> {
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
mod tests;
