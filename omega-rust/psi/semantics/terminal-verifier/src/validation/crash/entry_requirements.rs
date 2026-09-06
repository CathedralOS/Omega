//! A sufficient caller ceiling proved solely from invocation-entry requirements.
//! Callee continuations retain their independently reconstructed exact routes.

use proof_admission::{PrimitiveJudgment, ProofNode, ProofRule, check_predicate_denotations};
use semantic_vocabulary::{Proposition, PropositionContext, StructuralPlaceKind};
use terminal_psi::{CrashRouteBucket, CrashRouteGuard, TerminalMachine};

const MAXIMUM_SEARCH_STEPS: usize = 4096;
const MAXIMUM_PROOF_DEPTH: usize = 64;

/// Establish a crash predicate from invocation requirements and any exact
/// independently reconstructed site facts. Call ceilings supply no site facts.
/// Predicate conversion and the certificate are checked by the proof owner.
pub(super) fn establishes(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
) -> bool {
    let Ok(denotations) = check_predicate_denotations(context, goal, requirements, semantic_axioms)
    else {
        return false;
    };
    let mut remaining = MAXIMUM_SEARCH_STEPS;
    let Some(proof) = prove(
        denotations.goal(),
        denotations.requirements(),
        denotations.semantic_axioms(),
        &mut remaining,
        0,
    ) else {
        return false;
    };
    denotations.check_certificate(context, &proof).is_ok()
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
            if let Some(proof) =
                common_consequence(goal, premise, requirements.len(), remaining, depth + 1)
            {
                return Some(proof);
            }
        }
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
                if let Some(proof) =
                    common_consequence(goal, child, assumption_count, remaining, depth + 1)
                {
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

pub(super) fn covers(caller: &TerminalMachine, published: &CrashRouteBucket) -> bool {
    // Body values, result pseudo-values and current storage do not belong to
    // this context. Ordinary module validation separately verifies complete
    // contract scope, including the declared structural parameter associations.
    let Ok(context) = PropositionContext::from_value_types_and_places(
        caller
            .parameters
            .iter()
            .map(|parameter| (parameter.id, parameter.scalar_type)),
        caller.structural_places.iter().filter_map(|place| {
            matches!(place.kind, StructuralPlaceKind::Parameter { .. })
                .then_some((place.id, place.kind))
        }),
    ) else {
        return false;
    };
    match published.alternatives.as_slice() {
        [] => false,
        [CrashRouteGuard::Predicate(predicate)] => establishes(
            &context,
            predicate.proposition(),
            &caller.contract.requires,
            &[],
        ),
        alternatives => {
            // A bucket publishes a union, not a chosen route. One checked
            // union goal shares the conversion and search budgets across all
            // alternatives, and preserves disjunctive entry requirements.
            let goal = Proposition::Disjunction(
                alternatives
                    .iter()
                    .map(|route| match route {
                        CrashRouteGuard::Truth => Proposition::Truth,
                        CrashRouteGuard::Predicate(predicate) => predicate.proposition().clone(),
                    })
                    .collect(),
            );
            establishes(&context, &goal, &caller.contract.requires, &[])
        }
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
    use super::*;
    use proof_admission::check_certificate;

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
                    &mut remaining,
                    0
                )
                .is_none()
            );
            assert_eq!(remaining, 0);
            let mut remaining = MAXIMUM_SEARCH_STEPS;
            let proof =
                common_consequence(&goal, premise, requirements.len(), &mut remaining, 0).unwrap();
            check_certificate(&context, &goal, &requirements, &axioms, &proof).unwrap();
        }
        let leaking_cases = Proposition::Disjunction(vec![
            Proposition::Conjunction(vec![goal.clone(), other.clone()]),
            other,
        ]);
        assert!(!establishes(&context, &goal, &[leaking_cases], &[]));
    }
}
