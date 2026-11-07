//! Unit completion retains a selected invocation graph without result storage.
//! Every leaf uses the same ordinary call transaction as a transition arm.

use super::{CheckFacts, ScalarCalleeClosure, ShapeCollector, control};
use checked_trees::{
    CheckedCompletionExpressionKind as Kind, CheckedConditionalReturnArm as Arm,
    CheckedControlResultPlan, CheckedUnitCompletionExpression, CheckedUnitCompletionKind,
    CheckedUnitCompletionPlan, CheckedUnitCompletionSelection, CheckedUnitEffectOperationPlan,
    CheckedUnitScalarCompletionArm, CheckedUnitScalarCompletionCondition,
    CheckedUnitStructuralCompletionArm, CheckedUnitStructuralParameterPlan,
};
use std::collections::BTreeSet;

#[allow(clippy::too_many_arguments)]
pub(super) fn build(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    scalar_callees: ScalarCalleeClosure,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    structural: &[CheckedUnitStructuralParameterPlan],
    entry_claims: &[checked_trees::CheckedUnitEntryClaimPlan],
    local_results: &[(
        checked_trees::CheckedUnitStructuralResultBindingPlan,
        facts::PlaceRoot,
    )],
    preamble: &[CheckedUnitEffectOperationPlan],
    statement: u32,
    expression: typed_trees::expression::ExpressionHandle,
    trace: &control::LocalConstructionTrace,
) -> Option<CheckedUnitCompletionPlan> {
    let plans = &facts.values.completions;
    let root = plans.root_at(machine.symbol, state.symbol, statement, expression)?;
    if !matches!(plans.nodes.get(root.root).kind, Kind::Dispatch { .. }) {
        return None;
    }
    let flow = super::super::types::state_flow(facts, machine.symbol, state.symbol)?;
    let call_roster = facts.flow.control.calls.span_or_empty(flow.calls);
    let mut bind_call =
        |node: &checked_trees::CheckedCompletionExpression,
         source_call: arena::Handle<checked_trees::FlowCallFact>| {
            let offset = source_call
                .arena_index()
                .checked_sub(flow.calls.start().arena_index())?;
            if flow.calls.handles().nth(offset as usize) != Some(source_call) {
                return None;
            }
            let call = facts.flow.control.calls.get(source_call);
            if call.statement_index != statement as usize
                || call.authored_expression != node.expression
            {
                return None;
            }
            let arm = super::terminal_calls::plan_call_arm(
                program,
                facts,
                scalar_callees,
                shapes,
                machine,
                state,
                structural,
                entry_claims,
                local_results,
                preamble,
                &CheckedControlResultPlan::Unit,
                0,
                call_roster,
                call,
                call.target_symbol,
                statement,
                trace,
            )?;
            let operations = match arm {
                Arm::EvaluateNoNormal { operations, .. } => operations,
                Arm::Call {
                    operands,
                    operation,
                    completions,
                } => {
                    if !matches!(
                        operation.call()?.result(),
                        checked_trees::CheckedUnitCallResult::Unit
                            | checked_trees::CheckedUnitCallResult::NoNormal
                    ) {
                        return None;
                    }
                    checked_trees::CheckedOperandSequence::new(
                        operands
                            .iter()
                            .chain(std::iter::once(&operation))
                            .chain(completions.iter())
                            .cloned(),
                    )
                }
                _ => return None,
            };
            Some(operations)
        };
    let expression = bind_expression(plans, root.root, &mut BTreeSet::new(), &mut bind_call)?;
    Some(CheckedUnitCompletionPlan {
        statement_ordinal: statement,
        expression,
        // A union of leaf outcomes supplies no abandonment proof for earlier
        // subject or pattern execution. Each call retains its exact outcome.
        abandons_custody: false,
    })
}

fn bind_expression(
    plans: &checked_trees::CheckedCompletionPlans,
    handle: checked_trees::CheckedCompletionExpressionHandle,
    active: &mut BTreeSet<(u32, u32)>,
    bind_call: &mut impl FnMut(
        &checked_trees::CheckedCompletionExpression,
        arena::Handle<checked_trees::FlowCallFact>,
    ) -> Option<checked_trees::CheckedOperandSequence>,
) -> Option<CheckedUnitCompletionExpression> {
    let key = (handle.arena_index(), handle.generation());
    if !handle.is_valid() || !active.insert(key) {
        return None;
    }
    let node = plans.nodes.get(handle);
    let kind = match &node.kind {
        Kind::Missing => return None,
        Kind::Invocation { source_call } => {
            CheckedUnitCompletionKind::Invocation(bind_call(node, *source_call)?)
        }
        Kind::Dispatch { subject, arms } => {
            let arms = plans.arms.span(*arms)?;
            if arms.is_empty() {
                return None;
            }
            match subject {
                checked_trees::CheckedDispatchSubject::Scalar(subject) => {
                    let mut selected = Vec::with_capacity(arms.len());
                    for arm in arms {
                        use checked_trees::{
                            CheckedCompletionComparison as Comparison,
                            CheckedScalarDispatchPattern as Pattern,
                        };
                        let condition = match (&arm.pattern, arm.comparison) {
                            (Pattern::Value(value), Comparison::Boolean) => {
                                CheckedUnitScalarCompletionCondition::Boolean(*value)
                            }
                            (Pattern::Value(value), Comparison::Integer) => {
                                CheckedUnitScalarCompletionCondition::Integer(*value)
                            }
                            (Pattern::Predicate(value), Comparison::Predicate) => {
                                CheckedUnitScalarCompletionCondition::Predicate(*value)
                            }
                            (Pattern::Wildcard, Comparison::Predicate) => {
                                CheckedUnitScalarCompletionCondition::Default
                            }
                            _ => return None,
                        };
                        selected.push(CheckedUnitScalarCompletionArm {
                            source_arm: arm.source_arm,
                            condition,
                            completion: bind_expression(plans, arm.completion, active, bind_call)?,
                        });
                    }
                    CheckedUnitCompletionKind::Selection(CheckedUnitCompletionSelection::Scalar {
                        subject: *subject,
                        arms: selected,
                    })
                }
                checked_trees::CheckedDispatchSubject::Structural {
                    value,
                    place,
                    alias,
                } => {
                    let mut selected = Vec::with_capacity(arms.len());
                    for arm in arms {
                        let predicate = match (&arm.pattern, arm.comparison) {
                            (
                                checked_trees::CheckedScalarDispatchPattern::Predicate(value),
                                checked_trees::CheckedCompletionComparison::Predicate,
                            ) => Some(*value),
                            (
                                checked_trees::CheckedScalarDispatchPattern::Wildcard,
                                checked_trees::CheckedCompletionComparison::Predicate,
                            ) => None,
                            _ => return None,
                        };
                        selected.push(CheckedUnitStructuralCompletionArm {
                            source_arm: arm.source_arm,
                            predicate,
                            completion: bind_expression(plans, arm.completion, active, bind_call)?,
                        });
                    }
                    CheckedUnitCompletionKind::Selection(
                        CheckedUnitCompletionSelection::Structural {
                            value: *value,
                            place: place.clone(),
                            alias: *alias,
                            arms: selected,
                        },
                    )
                }
            }
        }
    };
    active.remove(&key);
    Some(CheckedUnitCompletionExpression {
        source: handle,
        expression: node.expression,
        normal_continuation: node.normal_continuation,
        kind,
    })
}
