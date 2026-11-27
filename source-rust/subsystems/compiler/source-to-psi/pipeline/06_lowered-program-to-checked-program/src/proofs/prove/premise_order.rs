//! Transitivity between integer subjects whose carriers can differ.
//! Each edge cites one fact in this scope. Construct the certificate only for
//! the selected path so branches of the search never copy a proof tree.

use super::{
    BoundFactCursor, IntegerMathTerm, ProofNode, ProofRule, Proposition, Prover, chain_math, node,
};
use semantic_vocabulary::{IntegerCarrier, IntegerType, ScalarTerm, ValueId};

/// Read a bare subject only when its mathematical integer value is preserved.
fn fixed(id: ValueId, integer: IntegerType) -> Option<ValueId> {
    (integer.carrier() == IntegerCarrier::Fixed && !integer.is_address()).then_some(id)
}

fn subject(term: &ScalarTerm) -> Option<ValueId> {
    match term {
        ScalarTerm::Value { id, scalar_type } => {
            let integer = scalar_type.as_integer()?;
            fixed(*id, integer)
        }
        _ => None,
    }
}

fn order(proposition: &Proposition) -> Option<(ValueId, ValueId, bool)> {
    let (left, right, strict) = match proposition {
        Proposition::IntegerMathLessThan(left, right) => {
            (math_subject(left)?, math_subject(right)?, true)
        }
        Proposition::IntegerMathLessOrEqual(left, right) => {
            (math_subject(left)?, math_subject(right)?, false)
        }
        Proposition::LessThan(left, right) => (subject(left)?, subject(right)?, true),
        Proposition::LessOrEqual(left, right) => (subject(left)?, subject(right)?, false),
        _ => return None,
    };
    Some((left, right, strict))
}

fn math_subject(term: &IntegerMathTerm) -> Option<ValueId> {
    match term {
        IntegerMathTerm::MathValue { value, source_type } => fixed(*value, *source_type),
        _ => None,
    }
}

struct Step {
    fact: usize,
    parent: Option<usize>,
}

impl Prover<'_> {
    pub(super) fn premise_order(&self, goal: &Proposition) -> Option<ProofNode> {
        let (from, to, strict_goal) = order(goal)?;
        // One carrier already has its own order search. Try this graph for
        // mixed goals and for same-carrier goals with a mixed premise.
        if !matches!(
            goal,
            Proposition::IntegerMathLessThan(..) | Proposition::IntegerMathLessOrEqual(..)
        ) && !self.bound_facts.has_mathematical_order
        {
            return None;
        }
        let mut seen = self.bounds.like([false; 2]);
        let mut steps = Vec::<Step>::new();
        let mut queue = vec![(from, false, None)];
        let mut position = 0;
        while let Some(&(at, strict_path, parent)) = queue.get(position) {
            position += 1;
            let mut cursor = BoundFactCursor::default();
            while let Some(fact) = self.bound_facts.next(at, &mut cursor) {
                let Some((left, right, strict)) = order(self.facts[fact].proposition()) else {
                    continue;
                };
                if left != at {
                    continue;
                }
                let strict_path = strict_path || strict;
                let Some(visited) = seen.get_mut(right) else {
                    continue;
                };
                if std::mem::replace(&mut visited[usize::from(strict_path)], true) {
                    continue;
                }
                let edge = steps.len();
                steps.push(Step { fact, parent });
                if right == to && (strict_path || !strict_goal) {
                    return self.order_path(goal, &steps, edge, strict_path && !strict_goal);
                }
                queue.push((right, strict_path, Some(edge)));
            }
        }
        None
    }

    fn order_path(
        &self,
        goal: &Proposition,
        steps: &[Step],
        mut edge: usize,
        weaken: bool,
    ) -> Option<ProofNode> {
        let mut selected = Vec::new();
        loop {
            let step = &steps[edge];
            let fact = &self.facts[step.fact];
            let proof = fact.proof(&self.facts);
            let math = proof_admission::lift_fixed_integer_relation(fact.proposition());
            selected.push(match math {
                Some(math) => {
                    proof_admission::restate_integer_relation(math, proof, self.assumptions.len())
                }
                None => proof,
            });
            match step.parent {
                Some(parent) => edge = parent,
                None => break,
            }
        }
        let count = selected.len();
        let mut proof = chain_path(&mut selected.into_iter().rev(), count)?;
        if weaken {
            let (_, left, right) = math_order(&proof.conclusion)?;
            proof = node(
                Proposition::IntegerMathLessOrEqual(left.clone(), right.clone()),
                ProofRule::IntegerOrderWeakening {
                    relation: Box::new(proof),
                },
            );
        }
        if &proof.conclusion != goal {
            proof = proof_admission::restate_integer_relation(
                goal.clone(),
                proof,
                self.assumptions.len(),
            );
        }
        Some(proof)
    }
}

pub(super) fn math_order(
    proposition: &Proposition,
) -> Option<(bool, &IntegerMathTerm, &IntegerMathTerm)> {
    match proposition {
        Proposition::IntegerMathLessThan(left, right) => Some((true, left, right)),
        Proposition::IntegerMathLessOrEqual(left, right) => Some((false, left, right)),
        _ => None,
    }
}

/// Balance the transitivity tree so even a long chain has logarithmic depth.
fn chain_path(edges: &mut impl Iterator<Item = ProofNode>, count: usize) -> Option<ProofNode> {
    if count == 1 {
        return edges.next();
    }
    let first = chain_path(edges, count / 2)?;
    let second = chain_path(edges, count - count / 2)?;
    chain_math(first, second)
}
