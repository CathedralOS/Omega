//! Carrier-independent landed exact-operation range certificates.

use proof_admission::{
    IntegerAffineWitness, PrimitiveJudgment, ProofNode, ProofRule, check_integer_affine_witness,
    integer_affine_truth_bounds,
};
use semantic_vocabulary::{Proposition, PropositionContext, ScalarTerm};

use super::super::integer_evidence::{Citation, cited_facts, goal_target, relax};
use std::collections::{BTreeMap, BTreeSet};

#[cfg(test)]
mod tests;

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    let target = goal_target(goal)?;
    for mapped_proof in target_bounds(context, target, semantic_axioms) {
        if &mapped_proof.conclusion == goal {
            return Some(mapped_proof);
        }
        if let Some(proof) = relax(goal, mapped_proof) {
            return Some(proof);
        }
    }
    let Proposition::LessOrEqual(goal_left, _) = goal else {
        unreachable!()
    };
    let endpoint = usize::from(goal_left != target);
    // Index cited edges once. Discover only the target's connected frontier;
    // unrelated operation definitions must not each trigger a fresh proof search.
    // Scratch holds borrowed facts and predecessor indices, not cloned proof trees.
    let edges = relation_edges(assumptions, semantic_axioms, endpoint);
    let mut visited = BTreeSet::from([target]);
    let mut frontier = vec![(target, None::<(usize, &RelationEdge<'_>)>)];
    let mut cursor = 0;
    while cursor < frontier.len() {
        let current = frontier[cursor].0;
        if cursor != 0 && matches!(current, ScalarTerm::Value { .. }) {
            for mut bound in target_bounds(context, current, semantic_axioms) {
                let Proposition::LessOrEqual(left, right) = &bound.conclusion else {
                    continue;
                };
                if (if endpoint == 0 { left } else { right }) != current {
                    continue;
                }
                let mut predecessor = cursor;
                while let Some((previous, edge)) = frontier[predecessor].1 {
                    bound = edge.transport(bound, frontier[previous].0, endpoint);
                    predecessor = previous;
                }
                if &bound.conclusion == goal {
                    return Some(bound);
                }
                if let Some(proof) = relax(goal, bound) {
                    return Some(proof);
                }
            }
        }
        if let Some(outgoing) = edges.get(current) {
            for edge in outgoing {
                if visited.insert(edge.destination) {
                    frontier.push((edge.destination, Some((cursor, edge))));
                }
            }
        }
        cursor += 1;
    }
    None
}

struct RelationEdge<'fact> {
    destination: &'fact ScalarTerm,
    root: &'fact Proposition,
    citation: Citation,
    projections: Vec<usize>,
    reverse_equality: bool,
}

impl RelationEdge<'_> {
    fn transport(&self, bound: ProofNode, target: &ScalarTerm, endpoint: usize) -> ProofNode {
        let mut relation = self.citation.proof(self.root);
        for &conjunct in &self.projections {
            let Proposition::Conjunction(conjuncts) = &relation.conclusion else {
                unreachable!()
            };
            relation = ProofNode {
                conclusion: conjuncts[conjunct].clone(),
                rule: ProofRule::ConjunctionElimination {
                    conjunction: Box::new(relation),
                    conjunct,
                },
            };
        }
        let Proposition::LessOrEqual(left, right) = &bound.conclusion else {
            unreachable!()
        };
        let conclusion = if endpoint == 0 {
            Proposition::LessOrEqual(target.clone(), right.clone())
        } else {
            Proposition::LessOrEqual(left.clone(), target.clone())
        };
        let rule = if let Proposition::Equal(left, right) = &relation.conclusion {
            if self.reverse_equality {
                relation = ProofNode {
                    conclusion: Proposition::Equal(right.clone(), left.clone()),
                    rule: ProofRule::EqualitySymmetry {
                        equality: Box::new(relation),
                    },
                };
            }
            ProofRule::IntegerOrderSubstitution {
                relation: Box::new(bound),
                equality: Box::new(relation),
                endpoint,
            }
        } else {
            if let Proposition::LessThan(left, right) = &relation.conclusion {
                relation = ProofNode {
                    conclusion: Proposition::LessOrEqual(left.clone(), right.clone()),
                    rule: ProofRule::IntegerOrderWeakening {
                        relation: Box::new(relation),
                    },
                };
            }
            let (left, right) = if endpoint == 0 {
                (relation, bound)
            } else {
                (bound, relation)
            };
            ProofRule::IntegerLessOrEqualTransitivity {
                left_less_or_equal_middle: Box::new(left),
                middle_less_or_equal_right: Box::new(right),
            }
        };
        ProofNode { conclusion, rule }
    }
}

fn relation_edges<'fact>(
    assumptions: &'fact [Proposition],
    semantic_axioms: &'fact [Proposition],
    endpoint: usize,
) -> BTreeMap<&'fact ScalarTerm, Vec<RelationEdge<'fact>>> {
    let mut edges = BTreeMap::<_, Vec<_>>::new();
    for (citation, root) in cited_facts(assumptions, semantic_axioms) {
        let mut pending = vec![(root, Vec::new())];
        while let Some((fact, projections)) = pending.pop() {
            let (left, right, equality) = match fact {
                Proposition::Conjunction(conjuncts) => {
                    for (index, conjunct) in conjuncts.iter().enumerate() {
                        let mut path = projections.clone();
                        path.push(index);
                        pending.push((conjunct, path));
                    }
                    continue;
                }
                Proposition::Equal(left, right) => (left, right, true),
                Proposition::LessOrEqual(left, right) | Proposition::LessThan(left, right) => {
                    (left, right, false)
                }
                _ => continue,
            };
            if equality || endpoint == 0 {
                edges.entry(left).or_default().push(RelationEdge {
                    destination: right,
                    root,
                    citation,
                    projections: projections.clone(),
                    reverse_equality: false,
                });
            }
            if equality || endpoint == 1 {
                edges.entry(right).or_default().push(RelationEdge {
                    destination: left,
                    root,
                    citation,
                    projections,
                    reverse_equality: equality,
                });
            }
        }
    }
    edges
}

pub(super) fn target_bounds(
    context: &PropositionContext,
    target: &ScalarTerm,
    semantic_axioms: &[Proposition],
) -> Vec<ProofNode> {
    // Each candidate is a `(root, sibling)` operand order the kernel can
    // replay: the ordered operations resume only at their left operand,
    // while a bitwise and resumes at either operand with the other landing
    // as the mask literal.
    let Some((definition_axiom, candidates)) =
        semantic_axioms
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, proposition)| {
                let Proposition::Equal(left, right) = proposition else {
                    return None;
                };
                let expression = if left == target {
                    right
                } else if right == target {
                    left
                } else {
                    return None;
                };
                let candidates: Vec<(&ScalarTerm, &ScalarTerm)> = match expression {
                    ScalarTerm::ExactIntegerMultiply {
                        left: root,
                        right: sibling,
                        ..
                    }
                    | ScalarTerm::ExactIntegerDivide {
                        left: root,
                        right: sibling,
                        ..
                    }
                    | ScalarTerm::ExactIntegerRemainder {
                        left: root,
                        right: sibling,
                        ..
                    } => vec![(root.as_ref(), sibling.as_ref())],
                    ScalarTerm::IntegerBitwiseAnd { left, right, .. } => vec![
                        (left.as_ref(), right.as_ref()),
                        (right.as_ref(), left.as_ref()),
                    ],
                    _ => return None,
                };
                Some((index, candidates))
            })
    else {
        return Vec::new();
    };
    let truth = ProofNode {
        conclusion: Proposition::Truth,
        rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
    };
    let mut bounds = Vec::new();
    for (root, sibling) in candidates {
        let literal_axiom = if sibling.integer_value().is_some() {
            None
        } else {
            let Some(literal_axiom) = semantic_axioms[..definition_axiom]
                .iter()
                .enumerate()
                .rev()
                .find_map(|(index, proposition)| match proposition {
                    Proposition::Equal(left, right)
                        if left == sibling && right.integer_value().is_some() =>
                    {
                        Some(index)
                    }
                    Proposition::Equal(left, right)
                        if right == sibling && left.integer_value().is_some() =>
                    {
                        Some(index)
                    }
                    _ => None,
                })
            else {
                continue;
            };
            Some(literal_axiom)
        };
        let witness = IntegerAffineWitness {
            root: root.clone(),
            target: target.clone(),
            definition_axioms: vec![definition_axiom],
            literal_axioms: vec![literal_axiom],
        };
        let Ok(form) = check_integer_affine_witness(context, semantic_axioms, &witness) else {
            continue;
        };
        let Ok(mapped_bounds) = integer_affine_truth_bounds(&form) else {
            continue;
        };
        bounds.extend(mapped_bounds.into_iter().map(|mapped| ProofNode {
            conclusion: mapped,
            rule: ProofRule::IntegerAffineBound {
                root_bound: Box::new(truth.clone()),
                witness: witness.clone(),
            },
        }));
    }
    bounds
}
