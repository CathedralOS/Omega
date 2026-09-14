//! Two cited integer bounds can establish a closed false comparison.

use proof_admission::{ProofNode, ProofRule};
use semantic_vocabulary::{IntegerCarrier, Proposition, ScalarTerm};

use super::super::super::integer_evidence::projected_facts;

pub(super) fn prove(
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    let facts = projected_facts(assumptions, semantic_axioms);
    for fact in &facts {
        if is_closed_false(fact.proposition) {
            return Some(falsehood(fact.proof()));
        }
    }
    // This is a bounded two-edge search, not interval propagation. Exhaustion
    // leaves the goal unproved and never changes the reconstructed question.
    let mut remaining_pairs = 4096usize;
    for lower in &facts {
        let Some((left, middle, lower_strict)) = order(lower.proposition) else {
            continue;
        };
        if !matches!(left, ScalarTerm::Integer { .. }) {
            continue;
        }
        for upper in &facts {
            remaining_pairs = remaining_pairs.checked_sub(1)?;
            let Some((other_middle, right, upper_strict)) = order(upper.proposition) else {
                continue;
            };
            if left.scalar_type() != middle.scalar_type()
                || middle.scalar_type() != other_middle.scalar_type()
            {
                continue;
            }
            let strict = lower_strict || upper_strict;
            let conclusion = if strict {
                Proposition::LessThan(left.clone(), right.clone())
            } else {
                Proposition::LessOrEqual(left.clone(), right.clone())
            };
            if !is_closed_false(&conclusion) {
                continue;
            }
            // Separate reads are separate values. Join them only through an
            // independently cited equality; transitivity still sees one exact
            // middle term after the explicit endpoint substitution.
            let upper_proof = if middle == other_middle {
                upper.proof()
            } else {
                let Some(equality) = super::super::exact::prove(
                    &Proposition::Equal(middle.clone(), other_middle.clone()),
                    assumptions,
                    semantic_axioms,
                ) else {
                    continue;
                };
                ProofNode {
                    conclusion: if upper_strict {
                        Proposition::LessThan(middle.clone(), right.clone())
                    } else {
                        Proposition::LessOrEqual(middle.clone(), right.clone())
                    },
                    rule: ProofRule::IntegerOrderSubstitution {
                        relation: Box::new(upper.proof()),
                        equality: Box::new(equality),
                        endpoint: 0,
                    },
                }
            };
            let rule = if strict {
                ProofRule::IntegerStrictOrderTransitivity {
                    left_to_middle: Box::new(lower.proof()),
                    middle_to_right: Box::new(upper_proof),
                }
            } else {
                ProofRule::IntegerLessOrEqualTransitivity {
                    left_less_or_equal_middle: Box::new(lower.proof()),
                    middle_less_or_equal_right: Box::new(upper_proof),
                }
            };
            return Some(falsehood(ProofNode { conclusion, rule }));
        }
    }
    None
}

fn order(proposition: &Proposition) -> Option<(&ScalarTerm, &ScalarTerm, bool)> {
    match proposition {
        Proposition::LessThan(left, right) => Some((left, right, true)),
        Proposition::LessOrEqual(left, right) => Some((left, right, false)),
        _ => None,
    }
}

fn is_closed_false(proposition: &Proposition) -> bool {
    let (left, right) = match proposition {
        Proposition::Equal(left, right)
        | Proposition::LessThan(left, right)
        | Proposition::LessOrEqual(left, right) => (left, right),
        _ => return false,
    };
    let (
        ScalarTerm::Integer {
            scalar_type: left_type,
            value: left,
        },
        ScalarTerm::Integer {
            scalar_type: right_type,
            value: right,
        },
    ) = (left, right)
    else {
        return false;
    };
    if left_type != right_type || left_type.carrier() != IntegerCarrier::Fixed {
        return false;
    }
    left_type
        .compare(*left, *right)
        .is_some_and(|ordering| match proposition {
            Proposition::Equal(_, _) => !ordering.is_eq(),
            Proposition::LessThan(_, _) => !ordering.is_lt(),
            Proposition::LessOrEqual(_, _) => ordering.is_gt(),
            _ => false,
        })
}

fn falsehood(premise: ProofNode) -> ProofNode {
    ProofNode {
        conclusion: Proposition::Falsehood,
        rule: ProofRule::PredicateDenotation {
            premise: Box::new(premise),
        },
    }
}

#[cfg(test)]
mod tests;
