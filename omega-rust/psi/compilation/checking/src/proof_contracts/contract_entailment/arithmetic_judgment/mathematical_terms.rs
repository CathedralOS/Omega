//! Read the prepared calculus application, not an authored-arity body route.
//! Kernel beta substitution preserves its caller capture telescope. Scalar
//! facts consume only retained source operation laws; opaque assumptions and
//! kernel type conversion never establish a source value equation.

use super::{Engine, Judgment, Polynomial};
use crate::mathematical_applications::application_spine;
use crate::mathematical_applications::{PreparedConstantMeaning, PreparedMathematicalView};
use proof_admission::{Term, TermHandle};
use typed_trees::expression::{BinaryOperator, ExpressionHandle};

impl Engine<'_> {
    pub(in crate::proof_contracts::contract_entailment) fn judge_mathematical_application(
        &mut self,
        expression: ExpressionHandle,
    ) -> Judgment {
        if super::super::struct_trace() {
            eprintln!(
                "STRUCT mathematical expression={expression:?} prepared={}",
                self.program.prepared_mathematics().is_some()
            );
        }
        let Some(view) = self.program.prepared_mathematics() else {
            return unknown();
        };
        let Some(application) = view.application(expression) else {
            return unknown();
        };
        if super::super::struct_trace() {
            eprintln!(
                "STRUCT mathematical term={:?} captures={:?}",
                view.arena.get(application.term),
                view.applications
                    .captures
                    .span_or_empty(application.captures)
            );
        }
        let captures = view
            .applications
            .captures
            .span_or_empty(application.captures)
            .iter()
            .map(|capture| Some(capture.expression))
            .collect::<Vec<_>>();
        self.judge_mathematical_term(view, application.term, &captures, 0)
    }

    pub(super) fn collect_mathematical_comparisons(
        &mut self,
        expression: ExpressionHandle,
        comparisons: &mut Vec<(BinaryOperator, Polynomial, Polynomial)>,
    ) -> bool {
        let Some(view) = self.program.prepared_mathematics() else {
            return false;
        };
        let Some(application) = view.application(expression) else {
            return false;
        };
        let captures = view
            .applications
            .captures
            .span_or_empty(application.captures)
            .iter()
            .map(|capture| Some(capture.expression))
            .collect::<Vec<_>>();
        self.collect_mathematical_term(view, application.term, &captures, comparisons, 0)
    }

    pub(super) fn mathematical_comparison(
        &mut self,
        expression: ExpressionHandle,
    ) -> Option<(BinaryOperator, Polynomial, Polynomial)> {
        let view = self.program.prepared_mathematics()?;
        let application = view.application(expression)?;
        let captures = view
            .applications
            .captures
            .span_or_empty(application.captures)
            .iter()
            .map(|capture| Some(capture.expression))
            .collect::<Vec<_>>();
        self.term_comparison(view, application.term, &captures, 0)
    }

    pub(super) fn normalize_mathematical_application(
        &mut self,
        expression: ExpressionHandle,
    ) -> Option<Polynomial> {
        let view = self.program.prepared_mathematics()?;
        let application = view.application(expression)?;
        let captures = view
            .applications
            .captures
            .span_or_empty(application.captures)
            .iter()
            .map(|capture| Some(capture.expression))
            .collect::<Vec<_>>();
        self.normalize_mathematical_term(view, application.term, &captures, 0)
    }

    fn collect_mathematical_term(
        &mut self,
        view: PreparedMathematicalView<'_>,
        term: TermHandle,
        captures: &[Option<ExpressionHandle>],
        comparisons: &mut Vec<(BinaryOperator, Polynomial, Polynomial)>,
        depth: usize,
    ) -> bool {
        if depth >= 128 {
            return false;
        }
        match view.arena.get(term) {
            Term::Squash { ty } => {
                self.collect_mathematical_term(view, ty, captures, comparisons, depth + 1)
            }
            Term::Sigma { domain, codomain } if !matches!(view.arena.get(domain), Term::Two) => {
                let mut extended = captures.to_vec();
                extended.push(None);
                let left =
                    self.collect_mathematical_term(view, domain, captures, comparisons, depth + 1);
                let right = self.collect_mathematical_term(
                    view,
                    codomain,
                    &extended,
                    comparisons,
                    depth + 1,
                );
                left && right
            }
            _ => {
                if let Some(comparison) = self.term_comparison(view, term, captures, depth + 1) {
                    comparisons.push(comparison);
                    true
                } else {
                    false
                }
            }
        }
    }

    fn judge_mathematical_term(
        &mut self,
        view: PreparedMathematicalView<'_>,
        term: TermHandle,
        captures: &[Option<ExpressionHandle>],
        depth: usize,
    ) -> Judgment {
        if depth >= 128 {
            return unknown();
        }
        match view.arena.get(term) {
            Term::Squash { ty } => self.judge_mathematical_term(view, ty, captures, depth + 1),
            Term::Id { ty, left, right } if matches!(view.arena.get(ty), Term::Two) => {
                match (view.arena.get(left), view.arena.get(right)) {
                    (Term::TwoZero, Term::TwoZero) | (Term::TwoOne, Term::TwoOne) => {
                        Judgment::Proven
                    }
                    (Term::TwoZero, Term::TwoOne) | (Term::TwoOne, Term::TwoZero) => {
                        Judgment::ConstantFalse
                    }
                    _ => unknown(),
                }
            }
            Term::Id { ty, left, right }
                if self.is_identity_carrier(view, ty) && same_term(view, left, right, 0) =>
            {
                Judgment::Proven
            }
            Term::Sigma { domain, codomain } => {
                let mut extended = captures.to_vec();
                extended.push(None);
                if matches!(view.arena.get(domain), Term::Two) {
                    let Term::CaseTwo {
                        zero_branch,
                        one_branch,
                        ..
                    } = view.arena.get(codomain)
                    else {
                        return unknown();
                    };
                    alternative(
                        self.judge_mathematical_term(view, zero_branch, &extended, depth + 1),
                        self.judge_mathematical_term(view, one_branch, &extended, depth + 1),
                    )
                } else {
                    conjunction(
                        self.judge_mathematical_term(view, domain, captures, depth + 1),
                        self.judge_mathematical_term(view, codomain, &extended, depth + 1),
                    )
                }
            }
            _ => match self.term_comparison(view, term, captures, depth + 1) {
                Some((operator, left, right)) => self.judge_comparison(operator, left, right),
                None => unknown(),
            },
        }
    }

    fn is_identity_carrier(&self, view: PreparedMathematicalView<'_>, ty: TermHandle) -> bool {
        let Term::Constant { declaration, .. } = view.arena.get(ty) else {
            return false;
        };
        matches!(
            view.constant(declaration),
            Some(
                PreparedConstantMeaning::IntegerCarrier
                    | PreparedConstantMeaning::AddressCarrier
                    | PreparedConstantMeaning::BooleanCarrier
            )
        )
    }

    fn term_comparison(
        &mut self,
        view: PreparedMathematicalView<'_>,
        term: TermHandle,
        captures: &[Option<ExpressionHandle>],
        depth: usize,
    ) -> Option<(BinaryOperator, Polynomial, Polynomial)> {
        if depth >= 128 {
            return None;
        }
        if let Term::Squash { ty } = view.arena.get(term) {
            return self.term_comparison(view, ty, captures, depth + 1);
        }
        if let Term::Id { ty, left, right } = view.arena.get(term) {
            if !self.is_identity_carrier(view, ty) {
                return None;
            }
            return Some((
                BinaryOperator::Equal,
                self.normalize_mathematical_term(view, left, captures, depth + 1)?,
                self.normalize_mathematical_term(view, right, captures, depth + 1)?,
            ));
        }
        let (declaration, arguments) = application_spine(view, term)?;
        match (view.constant(declaration)?, arguments.as_slice()) {
            (PreparedConstantMeaning::IntegerRelation(operator), [left, right]) => Some((
                *operator,
                self.normalize_mathematical_term(view, *left, captures, depth + 1)?,
                self.normalize_mathematical_term(view, *right, captures, depth + 1)?,
            )),
            (PreparedConstantMeaning::Negation, [proposition]) => {
                let (operator, left, right) =
                    self.term_comparison(view, *proposition, captures, depth + 1)?;
                Some((operator.complement()?, left, right))
            }
            _ => None,
        }
    }

    fn normalize_mathematical_term(
        &mut self,
        view: PreparedMathematicalView<'_>,
        term: TermHandle,
        captures: &[Option<ExpressionHandle>],
        depth: usize,
    ) -> Option<Polynomial> {
        if depth >= 128 {
            return None;
        }
        if let Term::Variable(index) = view.arena.get(term) {
            let position = captures.len().checked_sub(index as usize + 1)?;
            let expression = captures[position]?;
            return self
                .normalize(expression)
                .or_else(|| self.normalize_integer_embedding(expression));
        }
        let (declaration, arguments) = application_spine(view, term)?;
        match (view.constant(declaration)?, arguments.as_slice()) {
            (PreparedConstantMeaning::Integer(value), []) => {
                Some(Polynomial::constant(value.clone()))
            }
            (PreparedConstantMeaning::AddressBound(symbol), [])
                if self.program.symbols.builtin_type_member(*symbol)
                    == Some(symbols::BuiltinTypeMember::AddressBound) =>
            {
                self.address_model_bound()
            }
            (PreparedConstantMeaning::IntegerOperation(operator), [left, right]) => {
                let left = self.normalize_mathematical_term(view, *left, captures, depth + 1)?;
                let right = self.normalize_mathematical_term(view, *right, captures, depth + 1)?;
                match operator {
                    BinaryOperator::Add => Some(left.add(&right)),
                    BinaryOperator::Subtract => Some(left.sub(&right)),
                    BinaryOperator::Multiply => left.checked_mul(&right),
                    _ => None,
                }
            }
            (
                PreparedConstantMeaning::IntegerEmbedding {
                    minimum, maximum, ..
                },
                [argument],
            ) => {
                let value =
                    self.normalize_mathematical_term(view, *argument, captures, depth + 1)?;
                self.bounds.push((value.clone(), minimum.clone()));
                if let Some(maximum) = maximum {
                    self.bounds.push((value.neg(), maximum.negate()));
                } else {
                    let bound = self.address_model_bound()?;
                    self.bounds
                        .push((bound.sub(&value), numerics::bignum::BigInt::from_i64(1)));
                }
                self.refresh_difference_bounds();
                Some(value)
            }
            _ => None,
        }
    }
}

fn same_term(
    view: PreparedMathematicalView<'_>,
    left: TermHandle,
    right: TermHandle,
    depth: usize,
) -> bool {
    if left == right {
        return true;
    }
    if depth >= 128 {
        return false;
    }
    match (view.arena.get(left), view.arena.get(right)) {
        (Term::Variable(left), Term::Variable(right)) => left == right,
        (
            Term::Constant {
                declaration: left,
                levels: left_levels,
            },
            Term::Constant {
                declaration: right,
                levels: right_levels,
            },
        ) => left == right && left_levels == right_levels,
        (
            Term::Apply {
                function: left,
                argument: left_argument,
            },
            Term::Apply {
                function: right,
                argument: right_argument,
            },
        ) => {
            same_term(view, left, right, depth + 1)
                && same_term(view, left_argument, right_argument, depth + 1)
        }
        _ => false,
    }
}

fn unknown() -> Judgment {
    Judgment::Unknown {
        goal_in_language: false,
    }
}
fn conjunction(left: Judgment, right: Judgment) -> Judgment {
    match (left, right) {
        (Judgment::ConstantFalse, _) | (_, Judgment::ConstantFalse) => Judgment::ConstantFalse,
        (Judgment::Refuted, _) | (_, Judgment::Refuted) => Judgment::Refuted,
        (Judgment::Proven, Judgment::Proven) => Judgment::Proven,
        _ => unknown(),
    }
}
fn alternative(left: Judgment, right: Judgment) -> Judgment {
    match (left, right) {
        (Judgment::Proven, _) | (_, Judgment::Proven) => Judgment::Proven,
        (
            Judgment::ConstantFalse | Judgment::Refuted,
            Judgment::ConstantFalse | Judgment::Refuted,
        ) => Judgment::Refuted,
        _ => unknown(),
    }
}
