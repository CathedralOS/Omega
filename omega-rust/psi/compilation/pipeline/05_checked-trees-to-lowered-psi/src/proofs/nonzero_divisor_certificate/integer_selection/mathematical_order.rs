//! Translate a scoped integer order and retain explicit endpoint equalities.
//!
//! A loop guarantee often compares an invocation expression with a result,
//! while its exit guard compares current values. A retained conservation
//! equality connects those namespaces. This producer cites that equality,
//! translates a cited order by one common mathematical addend, and proves
//! both endpoint identities by checked ring normalization. It never invents
//! a finite operation for an intermediate mathematical sum or difference.

use proof_admission::{
    PrimitiveJudgment, ProofNode, ProofRule, decide_primitive, lift_exact_integer_order,
    lift_fixed_integer_relation,
};
use semantic_vocabulary::{IntegerMathTerm, Proposition, PropositionContext};

use super::super::integer_evidence::projected_facts;
use super::super::search_context::spend;

#[cfg(test)]
mod tests;

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    if matches!(goal, Proposition::LessOrEqual(..)) {
        let mathematical = lift_exact_integer_order(goal)?;
        let relation = prove_mathematical(context, &mathematical, assumptions, semantic_axioms)?;
        let (_, left, _) = endpoints(&mathematical)?;
        // The existing order-substitution rule checks the exact fixed/math
        // projection. SSA expansion and every finite operation's overflow
        // obligation remain with their existing owners.
        return Some(ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::IntegerOrderSubstitution {
                relation: Box::new(relation),
                equality: Box::new(identity(context, left, left)?),
                endpoint: 0,
            },
        });
    }
    prove_mathematical(context, goal, assumptions, semantic_axioms)
}

fn prove_mathematical(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    let (strict, left, right) = endpoints(goal)?;
    // The current core translation evidence derives nonstrict order only.
    // A strict goal must use its own independently justified producer.
    if strict {
        return None;
    }
    let mut facts = Vec::new();
    for fact in projected_facts(assumptions, semantic_axioms) {
        spend(1)?;
        if matches!(
            fact.proposition,
            Proposition::LessOrEqual(..)
                | Proposition::LessThan(..)
                | Proposition::IntegerMathEqual(..)
                | Proposition::IntegerMathLessOrEqual(..)
                | Proposition::IntegerMathLessThan(..)
        ) {
            facts.extend(super::super::value_transport::citation_forms(
                context,
                fact.proof(),
                assumptions,
                semantic_axioms,
            ));
        }
    }
    if let Some(proof) = translate(context, strict, left, right, &facts) {
        return Some(proof);
    }
    // Search one retained endpoint equality at a time. Further SSA expansion
    // belongs to the shared ValueEqualityTransport owner, not this producer.
    for fact in &facts {
        spend(1)?;
        let Proposition::IntegerMathEqual(first, second) = &fact.conclusion else {
            continue;
        };
        for (old, replacement) in [(first, second), (second, first)] {
            let (endpoint, intermediate) = if old == left {
                (0, translate(context, strict, replacement, right, &facts))
            } else if old == right {
                (1, translate(context, strict, left, replacement, &facts))
            } else {
                continue;
            };
            if let Some(relation) = intermediate {
                return Some(ProofNode {
                    conclusion: goal.clone(),
                    rule: ProofRule::IntegerOrderSubstitution {
                        relation: Box::new(relation),
                        equality: Box::new(fact.clone()),
                        endpoint,
                    },
                });
            }
        }
    }
    None
}

fn endpoints(proposition: &Proposition) -> Option<(bool, &IntegerMathTerm, &IntegerMathTerm)> {
    match proposition {
        Proposition::IntegerMathLessThan(left, right) => Some((true, left, right)),
        Proposition::IntegerMathLessOrEqual(left, right) => Some((false, left, right)),
        _ => None,
    }
}

fn order(strict: bool, left: IntegerMathTerm, right: IntegerMathTerm) -> Proposition {
    if strict {
        Proposition::IntegerMathLessThan(left, right)
    } else {
        Proposition::IntegerMathLessOrEqual(left, right)
    }
}

fn translate(
    context: &PropositionContext,
    strict: bool,
    left: &IntegerMathTerm,
    right: &IntegerMathTerm,
    facts: &[ProofNode],
) -> Option<ProofNode> {
    for fact in facts {
        spend(1)?;
        let Some(premise) = mathematical_premise(fact, strict) else {
            continue;
        };
        let (_, premise_left, premise_right) = endpoints(&premise.conclusion)?;
        let addend =
            IntegerMathTerm::Subtract(Box::new(right.clone()), Box::new(premise_right.clone()));
        let translated_left =
            IntegerMathTerm::Add(Box::new(premise_left.clone()), Box::new(addend.clone()));
        let translated_right =
            IntegerMathTerm::Add(Box::new(premise_right.clone()), Box::new(addend.clone()));
        let Some(left_identity) = identity(context, &translated_left, left) else {
            continue;
        };
        let Some(right_identity) = identity(context, &translated_right, right) else {
            continue;
        };
        let translated = ProofNode {
            conclusion: order(strict, translated_left, translated_right.clone()),
            rule: ProofRule::IntegerMathOrderTranslation {
                relation: Box::new(premise),
                addend,
            },
        };
        let substituted_left = ProofNode {
            conclusion: order(strict, left.clone(), translated_right),
            rule: ProofRule::IntegerOrderSubstitution {
                relation: Box::new(translated),
                equality: Box::new(left_identity),
                endpoint: 0,
            },
        };
        return Some(ProofNode {
            conclusion: order(strict, left.clone(), right.clone()),
            rule: ProofRule::IntegerOrderSubstitution {
                relation: Box::new(substituted_left),
                equality: Box::new(right_identity),
                endpoint: 1,
            },
        });
    }
    None
}

fn mathematical_premise(fact: &ProofNode, strict: bool) -> Option<ProofNode> {
    let mut premise = fact.clone();
    if let Some((premise_strict, ..)) = endpoints(&premise.conclusion) {
        return (premise_strict == strict).then_some(premise);
    }
    let (premise_strict, left, right) = match &premise.conclusion {
        Proposition::LessThan(left, right) => (true, left, right),
        Proposition::LessOrEqual(left, right) => (false, left, right),
        _ => return None,
    };
    if strict && !premise_strict {
        return None;
    }
    let scalar_left = left.clone();
    if premise_strict && !strict {
        premise = ProofNode {
            conclusion: Proposition::LessOrEqual(left.clone(), right.clone()),
            rule: ProofRule::IntegerOrderWeakening {
                relation: Box::new(premise),
            },
        };
    }
    let conclusion = lift_fixed_integer_relation(&premise.conclusion)?;
    // Keep conjunction projections exact: lift through the existing checked
    // order rule rather than changing a projected child's conclusion.
    Some(ProofNode {
        conclusion,
        rule: ProofRule::IntegerOrderSubstitution {
            relation: Box::new(premise),
            equality: Box::new(ProofNode {
                conclusion: Proposition::Equal(scalar_left.clone(), scalar_left),
                rule: ProofRule::Primitive(PrimitiveJudgment::ReflexiveEquality),
            }),
            endpoint: 0,
        },
    })
}

fn identity(
    context: &PropositionContext,
    left: &IntegerMathTerm,
    right: &IntegerMathTerm,
) -> Option<ProofNode> {
    spend(1)?;
    let (left, right) = if left <= right {
        (left, right)
    } else {
        (right, left)
    };
    let conclusion = Proposition::IntegerMathEqual(left.clone(), right.clone());
    decide_primitive(
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
