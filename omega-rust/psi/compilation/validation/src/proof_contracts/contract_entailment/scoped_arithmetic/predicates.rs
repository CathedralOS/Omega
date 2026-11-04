//! Retain complete Boolean premises when comparing substituted integer facts.
//!
//! The inequality engine consumes conjunctive comparisons. A declared choice
//! such as `(width == 16 && lanes == 1) || (width == 32 && lanes == 2)` must
//! also remain available as a whole fact. Normalize each comparison under its
//! own exact value bindings, preserving every connective and correlation.
//! This checks equality of facts without enumerating combinations of branches.

use super::{Engine, Polynomial, ScopedArithmeticExpression, ScopedArithmeticHypothesis};
use crate::CheckingInput;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};

#[derive(PartialEq, Eq)]
enum Predicate {
    Boolean(bool),
    Comparison(BinaryOperator, Polynomial),
    And(Box<Self>, Box<Self>),
    Or(Box<Self>, Box<Self>),
}

pub(super) fn matches_asserted(
    program: &dyn CheckingInput,
    owner: symbols::SymbolHandle,
    hypotheses: &[ScopedArithmeticHypothesis],
    goal: &ScopedArithmeticExpression,
) -> bool {
    // Use a separate normalization context. Reading a candidate premise must
    // not change the ordinary bound judgment when it does not match.
    let mut engine = if let Some(machine) = program.machine(owner) {
        Engine::strict(program, machine, &[])
    } else {
        Engine::strict_for_declaration(program, owner)
    };
    let Some(goal) = scoped(program, &mut engine, goal) else {
        return false;
    };
    hypotheses.iter().any(|hypothesis| {
        hypothesis.holds
            && scoped(program, &mut engine, &hypothesis.proposition)
                .is_some_and(|premise| premise == goal)
    })
}

fn scoped(
    program: &dyn CheckingInput,
    engine: &mut Engine<'_>,
    expression: &ScopedArithmeticExpression,
) -> Option<Predicate> {
    engine.install_scoped_expression(expression).then_some(())?;
    predicate(program, engine, expression.expression, 0)
}

fn predicate(
    program: &dyn CheckingInput,
    engine: &mut Engine<'_>,
    expression: ExpressionHandle,
    depth: usize,
) -> Option<Predicate> {
    if depth >= 64 || !program.expression_table.expression_is_valid(expression) {
        return None;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Boolean(value) => Some(Predicate::Boolean(*value)),
        ExpressionNode::Binary(binary)
            if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or) =>
        {
            let left = Box::new(predicate(program, engine, binary.left, depth + 1)?);
            let right = Box::new(predicate(program, engine, binary.right, depth + 1)?);
            Some(if binary.operator == BinaryOperator::And {
                Predicate::And(left, right)
            } else {
                Predicate::Or(left, right)
            })
        }
        _ => {
            // The common comparison reader checks selected meaning and keeps
            // integer policies and exact captured identities during normalization.
            let (operator, left, right) = engine.comparison_polynomials(expression)?;
            Some(Predicate::Comparison(operator, left.sub(&right)))
        }
    }
}
