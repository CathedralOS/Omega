//! Checked, non-serialized conversion of fixed Boolean predicate denotations.
//!
//! Conversion changes no premise's authority. The original inputs remain
//! borrowed, and only this owner can construct their equivalent proof views.

use semantic_vocabulary::{
    Proposition, PropositionContext, PropositionError, ScalarTerm, ScalarType,
};

use crate::{ProofError, ProofNode, check_certificate};

mod budget;
use budget::Budget;

pub struct CheckedPredicateDenotations<'input> {
    original_goal: &'input Proposition,
    original_requirements: &'input [Proposition],
    original_semantic_axioms: &'input [Proposition],
    goal: Proposition,
    requirements: Vec<Proposition>,
    semantic_axioms: Vec<Proposition>,
}

impl CheckedPredicateDenotations<'_> {
    pub fn goal(&self) -> &Proposition {
        &self.goal
    }

    pub fn requirements(&self) -> &[Proposition] {
        &self.requirements
    }

    pub fn semantic_axioms(&self) -> &[Proposition] {
        &self.semantic_axioms
    }

    pub fn check_certificate(
        &self,
        context: &PropositionContext,
        proof: &ProofNode,
    ) -> Result<(), PredicateDenotationError> {
        // A checked conversion may be used with a different context only if
        // every original proposition still has its exact declared types there.
        for proposition in std::iter::once(self.original_goal)
            .chain(self.original_requirements)
            .chain(self.original_semantic_axioms)
        {
            context
                .validate(proposition)
                .map_err(PredicateDenotationError::Malformed)?;
        }
        check_certificate(
            context,
            &self.goal,
            &self.requirements,
            &self.semantic_axioms,
            proof,
        )
        .map_err(PredicateDenotationError::Proof)
    }
}

pub fn check_predicate_denotations<'input>(
    context: &PropositionContext,
    goal: &'input Proposition,
    requirements: &'input [Proposition],
    semantic_axioms: &'input [Proposition],
) -> Result<CheckedPredicateDenotations<'input>, PredicateDenotationError> {
    let mut budget = Budget::new();
    // Bound input traversal before recursive type validation or cloning. The
    // same budget subsequently covers all newly expanded Boolean branches.
    for proposition in std::iter::once(goal)
        .chain(requirements)
        .chain(semantic_axioms)
    {
        budget.proposition(proposition, 0)?;
        context
            .validate(proposition)
            .map_err(PredicateDenotationError::Malformed)?;
    }
    let normalized_goal = normalize(goal, &mut budget, 0)?;
    let normalized_requirements = requirements
        .iter()
        .map(|proposition| normalize(proposition, &mut budget, 0))
        .collect::<Result<Vec<_>, _>>()?;
    let normalized_axioms = semantic_axioms
        .iter()
        .map(|proposition| normalize(proposition, &mut budget, 0))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(CheckedPredicateDenotations {
        original_goal: goal,
        original_requirements: requirements,
        original_semantic_axioms: semantic_axioms,
        goal: normalized_goal,
        requirements: normalized_requirements,
        semantic_axioms: normalized_axioms,
    })
}

fn normalize(
    proposition: &Proposition,
    budget: &mut Budget,
    depth: usize,
) -> Result<Proposition, PredicateDenotationError> {
    budget.step(depth)?;
    match proposition {
        Proposition::Equal(left, right) if left.scalar_type() == ScalarType::Boolean => {
            boolean_equality(left, right, true, budget, depth + 1)
        }
        Proposition::Equal(left, right) => Ok(equality(left.clone(), right.clone())),
        Proposition::Conjunction(children) | Proposition::Disjunction(children) => {
            let conjunction = matches!(proposition, Proposition::Conjunction(_));
            let children = children
                .iter()
                .map(|child| normalize(child, budget, depth + 1))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(connective(children, conjunction))
        }
        Proposition::Implication {
            premise,
            conclusion,
        } => Ok(Proposition::Implication {
            premise: Box::new(normalize(premise, budget, depth + 1)?),
            conclusion: Box::new(normalize(conclusion, budget, depth + 1)?),
        }),
        // Integer arithmetic, IEEE comparisons, content and declared atoms
        // remain exact observations; this conversion adds no laws for them.
        other => Ok(other.clone()),
    }
}

fn boolean_equality(
    left: &ScalarTerm,
    right: &ScalarTerm,
    positive: bool,
    budget: &mut Budget,
    depth: usize,
) -> Result<Proposition, PredicateDenotationError> {
    budget.step(depth)?;
    if left == right {
        return Ok(if positive {
            Proposition::Truth
        } else {
            Proposition::Falsehood
        });
    }
    if let ScalarTerm::Boolean(value) = left {
        return boolean(right, positive == *value, budget, depth + 1);
    }
    if let ScalarTerm::Boolean(value) = right {
        return boolean(left, positive == *value, budget, depth + 1);
    }
    if positive && boolean_atom(left) && boolean_atom(right) {
        return Ok(equality(left.clone(), right.clone()));
    }
    // Equality of predicates is equality of their two Boolean denotations,
    // never an assumption that either predicate is true. Expansion is charged
    // through the same budget on all four recursive visits.
    let same_true = connective(
        vec![
            boolean(left, true, budget, depth + 1)?,
            boolean(right, positive, budget, depth + 1)?,
        ],
        true,
    );
    let same_false = connective(
        vec![
            boolean(left, false, budget, depth + 1)?,
            boolean(right, !positive, budget, depth + 1)?,
        ],
        true,
    );
    Ok(connective(vec![same_true, same_false], false))
}

fn boolean_atom(term: &ScalarTerm) -> bool {
    matches!(
        term,
        ScalarTerm::Value {
            scalar_type: ScalarType::Boolean,
            ..
        } | ScalarTerm::BooleanField { .. }
    )
}

fn boolean(
    term: &ScalarTerm,
    positive: bool,
    budget: &mut Budget,
    depth: usize,
) -> Result<Proposition, PredicateDenotationError> {
    budget.step(depth)?;
    match term {
        ScalarTerm::Boolean(value) => Ok(if *value == positive {
            Proposition::Truth
        } else {
            Proposition::Falsehood
        }),
        ScalarTerm::BooleanNot { operand } => boolean(operand, !positive, budget, depth + 1),
        ScalarTerm::BooleanEqual { left, right } => {
            boolean_equality(left, right, positive, budget, depth + 1)
        }
        ScalarTerm::IntegerEqual { left, right, .. } => {
            if positive {
                Ok(equality(*left.clone(), *right.clone()))
            } else {
                Ok(connective(
                    vec![
                        Proposition::LessThan(*left.clone(), *right.clone()),
                        Proposition::LessThan(*right.clone(), *left.clone()),
                    ],
                    false,
                ))
            }
        }
        ScalarTerm::IntegerLessThan { left, right, .. } => Ok(if positive {
            Proposition::LessThan(*left.clone(), *right.clone())
        } else {
            Proposition::LessOrEqual(*right.clone(), *left.clone())
        }),
        ScalarTerm::IntegerLessOrEqual { left, right, .. } => Ok(if positive {
            Proposition::LessOrEqual(*left.clone(), *right.clone())
        } else {
            Proposition::LessThan(*right.clone(), *left.clone())
        }),
        _ => Ok(equality(term.clone(), ScalarTerm::Boolean(positive))),
    }
}

fn equality(left: ScalarTerm, right: ScalarTerm) -> Proposition {
    if left <= right {
        Proposition::Equal(left, right)
    } else {
        Proposition::Equal(right, left)
    }
}

fn connective(children: Vec<Proposition>, conjunction: bool) -> Proposition {
    let mut flattened = Vec::new();
    for child in children {
        match child {
            Proposition::Falsehood if conjunction => return Proposition::Falsehood,
            Proposition::Truth if !conjunction => return Proposition::Truth,
            Proposition::Truth | Proposition::Falsehood => {}
            Proposition::Conjunction(children) if conjunction => flattened.extend(children),
            Proposition::Disjunction(children) if !conjunction => flattened.extend(children),
            other => flattened.push(other),
        }
    }
    flattened.sort_unstable();
    flattened.dedup();
    match flattened.len() {
        0 => {
            if conjunction {
                Proposition::Truth
            } else {
                Proposition::Falsehood
            }
        }
        1 => flattened.pop().expect("one normalized connective child"),
        _ => {
            if conjunction {
                Proposition::Conjunction(flattened)
            } else {
                Proposition::Disjunction(flattened)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PredicateDenotationError {
    ResourceLimitExceeded,
    Malformed(PropositionError),
    Proof(ProofError),
}

impl std::fmt::Display for PredicateDenotationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for PredicateDenotationError {}
