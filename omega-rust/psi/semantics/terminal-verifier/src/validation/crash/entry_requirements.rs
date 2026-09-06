//! A sufficient caller ceiling proved solely from invocation-entry requirements.
//! Callee continuations retain their independently reconstructed exact routes.

use proof_admission::{ProofNode, ProofRule, check_certificate};
use semantic_vocabulary::{Proposition, PropositionContext, StructuralPlaceKind};
use terminal_psi::{CrashRouteBucket, CrashRouteGuard, TerminalMachine};

const MAXIMUM_SEARCH_STEPS: usize = 4096;
const MAXIMUM_PROOF_DEPTH: usize = 64;

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
    let mut remaining = MAXIMUM_SEARCH_STEPS;
    published.alternatives.iter().any(|route| {
        let CrashRouteGuard::Predicate(predicate) = route else {
            return false;
        };
        let goal = predicate.proposition();
        let Some(proof) = prove(goal, &caller.contract.requires, &mut remaining, 0) else {
            return false;
        };
        // The search does not authorize a conclusion. The existing proof
        // kernel checks its exact assumption/conjunction certificate without
        // producer evidence, inferred runtime facts, or admission choices.
        check_certificate(&context, goal, &caller.contract.requires, &[], &proof).is_ok()
    })
}

fn step(remaining: &mut usize, depth: usize) -> Option<()> {
    if depth >= MAXIMUM_PROOF_DEPTH {
        return None;
    }
    *remaining = remaining.checked_sub(1)?;
    Some(())
}

fn prove(
    goal: &Proposition,
    requirements: &[Proposition],
    remaining: &mut usize,
    depth: usize,
) -> Option<ProofNode> {
    step(remaining, depth)?;
    for (index, requirement) in requirements.iter().enumerate() {
        let mut path = Vec::new();
        if projection(requirement, goal, &mut path, remaining, depth)? {
            let mut premise = requirement;
            let mut proof = ProofNode {
                conclusion: premise.clone(),
                rule: ProofRule::Assumption { index },
            };
            for conjunct in path {
                let Proposition::Conjunction(conjuncts) = premise else {
                    return None;
                };
                premise = conjuncts.get(conjunct)?;
                proof = ProofNode {
                    conclusion: premise.clone(),
                    rule: ProofRule::ConjunctionElimination {
                        conjunction: Box::new(proof),
                        conjunct,
                    },
                };
            }
            return Some(proof);
        }
    }
    let Proposition::Conjunction(conjuncts) = goal else {
        return None;
    };
    let children = conjuncts
        .iter()
        .map(|conjunct| prove(conjunct, requirements, remaining, depth + 1))
        .collect::<Option<Vec<_>>>()?;
    Some(ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::ConjunctionIntroduction(children),
    })
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

    #[test]
    fn conjunction_proof_search_exhausts_a_shared_budget() {
        let goal = Proposition::Truth;
        let requirements = [Proposition::Conjunction(vec![
            Proposition::Falsehood,
            goal.clone(),
        ])];
        let mut remaining = 2;
        assert!(prove(&goal, &requirements, &mut remaining, 0).is_none());
        assert_eq!(remaining, 0);
        let mut remaining = MAXIMUM_SEARCH_STEPS;
        let proof = prove(&goal, &requirements, &mut remaining, 0).unwrap();
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
        let goal = Proposition::Truth;
        let requirements = [Proposition::Disjunction(vec![
            goal.clone(),
            Proposition::Falsehood,
        ])];
        let mut remaining = MAXIMUM_SEARCH_STEPS;
        assert!(prove(&goal, &requirements, &mut remaining, 0).is_none());
    }

    #[test]
    fn projection_depth_is_bounded_even_with_remaining_steps() {
        let goal = Proposition::Truth;
        let mut premise = goal.clone();
        for _ in 0..MAXIMUM_PROOF_DEPTH {
            premise = Proposition::Conjunction(vec![premise]);
        }
        let mut remaining = MAXIMUM_SEARCH_STEPS;
        assert!(prove(&goal, &[premise], &mut remaining, 0).is_none());
        assert!(remaining > 0);
    }
}
