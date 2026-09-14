//! Two cited integer bounds can establish a closed false comparison.
//!
//! A bound also arrives as an exact equality: a store's `field == 9` fact or a
//! computed `value == literal` equation is a pair of nonstrict bounds after
//! checked weakening, never a new rule. Equalities joined through the existing
//! equality bridge and closed arithmetic endpoints rewritten by the closed
//! relation primitive keep every contradiction inside the same two-leg search.

use proof_admission::{PrimitiveJudgment, ProofNode, ProofRule};
use semantic_vocabulary::{IntegerCarrier, Proposition, ScalarTerm, ScalarType};

use super::super::super::integer_evidence::projected_facts;

/// One checked `left </<= right` bound with its proof. Legs are built from
/// citations only; a leg's proof is what the kernel replays, so a derived leg
/// can never invent a bound the original citation did not carry.
struct OrderLeg {
    left: ScalarTerm,
    right: ScalarTerm,
    strict: bool,
    proof: ProofNode,
}

pub(super) fn prove(
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    let facts = projected_facts(assumptions, semantic_axioms);
    let mut legs = Vec::new();
    for fact in &facts {
        match fact.proposition {
            Proposition::LessThan(left, right) => {
                legs.push(leg(left.clone(), right.clone(), true, fact.proof()));
            }
            Proposition::LessOrEqual(left, right) => {
                legs.push(leg(left.clone(), right.clone(), false, fact.proof()));
            }
            Proposition::Equal(left, right) => {
                // Only integer equalities weaken to order legs; the kernel
                // refuses a Boolean or mixed-type weakening premise. Each
                // direction keeps its own checked node so the leg carries the
                // exact citation either way.
                if !matches!(left.scalar_type(), ScalarType::Integer(_))
                    || left.scalar_type() != right.scalar_type()
                {
                    continue;
                }
                let equality = fact.proof();
                legs.push(leg(
                    left.clone(),
                    right.clone(),
                    false,
                    ProofNode {
                        conclusion: Proposition::LessOrEqual(left.clone(), right.clone()),
                        rule: ProofRule::IntegerOrderWeakening {
                            relation: Box::new(equality.clone()),
                        },
                    },
                ));
                legs.push(leg(
                    right.clone(),
                    left.clone(),
                    false,
                    ProofNode {
                        conclusion: Proposition::LessOrEqual(right.clone(), left.clone()),
                        rule: ProofRule::IntegerOrderWeakening {
                            relation: Box::new(ProofNode {
                                conclusion: Proposition::Equal(right.clone(), left.clone()),
                                rule: ProofRule::EqualitySymmetry {
                                    equality: Box::new(equality),
                                },
                            }),
                        },
                    },
                ));
            }
            _ => {}
        }
    }
    for leg in &legs {
        if is_closed_false(&relation(leg)) {
            return Some(falsehood(leg.proof.clone()));
        }
    }
    // This is a bounded two-edge search, not interval propagation. Exhaustion
    // leaves the goal unproved and never changes the reconstructed question.
    let mut remaining_pairs = 4096usize;
    for lower in &legs {
        if !matches!(lower.left, ScalarTerm::Integer { .. }) {
            continue;
        }
        for upper in &legs {
            remaining_pairs = remaining_pairs.checked_sub(1)?;
            if lower.left.scalar_type() != lower.right.scalar_type()
                || lower.right.scalar_type() != upper.left.scalar_type()
            {
                continue;
            }
            let strict = lower.strict || upper.strict;
            let conclusion = if strict {
                Proposition::LessThan(lower.left.clone(), upper.right.clone())
            } else {
                Proposition::LessOrEqual(lower.left.clone(), upper.right.clone())
            };
            if !is_closed_false(&conclusion) {
                continue;
            }
            // Separate reads are separate values. Join them only through an
            // independently cited equality; transitivity still sees one exact
            // middle term after the explicit endpoint substitution.
            let upper_proof = if lower.right == upper.left {
                upper.proof.clone()
            } else {
                let Some(equality) = super::super::exact::prove(
                    &Proposition::Equal(lower.right.clone(), upper.left.clone()),
                    assumptions,
                    semantic_axioms,
                ) else {
                    continue;
                };
                ProofNode {
                    conclusion: if upper.strict {
                        Proposition::LessThan(lower.right.clone(), upper.right.clone())
                    } else {
                        Proposition::LessOrEqual(lower.right.clone(), upper.right.clone())
                    },
                    rule: ProofRule::IntegerOrderSubstitution {
                        relation: Box::new(upper.proof.clone()),
                        equality: Box::new(equality),
                        endpoint: 0,
                    },
                }
            };
            let rule = if strict {
                ProofRule::IntegerStrictOrderTransitivity {
                    left_to_middle: Box::new(lower.proof.clone()),
                    middle_to_right: Box::new(upper_proof),
                }
            } else {
                ProofRule::IntegerLessOrEqualTransitivity {
                    left_less_or_equal_middle: Box::new(lower.proof.clone()),
                    middle_less_or_equal_right: Box::new(upper_proof),
                }
            };
            return Some(falsehood(ProofNode { conclusion, rule }));
        }
    }
    None
}

fn relation(leg: &OrderLeg) -> Proposition {
    if leg.strict {
        Proposition::LessThan(leg.left.clone(), leg.right.clone())
    } else {
        Proposition::LessOrEqual(leg.left.clone(), leg.right.clone())
    }
}

/// Build one leg, then rewrite each closed endpoint to its evaluated literal
/// through the checked closed-relation primitive and endpoint substitution.
/// Open terms keep their exact identity; only a term whose own fixed-integer
/// denotation is a literal gains the literal's closed-comparison route.
fn leg(left: ScalarTerm, right: ScalarTerm, strict: bool, proof: ProofNode) -> OrderLeg {
    let (left, proof) = literal_endpoint(left, &right, strict, proof, 0);
    let (right, proof) = literal_endpoint(right, &left, strict, proof, 1);
    OrderLeg {
        left,
        right,
        strict,
        proof,
    }
}

fn literal_endpoint(
    term: ScalarTerm,
    other: &ScalarTerm,
    strict: bool,
    proof: ProofNode,
    endpoint: usize,
) -> (ScalarTerm, ProofNode) {
    let Some((integer_type, value)) = term.integer_value() else {
        return (term, proof);
    };
    let Ok(literal) = ScalarTerm::integer(integer_type, value) else {
        return (term, proof);
    };
    if literal == term {
        return (term, proof);
    }
    let equality = ProofNode {
        conclusion: Proposition::Equal(term.clone(), literal.clone()),
        rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
    };
    let (left, right) = if endpoint == 0 {
        (literal.clone(), other.clone())
    } else {
        (other.clone(), literal.clone())
    };
    let conclusion = if strict {
        Proposition::LessThan(left, right)
    } else {
        Proposition::LessOrEqual(left, right)
    };
    (
        literal,
        ProofNode {
            conclusion,
            rule: ProofRule::IntegerOrderSubstitution {
                relation: Box::new(proof),
                equality: Box::new(equality),
                endpoint,
            },
        },
    )
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
