//! One observation cut for predicates and content equations on completed values.
//!
//! The prefix owner supplies only dominating premises. The normal-return owner
//! then captures the selected expression in authored operand order, independently
//! of later siblings or statements. Consumers judge that captured value; they
//! cannot import a machine's eventual guarantees as premises at an earlier cut.

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
    let mut proven = true;
    let complete = observe_completed_values(
        program,
        machine,
        state,
        statement_index,
        expression,
        reference,
        |judge, value| {
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
            proven &= matches!(
                judge.judge_observed_predicate(predicate, 0),
                super::structural_judgment::StructuralJudgment::Proven
            );
        },
    );
    complete == Some(true) && proven
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
    let mut proven = true;
    let complete = observe_completed_values(
        program,
        machine,
        state,
        statement_index,
        expression,
        reference,
        |judge, value| {
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
                proven &= matches!(
                    judgment,
                    super::structural_judgment::StructuralJudgment::Proven
                );
            }
        },
    );
    if super::struct_trace() {
        eprintln!(
            "STRUCT completed interval expression={expression:?} coverage={complete:?} proven={proven}"
        );
    }
    complete == Some(true) && proven
}

/// Visit every reachable completion. None means the prefix was unavailable;
/// false means that the callbacks do not cover all reachable completions.
/// Partial observations may refute a predicate, but cannot establish one.
pub(super) fn observe_completed_values(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    expression: ExpressionHandle,
    reference: TypeReferenceHandle,
    observe: impl FnMut(&StructuralJudge<'_>, StructuralTerm),
) -> Option<bool> {
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
        observe,
        complete: true,
        observed: false,
    };
    observe_runtime_state_prefix(program, machine, state, &mut visitor)?;
    Some(visitor.complete && visitor.observed)
}

struct CompletedObservation<Observer> {
    site: StructuralPrefixSite,
    expression: ExpressionHandle,
    reference: TypeReferenceHandle,
    observe: Observer,
    complete: bool,
    observed: bool,
}

impl<Observer> StructuralPrefixVisitor for CompletedObservation<Observer>
where
    Observer: FnMut(&StructuralJudge<'_>, StructuralTerm),
{
    fn site(&self) -> StructuralPrefixSite {
        self.site
    }

    fn unresolved(&mut self, _machine: &Machine) {
        self.complete = false;
    }

    fn observe<'program>(&mut self, observation: PrefixObservation<'_, 'program>) {
        let (_, machine, state, judge, environment) = observation;
        let Some(outcomes) = judge.completed_statement_paths(
            self.site.statement_index,
            self.expression,
            environment,
            (machine, state, self.reference),
        ) else {
            self.complete = false;
            return;
        };
        self.observed |= !outcomes.is_empty();
        for outcome in outcomes {
            (self.observe)(
                &outcome.judge,
                outcome.judge.completed_value_identity(outcome.value),
            );
        }
    }
}
