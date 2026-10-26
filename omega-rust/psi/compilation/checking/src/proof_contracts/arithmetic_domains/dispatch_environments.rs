//! Branch facts for every consumer of a value dispatch.
//!
//! A cast or call inside an arm sees the selected pattern's facts, not the
//! outer expression's environment. Earlier pattern failures narrow later arms.
//! Calls may invalidate those facts, so the saved subject never restores a
//! predicate about a place that the dispatch can mutate.

use super::{Machine, State, ValueEnvironment, guard_narrowing};
use crate::machine_calls::calls::{CallFrameResolver, expression_has_calls};
use typed_trees::expression::{
    BinaryOperator, ExpressionHandle, MatchPattern, TableBinaryExpression, TableMatchExpression,
};

/// One selected or failed authored pattern. This invocation-local observation
/// borrows the original operands; it manufactures no expression or proof fact.
#[derive(Clone, Copy)]
pub(crate) struct DispatchSelection {
    pub(crate) expression: ExpressionHandle,
    pub(crate) condition: DispatchCondition,
    pub(crate) matched: bool,
}

#[derive(Clone, Copy)]
pub(crate) enum DispatchCondition {
    Equality {
        subject: ExpressionHandle,
        pattern: ExpressionHandle,
    },
    Predicate(ExpressionHandle),
}

/// Interpret patterns in order. A pattern forms under preceding failures,
/// before its equality can refine either its own arm or the remaining paths.
/// Consumers own subject formation and the authority to preserve observations.
pub(crate) fn dispatch_arm_contexts<Context: Clone>(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    dispatch: &TableMatchExpression,
    context: &Context,
    mut form_pattern: impl FnMut(&Context, ExpressionHandle) -> bool,
    mut select: impl FnMut(&mut Context, DispatchSelection),
) -> Vec<Context> {
    let mut remaining = context.clone();
    program
        .expression_table
        .match_arms(dispatch.arms)
        .iter()
        .map(|arm| {
            let mut selected = remaining.clone();
            let condition = match arm.pattern {
                MatchPattern::Value(pattern) if form_pattern(&remaining, pattern) => {
                    Some(DispatchCondition::Equality {
                        subject: dispatch.subject,
                        pattern,
                    })
                }
                MatchPattern::Destructure(pattern)
                    if form_pattern(&remaining, pattern.condition) =>
                {
                    Some(DispatchCondition::Predicate(pattern.condition))
                }
                _ => None,
            };
            if let Some(condition) = condition {
                select(
                    &mut selected,
                    DispatchSelection {
                        expression,
                        condition,
                        matched: true,
                    },
                );
                select(
                    &mut remaining,
                    DispatchSelection {
                        expression,
                        condition,
                        matched: false,
                    },
                );
            }
            selected
        })
        .collect()
}

pub(crate) fn dispatch_arm_environments(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
    dispatch: &TableMatchExpression,
    environment: &ValueEnvironment,
) -> Vec<ValueEnvironment> {
    let writes = if expression_has_calls(program, expression) {
        CallFrameResolver::new(program)
            .and_then(|frames| frames.expression_may_write_paths(machine, expression))
    } else {
        Some(Vec::new())
    };
    dispatch_arm_contexts(
        program,
        expression,
        dispatch,
        environment,
        |_, _| true,
        |selected, selection| {
            match selection.condition {
                DispatchCondition::Equality { subject, pattern } => {
                    let comparison = TableBinaryExpression {
                        left: subject,
                        operator: BinaryOperator::Equal,
                        right: pattern,
                    };
                    guard_narrowing::narrow_environment_by_comparison(
                        program,
                        machine,
                        state,
                        selected,
                        expression,
                        &comparison,
                        selection.matched,
                    );
                }
                DispatchCondition::Predicate(condition) => {
                    guard_narrowing::narrow_environment_by_condition(
                        program,
                        machine,
                        state,
                        selected,
                        condition,
                        selection.matched,
                    );
                }
            }
            // The captured value and its source agree until a write changes
            // the source. Rebind the predicate's captured paths now; the full
            // dispatch write frame below then withdraws every unstable source
            // consequence before an arm can consume it. Capture paths retain
            // their own identity and never become global source aliases.
            if dispatch.capture.is_valid()
                && let Some(source) = super::place_path(program, dispatch.subject)
            {
                let anchor = program
                    .expression_table
                    .value_capture(dispatch.capture)
                    .value_anchor;
                selected.copy_path_facts(&super::place_paths::capture_path(anchor), &source);
            }
        },
    )
    .into_iter()
    .map(|mut selected| {
        match &writes {
            Some(writes) => selected.invalidate_call_paths(program, state, writes),
            None => selected.invalidate_unknown_writes(program, state),
        }
        selected
    })
    .collect()
}
