//! Selected Boolean polarity reconstructed from prior terminal equations.

use proof_admission::{ProofNode, ProofRule, check_certificate};
use semantic_vocabulary::{Proposition, PropositionContext, ScalarTerm, ScalarType, ValueId};
use std::collections::HashSet;

#[cfg(test)]
mod tests;

/// One reconstructed path-condition fact tagged by how the generator
/// discharged it.
pub(in super::super) struct ConditionFact {
    /// The reconstructed proposition appended to the selected successor's
    /// axiom roster.
    pub proposition: Proposition,
    /// `true` when a fixed-shape `ValueEqualityTransport` certificate —
    /// the selected arm's truth premise transported through the roster's
    /// own value equations — was re-decided by the certificate checker
    /// before this fact was emitted. `false` marks a licensed premise
    /// introduction recorded under `fact:branch-condition`: boundary arm
    /// truths the transport cannot represent, the literal-adjacency
    /// disequality strengthening, a closed unsatisfiable arm's falsehood,
    /// and any emission whose fixed-shape certificate was rejected.
    ///
    /// The generation-time check runs in production regardless; only the
    /// test module reads the classification back. The certificate
    /// machinery is exercised against every reconstructed arm fact either
    /// way.
    #[allow(dead_code)]
    pub certified: bool,
}

pub(in super::super) fn condition_fact(
    condition: ValueId,
    positive: bool,
    axioms: &[Proposition],
    value_term: &impl Fn(ValueId) -> ScalarTerm,
    context: &PropositionContext,
) -> Option<ConditionFact> {
    condition_proposition(condition, positive, axioms, value_term).map(|proposition| {
        ConditionFact {
            certified: transport_certified(
                condition,
                positive,
                &proposition,
                axioms,
                value_term,
                context,
            ),
            proposition,
        }
    })
}

/// Re-decide a fixed-shape transport certificate for the reconstructed fact
/// before it is classified. The selected arm's truth premise —
/// `condition == Boolean(positive)` — is assumption zero, and the roster
/// equations the transport can reach are cited as semantic axioms in
/// newest-first order so the transport's first-match equation discipline
/// reproduces the walk's `.rev()` lookup. A certificate the checker rejects
/// leaves the emission under `fact:branch-condition`'s licensed premise
/// introductions rather than failing the module.
fn transport_certified(
    condition: ValueId,
    positive: bool,
    proposition: &Proposition,
    axioms: &[Proposition],
    value_term: &impl Fn(ValueId) -> ScalarTerm,
    context: &PropositionContext,
) -> bool {
    let premise = Proposition::Equal(value_term(condition), ScalarTerm::Boolean(positive));
    let equalities = reachable_axiom_equalities(axioms, [&premise, proposition])
        .into_iter()
        .map(|(index, axiom)| ProofNode {
            conclusion: axiom.clone(),
            rule: ProofRule::SemanticAxiom { index },
        })
        .collect::<Vec<_>>();
    let certificate = ProofNode {
        conclusion: proposition.clone(),
        rule: ProofRule::ValueEqualityTransport {
            premise: Box::new(ProofNode {
                conclusion: premise.clone(),
                rule: ProofRule::Assumption { index: 0 },
            }),
            equalities,
        },
    };
    check_certificate(
        context,
        proposition,
        std::slice::from_ref(&premise),
        axioms,
        &certificate,
    )
    .is_ok()
}

/// The `Equal(Value, _)` axiom rows a value-equality transport can consult
/// for this premise/conclusion pair, in the roster's newest-first order with
/// the original `SemanticAxiom` indices. A cited equation participates only
/// when one of its value identities is reachable from the walked terms —
/// seeded by the premise and conclusion and closed under the cited
/// equations' own values — so unreachable rows cannot influence the checked
/// relation and are not cloned into the certificate. A term whose value
/// identities cannot be completely walked keeps the whole roster cited.
fn reachable_axiom_equalities<'a>(
    axioms: &'a [Proposition],
    roots: [&'a Proposition; 2],
) -> Vec<(usize, &'a Proposition)> {
    let roster_row = |(_, axiom): &(usize, &'a Proposition)| matches!(axiom, Proposition::Equal(left @ ScalarTerm::Value { .. }, right) if left != right);
    let mut reachable = HashSet::new();
    let mut complete = roots.iter().all(|proposition| {
        proposition.visit_value_ids(|id| {
            reachable.insert(id);
        })
    });
    let mut cited = vec![false; axioms.len()];
    let mut extended = true;
    while complete && extended {
        extended = false;
        for (index, axiom) in axioms.iter().enumerate() {
            if cited[index] || !roster_row(&(index, axiom)) {
                continue;
            }
            let mut touches = false;
            complete &= axiom.visit_value_ids(|id| {
                touches |= reachable.contains(&id);
            });
            if !complete {
                break;
            }
            if !touches {
                continue;
            }
            cited[index] = true;
            extended = true;
            axiom.visit_value_ids(|id| {
                reachable.insert(id);
            });
        }
    }
    axioms
        .iter()
        .enumerate()
        .rev()
        .filter(|row| (!complete || cited[row.0]) && roster_row(row))
        .collect()
}

fn condition_proposition(
    condition: ValueId,
    mut positive: bool,
    axioms: &[Proposition],
    value_term: &impl Fn(ValueId) -> ScalarTerm,
) -> Option<Proposition> {
    let mut predicate = value_term(condition);
    let mut expanded = Vec::new();
    loop {
        if predicate.scalar_type() != ScalarType::Boolean {
            return None;
        }
        match predicate {
            ScalarTerm::Value { id, .. } => {
                if expanded.contains(&id) {
                    return None;
                }
                expanded.push(id);
                if let Some(definition) = defining_term(&predicate, axioms) {
                    if matches!(definition, ScalarTerm::Value { id, .. } if expanded.contains(id))
                        && !axioms.iter().any(|axiom| {
                            matches!(axiom, Proposition::Equal(left, right)
                                if left == &predicate && right != &predicate)
                        })
                    {
                        // A formal may occur only on the right of a retained
                        // edge alias. Walking that equality backwards would
                        // return to the condition we just expanded. Keep the
                        // exact boundary value's selected truth instead; a
                        // genuine cycle of forward definitions still rejects.
                        return Some(Proposition::Equal(predicate, ScalarTerm::Boolean(positive)));
                    }
                    predicate = definition.clone();
                } else {
                    return Some(Proposition::Equal(predicate, ScalarTerm::Boolean(positive)));
                }
            }
            ScalarTerm::Boolean(value) => {
                return Some(if value == positive {
                    Proposition::Truth
                } else {
                    Proposition::Falsehood
                });
            }
            ScalarTerm::BooleanNot { operand } => {
                predicate = *operand;
                positive = !positive;
            }
            ScalarTerm::BooleanEqual { left, right } => {
                let left = constant_or_original(*left, axioms);
                let right = constant_or_original(*right, axioms);
                match (left, right) {
                    (ScalarTerm::Boolean(value), other) | (other, ScalarTerm::Boolean(value)) => {
                        predicate = other;
                        positive = positive == value;
                    }
                    (left, right) => {
                        return Some(Proposition::Equal(
                            if positive {
                                left
                            } else {
                                ScalarTerm::boolean_not(left).ok()?
                            },
                            right,
                        ));
                    }
                }
            }
            ScalarTerm::IntegerEqual { left, right, .. } => {
                let left = constant_or_original(*left, axioms);
                let right = constant_or_original(*right, axioms);
                return Some(if positive {
                    Proposition::Equal(left, right)
                } else {
                    super::discrete::unequal(left, right)
                });
            }
            ScalarTerm::IntegerLessThan { left, right, .. } => {
                let left = constant_or_original(*left, axioms);
                let right = constant_or_original(*right, axioms);
                return Some(if positive {
                    Proposition::LessThan(left, right)
                } else {
                    Proposition::LessOrEqual(right, left)
                });
            }
            ScalarTerm::IntegerLessOrEqual { left, right, .. } => {
                let left = constant_or_original(*left, axioms);
                let right = constant_or_original(*right, axioms);
                return Some(if positive {
                    Proposition::LessOrEqual(left, right)
                } else {
                    Proposition::LessThan(right, left)
                });
            }
            // No integer order law is applied to an unrelated Boolean
            // observation. Its selected truth value remains an exact fact.
            other => return Some(Proposition::Equal(other, ScalarTerm::Boolean(positive))),
        }
    }
}

fn defining_term<'a>(term: &ScalarTerm, axioms: &'a [Proposition]) -> Option<&'a ScalarTerm> {
    // Operation and edge equations put their newly defined value on the left.
    // Prefer that definition over a later alias which mentions it on the right;
    // otherwise a forward alias immediately walks back to its predecessor.
    axioms
        .iter()
        .rev()
        .find_map(|axiom| match axiom {
            Proposition::Equal(left, right) if left == term && right != term => Some(right),
            _ => None,
        })
        .or_else(|| {
            axioms.iter().rev().find_map(|axiom| match axiom {
                Proposition::Equal(left, right) if right == term && left != term => Some(left),
                _ => None,
            })
        })
}

fn constant_or_original(term: ScalarTerm, axioms: &[Proposition]) -> ScalarTerm {
    let mut current = &term;
    let mut expanded = Vec::new();
    while let ScalarTerm::Value { id, .. } = current {
        if expanded.contains(id) {
            return term;
        }
        expanded.push(*id);
        let Some(definition) = defining_term(current, axioms) else {
            return term;
        };
        current = definition;
    }
    match current {
        ScalarTerm::Boolean(_) | ScalarTerm::Integer { .. } => current.clone(),
        _ => term,
    }
}
