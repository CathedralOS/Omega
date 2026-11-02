//! One observation cut for predicates and content equations on completed values.
//!
//! The prefix owner supplies only dominating premises. The normal-return owner
//! then captures the selected expression in authored operand order, independently
//! of later siblings or statements. Consumers judge that captured value; they
//! cannot import a machine's eventual guarantees as premises at an earlier cut.

use std::ops::ControlFlow;

use super::structural_judgment::{StructuralJudge, StructuralTerm};
use super::structural_prefix::{
    PrefixObservation, StructuralPrefixSite, StructuralPrefixVisitor, observe_runtime_state_prefix,
};
use typed_trees::expression::ExpressionHandle;
use typed_trees::types::TypeReferenceHandle;
use typed_trees::{machine::Machine, state::State};

pub fn completed_value_is_bounded_by_static(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    expression: ExpressionHandle,
    binder: symbols::SymbolHandle,
    strict: bool,
) -> bool {
    let Some(parameter) = program
        .machine_type_parameters(machine)
        .iter()
        .find(|parameter| parameter.symbol == binder)
    else {
        return false;
    };
    let typed_trees::data::TypeParameterKind::Const {
        type_reference: bound_type,
    } = parameter.kind
    else {
        return false;
    };
    let Some(reference) =
        crate::expression_result_type_reference(program, machine, state, expression)
    else {
        return false;
    };
    let Some(primitive) = program
        .primitive_type_reference(reference)
        .filter(|primitive| primitive.is_fixed_integer())
    else {
        return false;
    };
    if program.primitive_type_reference(bound_type) != Some(primitive) {
        return false;
    }
    let result = fold_completed_values(
        program,
        machine,
        state,
        statement_index,
        expression,
        reference,
        true,
        |proven| *proven,
        |proven, observation| {
            let Some((judge, value)) = observation else {
                *proven = false;
                return ControlFlow::Break(());
            };
            let predicate = StructuralTerm::ScalarBinary {
                references: [reference, bound_type],
                operator: if strict {
                    typed_trees::expression::BinaryOperator::Less
                } else {
                    typed_trees::expression::BinaryOperator::LessOrEqual
                },
                meaning: [(primitive, numerics::arithmetic::ArithmeticDomain::Exact); 2],
                left: Box::new(value),
                right: Box::new(StructuralTerm::BoundValue(binder)),
            };
            *proven &= matches!(
                judge.judge_observed_predicate(predicate, 0),
                super::structural_judgment::StructuralJudgment::Proven
            );
            if *proven {
                ControlFlow::Continue(())
            } else {
                ControlFlow::Break(())
            }
        },
    );
    result.is_some_and(|result| result.complete && result.state)
}

/// Prove a finite receiving interval on the actual completed scalar value.
/// The goal contributes no premises; ordered child snapshots and checked call
/// guarantees come only from the normal-return owner at this occurrence.
pub(crate) fn completed_value_fits_integer_interval(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    expression: ExpressionHandle,
    interval: crate::proof_contracts::arithmetic_domains::Interval,
) -> bool {
    let Some(reference) =
        crate::expression_result_type_reference(program, machine, state, expression)
    else {
        return false;
    };
    let Some(primitive) = program
        .primitive_type_reference(reference)
        .filter(|p| p.is_fixed_integer())
    else {
        return false;
    };
    let result = fold_completed_values(
        program,
        machine,
        state,
        statement_index,
        expression,
        reference,
        true,
        |proven| *proven,
        |proven, observation| {
            let Some((judge, value)) = observation else {
                *proven = false;
                return ControlFlow::Break(());
            };
            for (bound, lower) in [(interval.low, true), (interval.high, false)] {
                let Some(bound) = bound else {
                    continue;
                };
                let bound = StructuralTerm::Integer(super::BigInt::from_i128(i128::from(bound)));
                let (left, right) = if lower {
                    (bound, value.clone())
                } else {
                    (value.clone(), bound)
                };
                let predicate = StructuralTerm::ScalarBinary {
                    references: [reference; 2],
                    operator: typed_trees::expression::BinaryOperator::LessOrEqual,
                    meaning: [(primitive, numerics::arithmetic::ArithmeticDomain::Exact); 2],
                    left: Box::new(left),
                    right: Box::new(right),
                };
                let judgment = judge.judge_observed_predicate(predicate.clone(), 0);
                if super::struct_trace() {
                    eprintln!(
                        "STRUCT completed interval expression={expression:?} predicate={predicate:?} judgment={judgment:?}"
                    );
                }
                *proven &= matches!(
                    judgment,
                    super::structural_judgment::StructuralJudgment::Proven
                );
                if !*proven {
                    return ControlFlow::Break(());
                }
            }
            ControlFlow::Continue(())
        },
    );
    if super::struct_trace() {
        eprintln!("STRUCT completed interval expression={expression:?} result={result:?}");
    }
    result.is_some_and(|result| result.complete && result.state)
}

/// A query-owned fold result. Partial universal observations may refute a
/// predicate, but only complete coverage may establish it.
#[derive(Debug)]
pub(super) struct CompletedValueFold<FoldState> {
    pub state: FoldState,
    pub complete: bool,
}

/// Fold completed values without publishing a tentative arrival's judgment.
/// Complete universal evidence avoids a private-arrival traversal. Otherwise
/// try a fresh closed-private batch and retain it only with complete coverage;
/// a failed private attempt leaves the saved universal result unchanged.
/// The fold owns every mutable result; it must not publish effects externally.
/// A missing observation is delivered as None. Breaking aborts the batch and
/// marks it incomplete, so an early failure can never establish a predicate.
#[allow(clippy::too_many_arguments)]
pub(super) fn fold_completed_values<FoldState: Clone>(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    expression: ExpressionHandle,
    reference: TypeReferenceHandle,
    initial: FoldState,
    succeeds: impl Fn(&FoldState) -> bool,
    fold: impl Fn(&mut FoldState, Option<(&StructuralJudge<'_>, StructuralTerm)>) -> ControlFlow<()>,
) -> Option<CompletedValueFold<FoldState>> {
    let mut visitor = CompletedObservation {
        site: StructuralPrefixSite {
            state: state.symbol,
            statement_index,
            in_transition_target: super::structural_prefix::expression_is_transition_target(
                program,
                state,
                statement_index,
                expression,
            ),
        },
        expression,
        reference,
        fold: &fold,
        succeeds: &succeeds,
        state: initial.clone(),
        complete: true,
        observed: false,
        halted: false,
    };
    let recognized = observe_runtime_state_prefix(program, machine, state, &mut visitor).is_some();
    let universal = recognized.then_some(CompletedValueFold {
        state: std::mem::replace(&mut visitor.state, initial),
        complete: visitor.complete && visitor.observed,
    });
    if universal
        .as_ref()
        .is_some_and(|result| result.complete && succeeds(&result.state))
    {
        return universal;
    }
    visitor.complete = true;
    visitor.observed = false;
    visitor.halted = false;
    if super::structural_prefix::observe_private_arrivals(program, machine, &mut visitor, false)
        == Some(true)
        && visitor.complete
        && visitor.observed
    {
        return Some(CompletedValueFold {
            state: visitor.state,
            complete: true,
        });
    }
    universal
}

#[derive(Clone)]
struct CompletedObservation<FoldState, Fold, Succeeds> {
    site: StructuralPrefixSite,
    expression: ExpressionHandle,
    reference: TypeReferenceHandle,
    fold: Fold,
    succeeds: Succeeds,
    state: FoldState,
    complete: bool,
    observed: bool,
    halted: bool,
}

impl<FoldState, Fold, Succeeds> CompletedObservation<FoldState, Fold, Succeeds>
where
    Fold: Fn(&mut FoldState, Option<(&StructuralJudge<'_>, StructuralTerm)>) -> ControlFlow<()>,
{
    fn unavailable(&mut self) {
        self.complete = false;
        if (self.fold)(&mut self.state, None).is_break() {
            self.halted = true;
        }
    }
}

impl<FoldState, Fold, Succeeds> StructuralPrefixVisitor
    for CompletedObservation<FoldState, Fold, Succeeds>
where
    Fold: Fn(&mut FoldState, Option<(&StructuralJudge<'_>, StructuralTerm)>) -> ControlFlow<()>,
    Succeeds: Fn(&FoldState) -> bool,
{
    fn has_positive_proof(&self) -> bool {
        self.complete && self.observed && !self.halted && (self.succeeds)(&self.state)
    }

    fn site(&self) -> StructuralPrefixSite {
        self.site
    }

    fn is_complete(&self) -> bool {
        self.halted
    }

    fn unresolved(&mut self, _machine: &Machine) {
        self.unavailable();
    }

    fn observe<'program>(&mut self, observation: PrefixObservation<'_, 'program>) {
        let (_, machine, state, judge, environment) = observation;
        let Some(outcomes) = judge.completed_statement_paths(
            self.site.statement_index,
            self.expression,
            environment,
            (machine, state, self.reference),
        ) else {
            self.unavailable();
            return;
        };
        if outcomes.is_empty() {
            self.unavailable();
            return;
        }
        self.observed = true;
        for outcome in outcomes {
            if (self.fold)(
                &mut self.state,
                Some((
                    &outcome.judge,
                    outcome.judge.completed_value_identity(outcome.value),
                )),
            )
            .is_break()
            {
                self.halted = true;
                self.complete = false;
                break;
            }
        }
    }
}
