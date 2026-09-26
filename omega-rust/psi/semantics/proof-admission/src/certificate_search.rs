//! Producer-side bounded certificate search over the denotation lanes.
//!
//! This is untrusted producer machinery, not a checker: it searches the same
//! bounded denotation-lane space the crash verifier accepts and emits
//! [`terminal_psi::CrashCertificate`] rosters naming which lane each node was
//! built under. Receivers never run this search — they replay the recorded
//! conversion through [`check_denotation_certificate`] and re-decide every
//! supplied node against goals they reconstructed themselves.
//!
//! Both the lowering producer and verification test fixtures share this
//! implementation so a certificate produced anywhere is accepted exactly when
//! the identical `(with_value_equalities, proof)` pair re-decides.

use crate::{
    CheckedPredicateDenotations, PredicateDenotationError, PrimitiveJudgment, ProofNode, ProofRule,
    check_predicate_denotations, check_predicate_denotations_with_value_equalities,
};
use semantic_vocabulary::{Proposition, PropositionContext};
use terminal_psi::CrashCertificate;

mod integer_order;
mod order_chain;

const MAXIMUM_SEARCH_STEPS: usize = 4096;
const MAXIMUM_PROOF_DEPTH: usize = 64;

/// One denotation conversion the producer can search under. The lane flag in
/// `CrashCertificate` selects the identical conversion at check time.
type DenotationConversion =
    for<'input> fn(
        &'input PropositionContext,
        &'input Proposition,
        &'input [Proposition],
        &'input [Proposition],
    ) -> Result<CheckedPredicateDenotations<'input>, PredicateDenotationError>;

/// Producer lanes in search order: the smaller Boolean-only question first,
/// then the same question under contextual value-equality transport.
const DENOTATION_LANES: [(bool, DenotationConversion); 2] = [
    (false, check_predicate_denotations),
    (true, check_predicate_denotations_with_value_equalities),
];

/// The producer stage of the crash ledger: bounded proof search over the
/// denotation lanes, recording which lane each emitted node was built under.
/// An empty supply is not a rejection verdict — the consumer's check of a
/// supplied node is what grants coverage, and no supply means none passes.
pub fn produce_denotation_certificates(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Vec<CrashCertificate> {
    DENOTATION_LANES
        .into_iter()
        .filter_map(|(with_value_equalities, convert)| {
            prove_lane(convert, context, goal, requirements, semantic_axioms).map(|proof| {
                CrashCertificate {
                    with_value_equalities,
                    proof,
                }
            })
        })
        .collect()
}

/// Check a supplied crash certificate without searching. The recorded
/// denotation lane is part of the certificate: a node produced under equality
/// transport is replayed against that conversion exactly as produced.
pub fn check_denotation_certificate(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
    certificate: &CrashCertificate,
) -> bool {
    let convert = if certificate.with_value_equalities {
        check_predicate_denotations_with_value_equalities
    } else {
        check_predicate_denotations
    };
    let Ok(denotations) = convert(context, goal, requirements, semantic_axioms) else {
        return false;
    };
    denotations
        .check_certificate(context, &certificate.proof)
        .is_ok()
}

/// Establish a goal from the supplied premises by producing then re-deciding
/// certificates. Test support for the search itself; production paths call
/// `produce_denotation_certificates` and let the receiver decide.
#[cfg(test)]
fn establishes(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
) -> bool {
    produce_denotation_certificates(context, goal, requirements, semantic_axioms)
        .iter()
        .any(|certificate| {
            check_denotation_certificate(context, goal, requirements, semantic_axioms, certificate)
        })
}

/// Run the bounded search under exactly one denotation conversion.
fn prove_lane(
    convert: DenotationConversion,
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    let denotations = convert(context, goal, requirements, semantic_axioms).ok()?;
    let mut remaining = MAXIMUM_SEARCH_STEPS;
    prove(
        denotations.goal(),
        denotations.requirements(),
        denotations.semantic_axioms(),
        &mut remaining,
        0,
    )
}

fn prove(
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
    remaining: &mut usize,
    depth: usize,
) -> Option<ProofNode> {
    step(remaining, depth)?;
    for (premises, semantic) in [(requirements, false), (semantic_axioms, true)] {
        for (index, premise) in premises.iter().enumerate() {
            let mut path = Vec::new();
            let mut reversed = false;
            let found = projection(premise, goal, &mut path, remaining, depth)?;
            let found = if !found && let Proposition::Equal(left, right) = goal {
                path.clear();
                reversed = true;
                projection(
                    premise,
                    &Proposition::Equal(right.clone(), left.clone()),
                    &mut path,
                    remaining,
                    depth,
                )?
            } else {
                found
            };
            if !found {
                continue;
            }
            let mut proof = ProofNode {
                conclusion: premise.clone(),
                rule: if semantic {
                    ProofRule::SemanticAxiom { index }
                } else {
                    ProofRule::Assumption { index }
                },
            };
            let mut current = premise;
            for conjunct in path {
                let Proposition::Conjunction(children) = current else {
                    return None;
                };
                current = children.get(conjunct)?;
                proof = ProofNode {
                    conclusion: current.clone(),
                    rule: ProofRule::ConjunctionElimination {
                        conjunction: Box::new(proof),
                        conjunct,
                    },
                };
            }
            return Some(if reversed {
                ProofNode {
                    conclusion: goal.clone(),
                    rule: ProofRule::EqualitySymmetry {
                        equality: Box::new(proof),
                    },
                }
            } else {
                proof
            });
        }
    }
    for (premises, semantic) in [(requirements, false), (semantic_axioms, true)] {
        for (index, premise) in premises.iter().enumerate() {
            let premise = ProofNode {
                conclusion: premise.clone(),
                rule: if semantic {
                    ProofRule::SemanticAxiom { index }
                } else {
                    ProofRule::Assumption { index }
                },
            };
            if let Some(proof) = common_consequence(
                goal,
                premise,
                requirements.len(),
                !semantic,
                remaining,
                depth + 1,
            ) {
                return Some(proof);
            }
        }
    }
    if let Some(proof) = order_chain::prove(goal, requirements, semantic_axioms, remaining, depth) {
        return Some(proof);
    }
    let rule = match goal {
        Proposition::Truth => ProofRule::Primitive(PrimitiveJudgment::Truth),
        Proposition::Equal(left, right) if left == right => {
            ProofRule::Primitive(PrimitiveJudgment::ReflexiveEquality)
        }
        Proposition::Conjunction(children) => ProofRule::ConjunctionIntroduction(
            children
                .iter()
                .map(|child| prove(child, requirements, semantic_axioms, remaining, depth + 1))
                .collect::<Option<Vec<_>>>()?,
        ),
        Proposition::Disjunction(children) => {
            let (index, proof) = children.iter().enumerate().find_map(|(index, child)| {
                prove(child, requirements, semantic_axioms, remaining, depth + 1)
                    .map(|proof| (index, proof))
            })?;
            ProofRule::DisjunctionIntroduction {
                disjunct: Box::new(proof),
                index,
            }
        }
        _ => return None,
    };
    Some(ProofNode {
        conclusion: goal.clone(),
        rule,
    })
}

/// Eliminate a disjunction only when every alternative proves the same goal.
/// Branch assumptions occupy the kernel's next local slot and cannot escape
/// into siblings or the enclosing entry requirement context.
fn common_consequence(
    goal: &Proposition,
    premise: ProofNode,
    assumption_count: usize,
    invocation_entry: bool,
    remaining: &mut usize,
    depth: usize,
) -> Option<ProofNode> {
    step(remaining, depth)?;
    if &premise.conclusion == goal {
        return Some(premise);
    }
    if let (Proposition::Equal(left, right), Proposition::Equal(other_left, other_right)) =
        (goal, &premise.conclusion)
        && left == other_right
        && right == other_left
    {
        return Some(ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::EqualitySymmetry {
                equality: Box::new(premise),
            },
        });
    }
    if invocation_entry && let Some(proof) = integer_order::from_premise(goal, &premise) {
        return Some(proof);
    }
    // A branch may prove a published union by proving one of its alternatives.
    // This happens inside its local assumption scope; eliminating the source
    // disjunction below still requires a certificate for every source branch.
    if let Proposition::Disjunction(children) = goal {
        for (index, child) in children.iter().enumerate() {
            if let Some(proof) = common_consequence(
                child,
                premise.clone(),
                assumption_count,
                invocation_entry,
                remaining,
                depth + 1,
            ) {
                return Some(ProofNode {
                    conclusion: goal.clone(),
                    rule: ProofRule::DisjunctionIntroduction {
                        disjunct: Box::new(proof),
                        index,
                    },
                });
            }
        }
    }
    match &premise.conclusion {
        Proposition::Conjunction(children) => {
            for (conjunct, child) in children.iter().enumerate() {
                let child = ProofNode {
                    conclusion: child.clone(),
                    rule: ProofRule::ConjunctionElimination {
                        conjunction: Box::new(premise.clone()),
                        conjunct,
                    },
                };
                if let Some(proof) = common_consequence(
                    goal,
                    child,
                    assumption_count,
                    invocation_entry,
                    remaining,
                    depth + 1,
                ) {
                    return Some(proof);
                }
            }
            None
        }
        Proposition::Disjunction(children) => {
            let mut branches = Vec::new();
            for child in children {
                branches.push(common_consequence(
                    goal,
                    ProofNode {
                        conclusion: child.clone(),
                        rule: ProofRule::Assumption {
                            index: assumption_count,
                        },
                    },
                    assumption_count + 1,
                    invocation_entry,
                    remaining,
                    depth + 1,
                )?);
            }
            Some(ProofNode {
                conclusion: goal.clone(),
                rule: ProofRule::DisjunctionElimination {
                    disjunction: Box::new(premise),
                    branches,
                },
            })
        }
        _ => None,
    }
}

fn step(remaining: &mut usize, depth: usize) -> Option<()> {
    if depth >= MAXIMUM_PROOF_DEPTH {
        return None;
    }
    *remaining = remaining.checked_sub(1)?;
    Some(())
}

fn projection(
    premise: &Proposition,
    goal: &Proposition,
    path: &mut Vec<usize>,
    remaining: &mut usize,
    depth: usize,
) -> Option<bool> {
    step(remaining, depth)?;
    if premise == goal {
        return Some(true);
    }
    if let Proposition::Conjunction(conjuncts) = premise {
        for (index, conjunct) in conjuncts.iter().enumerate() {
            path.push(index);
            if projection(conjunct, goal, path, remaining, depth + 1)? {
                return Some(true);
            }
            path.pop();
        }
    }
    Some(false)
}

#[cfg(test)]
mod tests {
    use super::{
        MAXIMUM_PROOF_DEPTH, MAXIMUM_SEARCH_STEPS, check_denotation_certificate,
        common_consequence, establishes, produce_denotation_certificates, prove,
    };
    use crate::{ProofNode, ProofRule, check_certificate};
    use semantic_vocabulary::{Proposition, PropositionContext};

    #[test]
    fn conjunction_proof_search_exhausts_a_shared_budget() {
        let goal = Proposition::Truth;
        let requirements = [Proposition::Conjunction(vec![
            Proposition::Falsehood,
            goal.clone(),
        ])];
        let mut remaining = 2;
        assert!(prove(&goal, &requirements, &[], &mut remaining, 0).is_none());
        assert_eq!(remaining, 0);
        let mut remaining = MAXIMUM_SEARCH_STEPS;
        let proof = prove(&goal, &requirements, &[], &mut remaining, 0).unwrap();
        check_certificate(
            &PropositionContext::default(),
            &goal,
            &requirements,
            &[],
            &proof,
        )
        .unwrap();
    }

    #[test]
    fn disjunction_is_not_projected_as_a_conjunction() {
        let flag = semantic_vocabulary::ScalarTerm::value(
            semantic_vocabulary::ValueId::new(1).unwrap(),
            semantic_vocabulary::ScalarType::Boolean,
        );
        let goal = Proposition::Equal(flag.clone(), semantic_vocabulary::ScalarTerm::boolean(true));
        let requirements = [Proposition::Disjunction(vec![
            goal.clone(),
            Proposition::Equal(flag, semantic_vocabulary::ScalarTerm::boolean(false)),
        ])];
        let mut remaining = MAXIMUM_SEARCH_STEPS;
        assert!(prove(&goal, &requirements, &[], &mut remaining, 0).is_none());
    }

    #[test]
    fn supplied_certificates_are_redecided_not_trusted() {
        use semantic_vocabulary::{ScalarTerm, ScalarType, ValueId};
        let identity = ValueId::new(1).unwrap();
        let context =
            PropositionContext::from_value_types([(identity, ScalarType::Boolean)]).unwrap();
        let goal = Proposition::Equal(
            ScalarTerm::value(identity, ScalarType::Boolean),
            ScalarTerm::boolean(true),
        );
        let requirements = [Proposition::Conjunction(vec![
            Proposition::Truth,
            goal.clone(),
        ])];
        let certificates = produce_denotation_certificates(&context, &goal, &requirements, &[]);
        assert!(!certificates.is_empty());
        // Every supplied node is replayed through the recorded denotation
        // conversion and the kernel check; supply alone grants nothing.
        assert!(certificates.iter().all(|certificate| {
            check_denotation_certificate(&context, &goal, &requirements, &[], certificate)
        }));
        // A node produced for this goal is not evidence for another question.
        let other = Proposition::Equal(
            ScalarTerm::value(identity, ScalarType::Boolean),
            ScalarTerm::boolean(false),
        );
        assert!(!certificates.iter().any(|certificate| {
            check_denotation_certificate(&context, &other, &requirements, &[], certificate)
        }));
    }

    #[test]
    fn an_empty_supply_establishes_nothing() {
        use semantic_vocabulary::{ScalarTerm, ScalarType, ValueId};
        let goal = Proposition::Equal(
            ScalarTerm::value(ValueId::new(1).unwrap(), ScalarType::Boolean),
            ScalarTerm::boolean(true),
        );
        let context =
            PropositionContext::from_value_types([(ValueId::new(1).unwrap(), ScalarType::Boolean)])
                .unwrap();
        assert!(produce_denotation_certificates(&context, &goal, &[], &[]).is_empty());
        assert!(!establishes(&context, &goal, &[], &[]));
    }

    #[test]
    fn projection_depth_is_bounded_even_with_remaining_steps() {
        let goal = Proposition::Truth;
        let mut premise = goal.clone();
        for _ in 0..MAXIMUM_PROOF_DEPTH {
            premise = Proposition::Conjunction(vec![premise]);
        }
        let mut remaining = MAXIMUM_SEARCH_STEPS;
        assert!(prove(&goal, &[premise], &[], &mut remaining, 0).is_none());
        assert!(remaining > 0);
    }

    #[test]
    fn nested_cases_discharge_local_assumptions_for_requirements_and_axioms() {
        use semantic_vocabulary::{ScalarTerm, ScalarType, ValueId};
        let identifiers = [1, 2, 3].map(|index| ValueId::new(index).unwrap());
        let context = PropositionContext::from_value_types(
            identifiers.map(|identity| (identity, ScalarType::Boolean)),
        )
        .unwrap();
        let [goal, other, third] = identifiers.map(|identity| {
            Proposition::Equal(
                ScalarTerm::value(identity, ScalarType::Boolean),
                ScalarTerm::boolean(true),
            )
        });
        let cases = Proposition::Disjunction(vec![
            Proposition::Conjunction(vec![goal.clone(), other.clone()]),
            Proposition::Conjunction(vec![
                third.clone(),
                Proposition::Disjunction(vec![
                    goal.clone(),
                    Proposition::Conjunction(vec![other.clone(), goal.clone()]),
                ]),
            ]),
        ]);
        for semantic in [false, true] {
            let mut requirements = vec![other.clone()];
            let mut axioms = Vec::new();
            if semantic {
                axioms.push(cases.clone());
            } else {
                requirements.push(cases.clone());
            }
            assert!(establishes(&context, &goal, &requirements, &axioms));
            assert!(!establishes(&context, &third, &requirements, &axioms));
            let mut remaining = 3;
            let premise = ProofNode {
                conclusion: cases.clone(),
                rule: if semantic {
                    ProofRule::SemanticAxiom { index: 0 }
                } else {
                    ProofRule::Assumption { index: 1 }
                },
            };
            assert!(
                common_consequence(
                    &goal,
                    premise.clone(),
                    requirements.len(),
                    !semantic,
                    &mut remaining,
                    0
                )
                .is_none()
            );
            assert_eq!(remaining, 0);
            let mut remaining = MAXIMUM_SEARCH_STEPS;
            let proof = common_consequence(
                &goal,
                premise,
                requirements.len(),
                !semantic,
                &mut remaining,
                0,
            )
            .unwrap();
            check_certificate(&context, &goal, &requirements, &axioms, &proof).unwrap();
        }
        let leaking_cases = Proposition::Disjunction(vec![
            Proposition::Conjunction(vec![goal.clone(), other.clone()]),
            other,
        ]);
        assert!(!establishes(&context, &goal, &[leaking_cases], &[]));
    }
}
