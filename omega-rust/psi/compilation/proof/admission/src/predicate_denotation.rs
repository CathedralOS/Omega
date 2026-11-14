/*
    Checked denotation of boolean scalar predicate trees and value equalities
    into propositions under explicit node and size budgets.

    We verify that claimed propositions match the exact denotation of scalar
    predicate trees and value-equality substitutions without unbudgeted
    traversal.
*/

use semantic_vocabulary::{
    Proposition, PropositionContext, PropositionError, ScalarTerm, ScalarType,
};

use crate::{ProofError, ProofNode, check_certificate};

pub(crate) use semantic_vocabulary::{scalar_term_binary_operands, scalar_term_unary_operand};
use std::iter::once;

mod budget {
    use semantic_vocabulary::{ContentTerm, IntegerMathTerm, Proposition, ScalarTerm};

    use super::PredicateDenotationError;
    use semantic_vocabulary::scalar_term_predicate_quint_pattern;

    pub(super) struct Budget {
        remaining: usize,
    }

    impl Budget {
        pub(super) fn new() -> Self {
            Self { remaining: 4096 }
        }

        pub(super) fn step(&mut self, depth: usize) -> Result<(), PredicateDenotationError> {
            if depth >= 64 {
                return Err(PredicateDenotationError::ResourceLimitExceeded);
            }
            self.count(1)
        }

        fn count(&mut self, count: usize) -> Result<(), PredicateDenotationError> {
            self.remaining = self
                .remaining
                .checked_sub(count)
                .ok_or(PredicateDenotationError::ResourceLimitExceeded)?;
            Ok(())
        }

        pub(super) fn proposition(
            &mut self,
            value: &Proposition,
            depth: usize,
        ) -> Result<(), PredicateDenotationError> {
            self.step(depth)?;
            match value {
                semantic_vocabulary::scalar_comparison_operands!(left, right) => {
                    self.scalar(left, depth + 1)?;
                    self.scalar(right, depth + 1)?;
                }
                semantic_vocabulary::integer_math_comparison_operands!(left, right) => {
                    self.mathematical(left, depth + 1)?;
                    self.mathematical(right, depth + 1)?;
                }
                Proposition::Conjunction(children) | Proposition::Disjunction(children) => {
                    for child in children {
                        self.proposition(child, depth + 1)?;
                    }
                }
                Proposition::Implication {
                    premise,
                    conclusion,
                } => {
                    self.proposition(premise, depth + 1)?;
                    self.proposition(conclusion, depth + 1)?;
                }
                Proposition::IeeeFloatComparison { left, right, .. } => {
                    self.count(left.path().len())?;
                    self.count(right.path().len())?;
                }
                Proposition::ByteSequenceEqual { left, right } => {
                    self.count(left.path().len())?;
                    self.count(right.path().len())?;
                }
                Proposition::StructuralCaseMembership { subject, .. } => {
                    self.count(subject.path().len())?
                }
                Proposition::StructuralIdentity { left, right } => {
                    self.count(left.path().len())?;
                    self.count(right.path().len())?;
                }
                Proposition::ContentConservation(value) => {
                    self.content(value.left(), depth + 1)?;
                    self.content(value.right(), depth + 1)?;
                }
                Proposition::Truth | Proposition::Falsehood | Proposition::Atom(_) => {}
            }
            Ok(())
        }

        fn content(
            &mut self,
            value: &ContentTerm,
            depth: usize,
        ) -> Result<(), PredicateDenotationError> {
            self.step(depth)?;
            match value {
                ContentTerm::Projection { subject, .. } => self.count(subject.segments.len())?,
                ContentTerm::Separate(children) => {
                    for child in children {
                        self.content(child, depth + 1)?;
                    }
                }
            }
            Ok(())
        }

        fn mathematical(
            &mut self,
            value: &IntegerMathTerm,
            depth: usize,
        ) -> Result<(), PredicateDenotationError> {
            self.step(depth)?;
            if let IntegerMathTerm::Embed(term) = value {
                return self.scalar(term, depth + 1);
            }
            for operand in value.operands() {
                self.mathematical(operand, depth + 1)?;
            }
            Ok(())
        }

        pub(super) fn scalar(
            &mut self,
            value: &ScalarTerm,
            depth: usize,
        ) -> Result<(), PredicateDenotationError> {
            self.step(depth)?;
            match value {
                ScalarTerm::Application { .. }
                | ScalarTerm::Value { .. }
                | ScalarTerm::Boolean(_)
                | ScalarTerm::Integer { .. }
                | ScalarTerm::IeeeFloatLiteral(_) => {}
                ScalarTerm::StructuralRead { path, .. } => {
                    self.count(path.len())?;
                    for operand in value.operands() {
                        self.scalar(operand, depth + 1)?;
                    }
                }
                scalar_term_predicate_quint_pattern!(path, ..) => self.count(path.len())?,
                super::scalar_term_unary_operand!(operand) => self.scalar(operand, depth + 1)?,
                super::scalar_term_binary_operands!(left, right) => {
                    self.scalar(left, depth + 1)?;
                    self.scalar(right, depth + 1)?;
                }
            }
            Ok(())
        }
    }
}
mod value_equalities {
    use semantic_vocabulary::{IntegerMathTerm, Proposition, ScalarTerm, ScalarType};

    use super::{PredicateDenotationError, budget::Budget};

    pub(super) struct ValueEqualities<'input> {
        definitions: Vec<(&'input ScalarTerm, &'input ScalarTerm)>,
    }

    pub(super) fn is_definition_subject(term: &ScalarTerm) -> bool {
        matches!(
            term,
            ScalarTerm::Value { .. }
                | ScalarTerm::IntegerField { .. }
                | ScalarTerm::BooleanField { .. }
                | ScalarTerm::IeeeFloatField { .. }
                | ScalarTerm::ViewExtent { .. }
                | ScalarTerm::StructuralRead { .. }
                | ScalarTerm::ByteSequencePredicate { .. }
        )
    }

    impl<'input> ValueEqualities<'input> {
        pub(super) fn from_semantic_axioms(
            axioms: impl IntoIterator<Item = &'input Proposition>,
        ) -> Self {
            let mut definitions = Vec::new();
            for axiom in axioms {
                let Some((left, right)) = axiom.as_equal() else {
                    continue;
                };
                if !is_definition_subject(left) {
                    continue;
                }
                if left != right && !definitions.iter().any(|(subject, _)| *subject == left) {
                    definitions.push((left, right));
                }
            }
            Self { definitions }
        }

        pub(super) fn proposition(
            &self,
            original: &Proposition,
            budget: &mut Budget,
            depth: usize,
        ) -> Result<Proposition, PredicateDenotationError> {
            budget.proposition(original, depth)?;
            let mut transported = original.clone();
            match &mut transported {
                semantic_vocabulary::ordered_comparison_operands!(left, right) => {
                    *left = self.scalar(left, budget, depth + 1, &mut Vec::new())?;
                    *right = self.scalar(right, budget, depth + 1, &mut Vec::new())?;
                }
                Proposition::ScalarIeeeFloatComparison {
                    kind, left, right, ..
                } => {
                    *left = self.scalar(left, budget, depth + 1, &mut Vec::new())?;
                    *right = self.scalar(right, budget, depth + 1, &mut Vec::new())?;
                    if kind.is_symmetric() && left > right {
                        std::mem::swap(left, right);
                    }
                }
                Proposition::Conjunction(children) | Proposition::Disjunction(children) => {
                    for child in children {
                        *child = self.proposition(child, budget, depth + 1)?;
                    }
                }
                Proposition::Implication {
                    premise,
                    conclusion,
                } => {
                    **premise = self.proposition(premise, budget, depth + 1)?;
                    **conclusion = self.proposition(conclusion, budget, depth + 1)?;
                }
                semantic_vocabulary::integer_math_comparison_operands!(left, right) => {
                    *left = self.mathematical(left, budget, depth + 1)?;
                    *right = self.mathematical(right, budget, depth + 1)?;
                }
                Proposition::IeeeFloatComparison { .. }
                | Proposition::ByteSequenceEqual { .. }
                | Proposition::StructuralIdentity { .. }
                | Proposition::StructuralCaseMembership { .. }
                | Proposition::ContentConservation(_)
                | Proposition::Truth
                | Proposition::Falsehood
                | Proposition::Atom(_) => {}
            }
            Ok(transported)
        }

        fn mathematical(
            &self,
            original: &IntegerMathTerm,
            budget: &mut Budget,
            depth: usize,
        ) -> Result<IntegerMathTerm, PredicateDenotationError> {
            budget.step(depth)?;
            Ok(match original {
                IntegerMathTerm::Embed(operand) => {
                    let scalar = self.scalar(operand, budget, depth + 1, &mut Vec::new())?;
                    if scalar.scalar_type() != operand.scalar_type() {
                        return Err(PredicateDenotationError::InvalidValueEquality);
                    }
                    crate::proof::integer_math_normalization::lift_exact_integer_term(&scalar)
                        .or_else(|| IntegerMathTerm::embed(scalar))
                        .ok_or(PredicateDenotationError::InvalidValueEquality)?
                }
                IntegerMathTerm::MathValue { source_type, value } => {
                    let scalar = self.scalar(
                        &ScalarTerm::value(*value, ScalarType::Integer(*source_type)),
                        budget,
                        depth + 1,
                        &mut Vec::new(),
                    )?;
                    if scalar.scalar_type() != ScalarType::Integer(*source_type) {
                        return Err(PredicateDenotationError::InvalidValueEquality);
                    }
                    crate::proof::integer_math_normalization::lift_exact_integer_term(&scalar)
                        .or_else(|| IntegerMathTerm::embed(scalar))
                        .ok_or(PredicateDenotationError::InvalidValueEquality)?
                }
                IntegerMathTerm::IntegerLiteral(_) => original.clone(),
                IntegerMathTerm::Add(left, right) => IntegerMathTerm::Add(
                    Box::new(self.mathematical(left, budget, depth + 1)?),
                    Box::new(self.mathematical(right, budget, depth + 1)?),
                ),
                IntegerMathTerm::Subtract(left, right) => IntegerMathTerm::Subtract(
                    Box::new(self.mathematical(left, budget, depth + 1)?),
                    Box::new(self.mathematical(right, budget, depth + 1)?),
                ),
                IntegerMathTerm::Multiply(left, right) => IntegerMathTerm::Multiply(
                    Box::new(self.mathematical(left, budget, depth + 1)?),
                    Box::new(self.mathematical(right, budget, depth + 1)?),
                ),
                IntegerMathTerm::ShiftLeft { value, count } => IntegerMathTerm::ShiftLeft {
                    value: Box::new(self.mathematical(value, budget, depth + 1)?),
                    count: Box::new(self.mathematical(count, budget, depth + 1)?),
                },
            })
        }

        fn scalar(
            &self,
            original: &ScalarTerm,
            budget: &mut Budget,
            depth: usize,
            active: &mut Vec<usize>,
        ) -> Result<ScalarTerm, PredicateDenotationError> {
            budget.scalar(original, depth)?;
            if let Some((position, (_, definition))) = self
                .definitions
                .iter()
                .enumerate()
                .find(|(_, (subject, _))| *subject == original)
            {
                if active.contains(&position) {
                    return Err(PredicateDenotationError::CyclicValueEquality);
                }
                active.push(position);
                let result = self.scalar(definition, budget, depth + 1, active)?;
                active.pop();
                return Ok(result);
            }
            let mut transported = original.clone();
            match &mut transported {
                ScalarTerm::StructuralRead { .. } => {
                    for operand in transported.operands_mut() {
                        *operand = self.scalar(operand, budget, depth + 1, active)?;
                    }
                }
                super::scalar_term_unary_operand!(operand) => {
                    **operand = self.scalar(operand, budget, depth + 1, active)?;
                }
                super::scalar_term_binary_operands!(left, right) => {
                    **left = self.scalar(left, budget, depth + 1, active)?;
                    **right = self.scalar(right, budget, depth + 1, active)?;
                }
                ScalarTerm::Application { .. }
                | ScalarTerm::Value { .. }
                | semantic_vocabulary::scalar_term_predicate_quint_pattern!(..)
                | ScalarTerm::Boolean(_)
                | ScalarTerm::Integer { .. }
                | ScalarTerm::IeeeFloatLiteral(_) => {}
            }
            Ok(transported)
        }
    }
}
use budget::Budget;

pub fn check_predicate_evaluation_size<'input>(
    propositions: impl IntoIterator<Item = &'input Proposition>,
) -> Result<(), PredicateDenotationError> {
    let mut budget = Budget::new();
    for proposition in propositions {
        budget.proposition(proposition, 0)?;
    }
    Ok(())
}

fn checked_denotations<'input>(
    context: &PropositionContext,
    goal: &'input Proposition,
    requirements: &'input [Proposition],
    semantic_axioms: &'input [Proposition],
    mut convert: impl FnMut(&Proposition, &mut Budget) -> Result<Proposition, PredicateDenotationError>,
) -> Result<CheckedPredicateDenotations<'input>, PredicateDenotationError> {
    let mut budget = Budget::new();
    for proposition in once(goal).chain(requirements).chain(semantic_axioms) {
        budget.proposition(proposition, 0)?;
        context
            .validate(proposition)
            .map_err(PredicateDenotationError::Malformed)?;
    }
    Ok(CheckedPredicateDenotations {
        original_goal: goal,
        original_requirements: requirements,
        original_semantic_axioms: semantic_axioms,
        goal: convert(goal, &mut budget)?,
        requirements: requirements
            .iter()
            .map(|proposition| convert(proposition, &mut budget))
            .collect::<Result<Vec<_>, _>>()?,
        semantic_axioms: semantic_axioms
            .iter()
            .map(|proposition| convert(proposition, &mut budget))
            .collect::<Result<Vec<_>, _>>()?,
    })
}

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
        for proposition in once(self.original_goal)
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
    checked_denotations(
        context,
        goal,
        requirements,
        semantic_axioms,
        |proposition, budget| normalize(proposition, budget, 0),
    )
}

pub fn check_predicate_denotations_with_value_equalities<'input>(
    context: &PropositionContext,
    goal: &'input Proposition,
    requirements: &'input [Proposition],
    semantic_axioms: &'input [Proposition],
) -> Result<CheckedPredicateDenotations<'input>, PredicateDenotationError> {
    let equalities = value_equalities::ValueEqualities::from_semantic_axioms(semantic_axioms);
    checked_denotations(
        context,
        goal,
        requirements,
        semantic_axioms,
        |proposition, budget| transport_denotation(context, proposition, &equalities, budget),
    )
}

pub fn check_value_equality_denotation<'input>(
    context: &PropositionContext,
    goal: &Proposition,
    equations: impl IntoIterator<Item = &'input Proposition>,
) -> Result<Proposition, PredicateDenotationError> {
    let mut budget = Budget::new();
    budget.proposition(goal, 0)?;
    context
        .validate(goal)
        .map_err(PredicateDenotationError::Malformed)?;
    let mut checked_equations = Vec::new();
    for equation in equations {
        budget.proposition(equation, 0)?;
        context
            .validate(equation)
            .map_err(PredicateDenotationError::Malformed)?;
        if !matches!(equation, Proposition::Equal(subject, _) if value_equalities::is_definition_subject(subject))
        {
            return Err(PredicateDenotationError::InvalidValueEquality);
        }
        checked_equations.push(equation);
    }
    if checked_equations.is_empty() {
        return Err(PredicateDenotationError::InvalidValueEquality);
    }
    let equalities = value_equalities::ValueEqualities::from_semantic_axioms(checked_equations);
    transport_denotation(context, goal, &equalities, &mut budget)
}

fn transport_denotation(
    context: &PropositionContext,
    proposition: &Proposition,
    equalities: &value_equalities::ValueEqualities<'_>,
    budget: &mut Budget,
) -> Result<Proposition, PredicateDenotationError> {
    let transported = equalities.proposition(proposition, budget, 0)?;
    context
        .validate(&transported)
        .map_err(PredicateDenotationError::Malformed)?;
    let normalized = normalize(&transported, budget, 0)?;
    budget.proposition(&normalized, 0)?;
    context
        .validate(&normalized)
        .map_err(PredicateDenotationError::Malformed)?;
    Ok(normalized)
}

fn normalize(
    proposition: &Proposition,
    budget: &mut Budget,
    depth: usize,
) -> Result<Proposition, PredicateDenotationError> {
    budget.step(depth)?;
    if let Some(denotation) = closed_integer_relation(proposition) {
        return Ok(denotation);
    }
    match proposition {
        Proposition::StructuralIdentity { left, right } if left == right => Ok(Proposition::Truth),
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
        other => Ok(other.clone()),
    }
}

fn closed_integer_relation(proposition: &Proposition) -> Option<Proposition> {
    let (left, right) = match proposition {
        semantic_vocabulary::ordered_comparison_operands!(left, right) => (left, right),
        _ => return None,
    };
    if !matches!(
        (left, right),
        (ScalarTerm::Integer { .. }, ScalarTerm::Integer { .. })
    ) {
        return None;
    }
    let integer = left.scalar_type().as_integer()?;
    if integer.carrier() != semantic_vocabulary::IntegerCarrier::Fixed {
        return None;
    }
    let ordering = crate::kernel::compare_integer_literals(left, right)?;
    let holds = match proposition {
        Proposition::Equal(..) => ordering.is_eq(),
        Proposition::LessThan(..) => ordering.is_lt(),
        Proposition::LessOrEqual(..) => !ordering.is_gt(),
        _ => return None,
    };
    Some(if holds {
        Proposition::Truth
    } else {
        Proposition::Falsehood
    })
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
            | ScalarTerm::ByteSequencePredicate { .. }
    )
}

fn boolean(
    term: &ScalarTerm,
    positive: bool,
    budget: &mut Budget,
    depth: usize,
) -> Result<Proposition, PredicateDenotationError> {
    budget.step(depth)?;
    if let Some(comparison) = comparison_with_representable_literal(term) {
        return boolean(&comparison, positive, budget, depth + 1);
    }
    if let Some(proposition) = term.mathematical_comparison(positive) {
        return normalize(&proposition, budget, depth + 1);
    }
    match term {
        ScalarTerm::IntegerEqual { left, right, .. }
        | ScalarTerm::IntegerLessThan { left, right, .. }
        | ScalarTerm::IntegerLessOrEqual { left, right, .. }
            if left.scalar_type() != right.scalar_type() =>
        {
            Ok(equality(term.clone(), ScalarTerm::Boolean(positive)))
        }
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
                normalize(&equality(*left.clone(), *right.clone()), budget, depth + 1)
            } else {
                Ok(connective(
                    vec![
                        normalize(
                            &Proposition::LessThan(*left.clone(), *right.clone()),
                            budget,
                            depth + 1,
                        )?,
                        normalize(
                            &Proposition::LessThan(*right.clone(), *left.clone()),
                            budget,
                            depth + 1,
                        )?,
                    ],
                    false,
                ))
            }
        }
        ScalarTerm::IntegerLessThan { left, right, .. } => normalize(
            &if positive {
                Proposition::LessThan(*left.clone(), *right.clone())
            } else {
                Proposition::LessOrEqual(*right.clone(), *left.clone())
            },
            budget,
            depth + 1,
        ),
        ScalarTerm::IntegerLessOrEqual { left, right, .. } => normalize(
            &if positive {
                Proposition::LessOrEqual(*left.clone(), *right.clone())
            } else {
                Proposition::LessThan(*right.clone(), *left.clone())
            },
            budget,
            depth + 1,
        ),
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
    CyclicValueEquality,
    InvalidValueEquality,
    Malformed(PropositionError),
    Proof(ProofError),
}

crate::debug_display!(PredicateDenotationError);

impl std::error::Error for PredicateDenotationError {}

fn comparison_with_representable_literal(term: &ScalarTerm) -> Option<ScalarTerm> {
    let (kind, left, right) = match term {
        ScalarTerm::IntegerEqual { left, right, .. } => (0, left.as_ref(), right.as_ref()),
        ScalarTerm::IntegerLessThan { left, right, .. } => (1, left.as_ref(), right.as_ref()),
        ScalarTerm::IntegerLessOrEqual { left, right, .. } => (2, left.as_ref(), right.as_ref()),
        _ => return None,
    };
    let (ScalarType::Integer(left_type), ScalarType::Integer(right_type)) =
        (left.scalar_type(), right.scalar_type())
    else {
        return None;
    };
    if left_type == right_type {
        return None;
    }
    let (carrier, left, right) = if let ScalarTerm::Integer { value, .. } = right {
        (
            left_type,
            left.clone(),
            ScalarTerm::integer(
                left_type,
                right_type.exact_cast_value_to(left_type, *value)?,
            )
            .ok()?,
        )
    } else if let ScalarTerm::Integer { value, .. } = left {
        (
            right_type,
            ScalarTerm::integer(
                right_type,
                left_type.exact_cast_value_to(right_type, *value)?,
            )
            .ok()?,
            right.clone(),
        )
    } else {
        return None;
    };
    match kind {
        0 => ScalarTerm::integer_equal(carrier, left, right),
        1 => ScalarTerm::integer_less_than(carrier, left, right),
        _ => ScalarTerm::integer_less_or_equal(carrier, left, right),
    }
    .ok()
}

#[cfg(test)]
mod value_equality_tests {
    use semantic_vocabulary::{
        IntegerSign, IntegerType, IntegerValue, Proposition, PropositionContext, ScalarTerm,
        ScalarType, ValueId,
    };

    use super::{
        CheckedPredicateDenotations, PredicateDenotationError,
        check_predicate_denotations_with_value_equalities, check_value_equality_denotation,
    };
    use crate::proof::{assumption_proof, semantic_axiom_proof};
    use crate::{PrimitiveJudgment, ProofNode, ProofRule, proof_node, value_context};

    fn integer_type() -> IntegerType {
        IntegerType::new(IntegerSign::Signed, 32).unwrap()
    }

    fn value(identity: u64) -> ScalarTerm {
        ScalarTerm::value(
            ValueId::new(identity).unwrap(),
            ScalarType::Integer(integer_type()),
        )
    }

    fn literal(integer: i128) -> ScalarTerm {
        ScalarTerm::integer(integer_type(), IntegerValue::Signed(integer)).unwrap()
    }

    fn sum(left: ScalarTerm, right: ScalarTerm) -> ScalarTerm {
        ScalarTerm::WrappingIntegerAdd {
            scalar_type: integer_type(),
            left: Box::new(left),
            right: Box::new(right),
        }
    }

    fn fixture() -> (PropositionContext, Proposition, Vec<Proposition>) {
        let context = PropositionContext::from_value_types((1..=5).map(|identity| {
            (
                ValueId::new(identity).unwrap(),
                if identity == 5 {
                    ScalarType::Boolean
                } else {
                    ScalarType::Integer(integer_type())
                },
            )
        }))
        .unwrap();
        let condition = ScalarTerm::value(ValueId::new(5).unwrap(), ScalarType::Boolean);
        let goal = Proposition::Equal(
            ScalarTerm::Boolean(true),
            ScalarTerm::IntegerLessOrEqual {
                scalar_type: integer_type(),
                left: Box::new(value(2)),
                right: Box::new(sum(value(1), literal(1))),
            },
        );
        let axioms = vec![
            Proposition::Equal(value(3), literal(1)),
            Proposition::Equal(value(4), sum(value(1), value(3))),
            Proposition::Equal(
                condition.clone(),
                ScalarTerm::IntegerLessOrEqual {
                    scalar_type: integer_type(),
                    left: Box::new(value(2)),
                    right: Box::new(value(4)),
                },
            ),
            Proposition::LessOrEqual(value(2), value(4)),
            Proposition::Equal(condition, ScalarTerm::Boolean(true)),
        ];
        (context, goal, axioms)
    }

    fn citation(checked: &CheckedPredicateDenotations<'_>, index: usize) -> ProofNode {
        proof_node!(checked.goal().clone() => ProofRule::SemanticAxiom { index })
    }

    #[test]
    fn missing_definitions_or_changed_literal_policy_and_order_cannot_prove_the_guard() {
        for mutation in ["constant", "operation", "literal", "policy", "order"] {
            let (context, goal, mut axioms) = fixture();
            match mutation {
                "constant" => axioms[0] = Proposition::Truth,
                "operation" => axioms[1] = Proposition::Truth,
                "literal" => axioms[0] = Proposition::Equal(value(3), literal(2)),
                "policy" => {
                    axioms[1] = Proposition::Equal(
                        value(4),
                        ScalarTerm::ExactIntegerAdd {
                            scalar_type: integer_type(),
                            left: Box::new(value(1)),
                            right: Box::new(value(3)),
                        },
                    );
                }
                "order" => axioms[1] = Proposition::Equal(value(4), sum(value(3), value(1))),
                _ => unreachable!(),
            }
            let checked =
                check_predicate_denotations_with_value_equalities(&context, &goal, &[], &axioms)
                    .unwrap();
            for index in [3, 4] {
                assert!(
                    checked
                        .check_certificate(&context, &citation(&checked, index))
                        .is_err(),
                    "{mutation}"
                );
            }
        }
    }

    #[test]
    fn requirements_and_conditional_equalities_never_supply_global_definitions() {
        for location in ["requirements", "conjunction", "disjunction", "implication"] {
            let (context, goal, mut axioms) = fixture();
            let definition = std::mem::replace(&mut axioms[0], Proposition::Truth);
            let mut requirements = Vec::new();
            match location {
                "requirements" => requirements.push(definition),
                "conjunction" => {
                    axioms[0] = Proposition::Conjunction(vec![definition, Proposition::Truth])
                }
                "disjunction" => {
                    axioms[0] = Proposition::Disjunction(vec![definition, Proposition::Truth])
                }
                "implication" => {
                    axioms[0] = Proposition::Implication {
                        premise: Box::new(Proposition::Falsehood),
                        conclusion: Box::new(definition),
                    }
                }
                _ => unreachable!(),
            }
            let checked = check_predicate_denotations_with_value_equalities(
                &context,
                &goal,
                &requirements,
                &axioms,
            )
            .unwrap();
            assert!(
                checked
                    .check_certificate(&context, &citation(&checked, 3))
                    .is_err(),
                "{location}"
            );
        }
        let (context, _, _) = fixture();
        let goal = Proposition::Equal(value(3), literal(1));
        let checked =
            check_predicate_denotations_with_value_equalities(&context, &goal, &[], &[]).unwrap();
        assert!(
        checked
            .check_certificate(
                &context,
                &proof_node!(checked.goal().clone() => ProofRule::Primitive(PrimitiveJudgment::ReflexiveEquality))
            )
            .is_err(),
        "the goal cannot establish its own equality"
    );
    }

    #[test]
    fn wrong_carrier_and_changed_replay_context_reject() {
        let (context, goal, mut axioms) = fixture();
        let wrong = IntegerType::new(IntegerSign::Unsigned, 32).unwrap();
        axioms[0] = Proposition::Equal(
            value(3),
            ScalarTerm::integer(wrong, IntegerValue::Unsigned(1)).unwrap(),
        );
        assert!(matches!(
            check_predicate_denotations_with_value_equalities(&context, &goal, &[], &axioms),
            Err(PredicateDenotationError::Malformed(_))
        ));
        let (context, goal, axioms) = fixture();
        let checked =
            check_predicate_denotations_with_value_equalities(&context, &goal, &[], &axioms)
                .unwrap();
        let changed = PropositionContext::from_value_types(
            (1..=5).map(|identity| (ValueId::new(identity).unwrap(), ScalarType::Boolean)),
        )
        .unwrap();
        assert!(matches!(
            checked.check_certificate(&changed, &citation(&checked, 3)),
            Err(PredicateDenotationError::Malformed(_))
        ));
    }

    #[test]
    fn cyclic_definitions_reject_but_shared_acyclic_aliases_preserve_identity() {
        let (context, goal, mut axioms) = fixture();
        axioms[0] = Proposition::Equal(value(3), value(4));
        axioms[1] = Proposition::Equal(value(4), value(3));
        assert!(matches!(
            check_predicate_denotations_with_value_equalities(&context, &goal, &[], &axioms),
            Err(PredicateDenotationError::CyclicValueEquality)
        ));
        let goal = Proposition::Equal(value(4), sum(value(1), value(1)));
        let axioms = [
            Proposition::Equal(value(3), value(1)),
            Proposition::Equal(value(4), sum(value(3), value(3))),
        ];
        let checked =
            check_predicate_denotations_with_value_equalities(&context, &goal, &[], &axioms)
                .unwrap();
        checked
        .check_certificate(
            &context,
            &proof_node!(checked.goal().clone() => ProofRule::Primitive(PrimitiveJudgment::ReflexiveEquality)),
        )
        .unwrap();
    }

    #[test]
    fn shared_budget_bounds_input_depth_and_repeated_dag_expansion() {
        let context = value_context(ScalarType::Integer(integer_type()), 16);
        let goal = Proposition::Equal(value(16), literal(1));
        let axioms = (2..=16)
            .map(|identity| {
                Proposition::Equal(
                    value(identity),
                    sum(value(identity - 1), value(identity - 1)),
                )
            })
            .collect::<Vec<_>>();
        assert!(matches!(
            check_predicate_denotations_with_value_equalities(&context, &goal, &[], &axioms),
            Err(PredicateDenotationError::ResourceLimitExceeded)
        ));
        let inputs = vec![Proposition::Truth; 4096];
        assert!(matches!(
            check_predicate_denotations_with_value_equalities(
                &context,
                &Proposition::Truth,
                &[],
                &inputs
            ),
            Err(PredicateDenotationError::ResourceLimitExceeded)
        ));
        let mut deep = value(1);
        for _ in 0..64 {
            deep = sum(deep, literal(1));
        }
        assert!(matches!(
            check_predicate_denotations_with_value_equalities(
                &context,
                &Proposition::Equal(value(2), deep),
                &[],
                &[]
            ),
            Err(PredicateDenotationError::ResourceLimitExceeded)
        ));
    }

    #[test]
    fn mixed_integer_predicate_denotation_keeps_exact_value_carriers() {
        use semantic_vocabulary::IntegerMathTerm;
        let signed = IntegerType::new(IntegerSign::Signed, 32).unwrap();
        let unsigned = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
        let left_id = ValueId::new(1).unwrap();
        let right_id = ValueId::new(2).unwrap();
        let context = PropositionContext::from_value_types([
            (left_id, ScalarType::Integer(signed)),
            (right_id, ScalarType::Integer(unsigned)),
        ])
        .unwrap();
        let left = ScalarTerm::value(left_id, ScalarType::Integer(signed));
        let right = ScalarTerm::value(right_id, ScalarType::Integer(unsigned));
        let comparison = ScalarTerm::integer_less_than(signed, left, right).unwrap();
        let goal = Proposition::Equal(comparison, ScalarTerm::Boolean(true));
        let denotation = super::check_predicate_denotations(&context, &goal, &[], &[]).unwrap();
        assert_eq!(
            denotation.goal(),
            &Proposition::IntegerMathLessThan(
                IntegerMathTerm::MathValue {
                    source_type: signed,
                    value: left_id
                },
                IntegerMathTerm::MathValue {
                    source_type: unsigned,
                    value: right_id
                },
            )
        );
        let proof = assumption_proof(denotation.goal().clone(), 0);
        let premises = [goal.clone()];
        let denotation =
            super::check_predicate_denotations(&context, &goal, &premises, &[]).unwrap();
        denotation.check_certificate(&context, &proof).unwrap();
    }

    #[test]
    fn mixed_math_guard_substitutes_cited_fields_without_a_common_finite_carrier() {
        use semantic_vocabulary::{
            CanonicalStructuralPathSegment, IntegerMathTerm, PlaceId, StructuralFieldId,
            StructuralPlaceKind,
        };
        let signed = integer_type();
        let unsigned = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
        let context = PropositionContext::from_value_types_and_places(
            [
                (ValueId::new(1).unwrap(), ScalarType::Integer(signed)),
                (ValueId::new(2).unwrap(), ScalarType::Integer(unsigned)),
            ],
            [(
                PlaceId::new(1).unwrap(),
                StructuralPlaceKind::Parameter { position: 0 },
            )],
        )
        .unwrap();
        let field = ScalarTerm::integer_field_path(
            PlaceId::new(1).unwrap(),
            vec![CanonicalStructuralPathSegment::Field(
                StructuralFieldId::new(1).unwrap(),
            )],
            signed,
        );
        let right = ScalarTerm::value(ValueId::new(2).unwrap(), ScalarType::Integer(unsigned));
        let guard = Proposition::IntegerMathLessThan(
            IntegerMathTerm::MathValue {
                source_type: signed,
                value: ValueId::new(1).unwrap(),
            },
            IntegerMathTerm::MathValue {
                source_type: unsigned,
                value: ValueId::new(2).unwrap(),
            },
        );
        for bound in [12, u64::MAX as u128] {
            let equations = [
                Proposition::Equal(value(1), field.clone()),
                Proposition::Equal(
                    right.clone(),
                    ScalarTerm::integer(unsigned, IntegerValue::Unsigned(bound)).unwrap(),
                ),
            ];
            let denotation = check_value_equality_denotation(&context, &guard, &equations).unwrap();
            assert_eq!(
                denotation,
                Proposition::IntegerMathLessThan(
                    IntegerMathTerm::embed(field.clone()).unwrap(),
                    IntegerMathTerm::literal(IntegerValue::Unsigned(bound)),
                ),
            );
            let missing =
                check_value_equality_denotation(&context, &guard, &equations[..1]).unwrap();
            assert_ne!(missing, denotation);
        }
    }

    #[test]
    fn concatenated_live_length_sum_uses_only_cited_integer_equations() {
        use semantic_vocabulary::IntegerMathTerm;
        let (context, _, _) = fixture();
        let embedded = |identity| IntegerMathTerm::MathValue {
            source_type: integer_type(),
            value: ValueId::new(identity).unwrap(),
        };
        let goal = Proposition::IntegerMathLessOrEqual(
            IntegerMathTerm::Add(Box::new(embedded(1)), Box::new(embedded(2))),
            IntegerMathTerm::literal(IntegerValue::Signed(8)),
        );
        let equations = vec![
            Proposition::Equal(value(1), literal(3)),
            Proposition::Equal(value(2), literal(4)),
        ];
        let expected = Proposition::IntegerMathLessOrEqual(
            IntegerMathTerm::Add(
                Box::new(IntegerMathTerm::literal(IntegerValue::Signed(3))),
                Box::new(IntegerMathTerm::literal(IntegerValue::Signed(4))),
            ),
            IntegerMathTerm::literal(IntegerValue::Signed(8)),
        );
        assert_eq!(
            check_value_equality_denotation(&context, &goal, &equations).unwrap(),
            expected
        );
        assert_ne!(
            check_value_equality_denotation(&context, &goal, &equations[..1]).unwrap(),
            expected
        );
        let nested = vec![
            Proposition::Equal(
                value(1),
                ScalarTerm::exact_integer_add(integer_type(), value(3), literal(1)).unwrap(),
            ),
            Proposition::Equal(value(3), literal(2)),
            equations[1].clone(),
        ];
        let transported = check_value_equality_denotation(&context, &goal, &nested).unwrap();
        let expected_nested = Proposition::IntegerMathLessOrEqual(
            IntegerMathTerm::Add(
                Box::new(IntegerMathTerm::Add(
                    Box::new(IntegerMathTerm::literal(IntegerValue::Signed(2))),
                    Box::new(IntegerMathTerm::literal(IntegerValue::Signed(1))),
                )),
                Box::new(IntegerMathTerm::literal(IntegerValue::Signed(4))),
            ),
            IntegerMathTerm::literal(IntegerValue::Signed(8)),
        );
        assert_eq!(transported, expected_nested);
        crate::decide_primitive(
            &context,
            &transported,
            PrimitiveJudgment::ClosedIntegerRelation,
        )
        .unwrap();
        let wrapped = vec![
            Proposition::Equal(value(1), sum(value(3), literal(1))),
            equations[1].clone(),
        ];
        let transported = check_value_equality_denotation(&context, &goal, &wrapped).unwrap();
        let Proposition::IntegerMathLessOrEqual(IntegerMathTerm::Add(left, _), _) = transported
        else {
            panic!("exact mathematical sum")
        };
        assert_eq!(
            *left,
            IntegerMathTerm::embed(sum(value(3), literal(1))).unwrap(),
        );
    }

    #[test]
    fn mathematical_leaf_order_expands_only_cited_exact_scalar_definitions() {
        use semantic_vocabulary::IntegerMathTerm;
        let (context, _, _) = fixture();
        let embedded = |identity| IntegerMathTerm::MathValue {
            source_type: integer_type(),
            value: ValueId::new(identity).unwrap(),
        };
        let original = Proposition::IntegerMathLessOrEqual(
            embedded(1),
            IntegerMathTerm::literal(IntegerValue::Signed(100)),
        );
        let expected = Proposition::IntegerMathLessOrEqual(
            IntegerMathTerm::Add(Box::new(embedded(2)), Box::new(embedded(3))),
            IntegerMathTerm::literal(IntegerValue::Signed(100)),
        );
        let exact = Proposition::Equal(
            value(1),
            ScalarTerm::exact_integer_add(integer_type(), value(2), value(3)).unwrap(),
        );
        assert_eq!(
            check_value_equality_denotation(&context, &original, [&exact]).unwrap(),
            expected,
        );
        let transport = |equation: &Proposition| ProofNode {
            conclusion: expected.clone(),
            rule: ProofRule::ValueEqualityTransport {
                premise: Box::new(assumption_proof(original.clone(), 0)),
                equalities: vec![semantic_axiom_proof(equation.clone(), 0)],
            },
        };
        crate::accept_certificate(
            &context,
            &expected,
            std::slice::from_ref(&original),
            std::slice::from_ref(&exact),
            &transport(&exact),
        )
        .expect("exact scalar definition and mathematical order share checked denotation");

        for changed in [
            Proposition::Equal(
                value(1),
                ScalarTerm::exact_integer_add(integer_type(), value(2), value(4)).unwrap(),
            ),
            Proposition::Equal(
                value(1),
                ScalarTerm::wrapping_integer_add(integer_type(), value(2), value(3)).unwrap(),
            ),
            Proposition::Equal(
                value(1),
                ScalarTerm::saturating_integer_add(integer_type(), value(2), value(3)).unwrap(),
            ),
        ] {
            assert_ne!(
                check_value_equality_denotation(&context, &original, [&changed]).unwrap(),
                expected,
            );
            assert!(
                crate::accept_certificate(
                    &context,
                    &expected,
                    std::slice::from_ref(&original),
                    std::slice::from_ref(&changed),
                    &transport(&changed),
                )
                .is_err()
            );
        }
        assert!(
            crate::accept_certificate(
                &context,
                &expected,
                std::slice::from_ref(&original),
                &[],
                &transport(&exact),
            )
            .is_err()
        );
    }

    #[test]
    fn literal_ieee_comparison_stays_the_relation() {
        use semantic_vocabulary::{IeeeFloatComparisonKind as Kind, IeeeFloatValue};
        // Fact validity keeps a closed IEEE relation as that relation. These
        // literals are the cases a Boolean decision would erase: +0 compared
        // with -0, and a NaN compared with itself.
        let relations = [
            (
                IeeeFloatValue::Binary32(0),
                IeeeFloatValue::Binary32(0x8000_0000),
                Kind::Equal,
            ),
            (
                IeeeFloatValue::Binary64(0),
                IeeeFloatValue::Binary64(0x8000_0000_0000_0000),
                Kind::Equal,
            ),
            (
                IeeeFloatValue::Binary32(0x7fc0_0001),
                IeeeFloatValue::Binary32(0x7fc0_0001),
                Kind::Equal,
            ),
            (
                IeeeFloatValue::Binary64(0x7ff8_0000_0000_0001),
                IeeeFloatValue::Binary64(0x7ff8_0000_0000_0001),
                Kind::Equal,
            ),
            (
                IeeeFloatValue::Binary32(0x7fc0_0001),
                IeeeFloatValue::Binary32(0x7fc0_0001),
                Kind::NotLessThan,
            ),
        ];
        let context = PropositionContext::default();
        for (left_bits, right_bits, kind) in relations {
            let format = left_bits.format();
            let mut left = ScalarTerm::IeeeFloatLiteral(left_bits);
            let mut right = ScalarTerm::IeeeFloatLiteral(right_bits);
            if kind.is_symmetric() && left > right {
                std::mem::swap(&mut left, &mut right);
            }
            let goal = Proposition::ScalarIeeeFloatComparison {
                kind,
                format,
                left,
                right,
            };
            let converted = super::check_predicate_denotations(&context, &goal, &[], &[]).unwrap();
            assert_eq!(
                converted.goal(),
                &goal,
                "{format:?} {kind:?} must stay a scalar IEEE comparison"
            );
            let truth = ProofNode {
                conclusion: Proposition::Truth,
                rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
            };
            assert!(
                converted.check_certificate(&context, &truth).is_err(),
                "a proof of Truth must not discharge {format:?} {kind:?}"
            );
        }
        let integer = IntegerType::new(IntegerSign::Signed, 32).unwrap();
        let one = ScalarTerm::integer(integer, IntegerValue::Signed(1)).unwrap();
        let two = ScalarTerm::integer(integer, IntegerValue::Signed(2)).unwrap();
        let order = Proposition::LessThan(one, two);
        assert_eq!(
            super::check_predicate_denotations(&context, &order, &[], &[])
                .unwrap()
                .goal(),
            &Proposition::Truth,
            "a closed fixed-integer relation is still decided"
        );
    }
}
