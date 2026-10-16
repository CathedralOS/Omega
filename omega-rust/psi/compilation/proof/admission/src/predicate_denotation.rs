//! Checked, non-serialized predicate conversion and optional value-equality transport.
//!
//! Conversion changes no premise's authority. The original inputs remain
//! borrowed, and only this owner can construct their equivalent proof views.
//! Closed fixed-integer comparisons share the primitive judgment's literal
//! order. This lets ordinary checked transitivity expose contradictory bounds
//! without adding a second interval checker or treating a failed proof as false.

use semantic_vocabulary::{
    Proposition, PropositionContext, PropositionError, ScalarTerm, ScalarType,
};

use crate::{ProofError, ProofNode, check_certificate};

pub(crate) use semantic_vocabulary::{scalar_term_binary_operands, scalar_term_unary_operand};

mod budget;
mod value_equalities;
use budget::Budget;

/// Bound a complete predicate input roster before recursive validation,
/// substitution, or cloning. This establishes neither formation nor truth;
/// arithmetic evaluation must separately bound intermediate integer growth.
pub fn check_predicate_evaluation_size<'input>(
    propositions: impl IntoIterator<Item = &'input Proposition>,
) -> Result<(), PredicateDenotationError> {
    let mut budget = Budget::new();
    for proposition in propositions {
        budget.proposition(proposition, 0)?;
    }
    Ok(())
}

#[cfg(test)]
mod value_equality_tests;

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

/// Check predicate conversion under unconditional, typed value equalities from
/// the original semantic premises. This is contextual equality transport, not
/// arithmetic simplification: constructors and operand order remain exact.
/// The original question and premise roster remain authoritative and borrowed.
/// Field equations name the current storage observation, not a timeless value;
/// callers supply only the live roster after store/call invalidation. Exact
/// IEEE bit equations do not assert IEEE comparison equality.
pub fn check_predicate_denotations_with_value_equalities<'input>(
    context: &PropositionContext,
    goal: &'input Proposition,
    requirements: &'input [Proposition],
    semantic_axioms: &'input [Proposition],
) -> Result<CheckedPredicateDenotations<'input>, PredicateDenotationError> {
    let mut budget = Budget::new();
    for proposition in std::iter::once(goal)
        .chain(requirements)
        .chain(semantic_axioms)
    {
        budget.proposition(proposition, 0)?;
        context
            .validate(proposition)
            .map_err(PredicateDenotationError::Malformed)?;
    }
    let equalities = value_equalities::ValueEqualities::from_semantic_axioms(semantic_axioms);
    let mut convert = |proposition: &Proposition| {
        transport_denotation(context, proposition, &equalities, &mut budget)
    };
    let normalized_goal = convert(goal)?;
    let normalized_requirements = requirements
        .iter()
        .map(&mut convert)
        .collect::<Result<Vec<_>, PredicateDenotationError>>()?;
    let normalized_axioms = semantic_axioms
        .iter()
        .map(&mut convert)
        .collect::<Result<Vec<_>, PredicateDenotationError>>()?;
    Ok(CheckedPredicateDenotations {
        original_goal: goal,
        original_requirements: requirements,
        original_semantic_axioms: semantic_axioms,
        goal: normalized_goal,
        requirements: normalized_requirements,
        semantic_axioms: normalized_axioms,
    })
}

/// Compute an equivalent denotation under explicitly supplied value equations.
/// The caller must independently prove every equation; this operation grants
/// no evidence authority and does not rewrite any ambient premise roster.
/// Input traversal is charged before validation, retaining references, or
/// cloning terms. One shared budget bounds all equations and transported output.
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
    if let Some(holds) = closed_ieee_float_relation(proposition) {
        return Ok(if holds {
            Proposition::Truth
        } else {
            Proposition::Falsehood
        });
    }
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
        // Open integer relations, arithmetic, IEEE comparisons, content and declared atoms
        // remain exact observations; this conversion adds no laws for them.
        other => Ok(other.clone()),
    }
}

fn closed_integer_relation(proposition: &Proposition) -> Option<Proposition> {
    let (left, right) = match proposition {
        semantic_vocabulary::ordered_comparison_operands!(left, right) => (left, right),
        _ => return None,
    };
    // integer_value(), used by the primitive comparator, can also evaluate
    // closed arithmetic. This conversion intentionally admits literal pairs
    // only; retain every arithmetic constructor for its existing proof route.
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

/// Decide only exact, same-format literals. Open IEEE relations remain atoms;
/// in particular `x == x` is not reflexive in the presence of NaN. Decoding
/// interchange bits is not a host arithmetic operation: the shared semantic
/// catalog compares their exact finite rationals and explicit special values.
fn closed_ieee_float_relation(proposition: &Proposition) -> Option<bool> {
    use numerics::float_semantics::{FloatMeaning, FloatSemantics};
    use semantic_vocabulary::{IeeeFloatComparisonKind as Kind, IeeeFloatValue};
    let Proposition::ScalarIeeeFloatComparison {
        kind,
        format,
        left,
        right,
    } = proposition
    else {
        return None;
    };
    let (ScalarTerm::IeeeFloatLiteral(left), ScalarTerm::IeeeFloatLiteral(right)) = (left, right)
    else {
        return None;
    };
    if left.format() != *format || right.format() != *format {
        return None;
    }
    let meaning = |value| match value {
        IeeeFloatValue::Binary32(bits) => FloatMeaning::from_f32(f32::from_bits(bits)),
        IeeeFloatValue::Binary64(bits) => FloatMeaning::from_f64(f64::from_bits(bits)),
    };
    let (left, right) = (meaning(*left), meaning(*right));
    Some(match kind {
        Kind::Equal => FloatSemantics::equal(&left, &right),
        Kind::NotEqual => FloatSemantics::not_equal(&left, &right),
        Kind::LessThan => FloatSemantics::less(&left, &right),
        Kind::LessOrEqual => FloatSemantics::less_or_equal(&left, &right),
        Kind::NotLessThan => !FloatSemantics::less(&left, &right),
        Kind::NotLessOrEqual => !FloatSemantics::less_or_equal(&left, &right),
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
            // A compound endpoint is still typed, but the existing
            // mathematical vocabulary cannot replace its identity.
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

/// A closed endpoint can change carrier only when its mathematical integer is
/// representable there. No conversion is applied to an unknown endpoint.
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
