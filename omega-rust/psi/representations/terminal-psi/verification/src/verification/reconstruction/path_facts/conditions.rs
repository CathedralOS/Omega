//! Selected Boolean polarity reconstructed from prior terminal equations.

use proof_admission::{ProofNode, ProofRule, check_certificate};
use semantic_vocabulary::{Proposition, PropositionContext, ScalarTerm, ScalarType, ValueId};
use std::collections::{HashMap, HashSet};

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
/// reproduces the walk's `.rev()` lookup. The checker receives only those
/// cited rows (see [`super::CitedRoster`]). A certificate the checker rejects
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
    let cited = reachable_axiom_equalities(axioms, [&premise, proposition])
        .into_iter()
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    transport_accepted(&premise, proposition, axioms, &cited, context)
}

/// Check the fixed-shape transport certificate citing roster rows `cited`
/// in the given order, against only those rows.
fn transport_accepted(
    premise: &Proposition,
    proposition: &Proposition,
    axioms: &[Proposition],
    cited: &[usize],
    context: &PropositionContext,
) -> bool {
    let Some(roster) = super::CitedRoster::new(axioms, cited.iter().copied()) else {
        return false;
    };
    let equalities = cited
        .iter()
        .map(|&index| roster.citation(index))
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
        std::slice::from_ref(premise),
        &roster.rows,
        &certificate,
    )
    .is_ok()
}

/// The `Equal(Value, _)` axiom rows a value-equality transport can consult
/// for this premise/conclusion pair, in the roster's newest-first order with
/// the original `SemanticAxiom` indices.
///
/// The transport rewrites a value only through an equation whose left side
/// is that value, then continues through the substituted right side. A row
/// is therefore cited when its defined value is reachable: seeded by the
/// premise's and conclusion's value identities and closed under the right
/// sides of the rows already cited. Every row defining a reachable value is
/// cited, not only the newest, so the checker's own first-equation choice is
/// unchanged; rows whose defined value is unreachable cannot influence the
/// checked relation and are not cloned into the certificate. Structural field
/// leaves carry no value identity and are never rewritten.
///
/// When no reachable value has a defining row, the transport rewrites
/// nothing, but the rule still needs one cited equation. The newest row
/// mentioning a premise or conclusion value then stands in without rewriting
/// anything; with no such row nothing is cited and the certificate is
/// rejected.
fn reachable_axiom_equalities<'a>(
    axioms: &'a [Proposition],
    roots: [&'a Proposition; 2],
) -> Vec<(usize, &'a Proposition)> {
    let mut definitions = HashMap::<ValueId, Vec<usize>>::new();
    for (index, axiom) in axioms.iter().enumerate() {
        if let Proposition::Equal(left @ ScalarTerm::Value { id, .. }, right) = axiom
            && left != right
        {
            definitions.entry(*id).or_default().push(index);
        }
    }
    let mut reachable = HashSet::new();
    let mut pending = Vec::new();
    for root in roots {
        root.visit_value_ids(|id| {
            if reachable.insert(id) {
                pending.push(id);
            }
        });
    }
    let mut cited = vec![false; axioms.len()];
    let mut any_cited = false;
    while let Some(value) = pending.pop() {
        for &index in definitions.get(&value).into_iter().flatten() {
            if cited[index] {
                continue;
            }
            cited[index] = true;
            any_cited = true;
            axioms[index].visit_value_ids(|id| {
                if reachable.insert(id) {
                    pending.push(id);
                }
            });
        }
    }
    if !any_cited {
        // Nothing was cited, so `reachable` still holds exactly the root
        // identities.
        if let Some(index) = definitions
            .values()
            .flatten()
            .copied()
            .filter(|&index| axioms[index].any_value_id(|id| reachable.contains(&id)))
            .max()
        {
            cited[index] = true;
        }
    }
    axioms
        .iter()
        .enumerate()
        .rev()
        .filter(|(index, _)| cited[*index])
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
