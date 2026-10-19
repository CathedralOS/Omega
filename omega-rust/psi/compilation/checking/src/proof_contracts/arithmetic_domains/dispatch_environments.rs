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
    pub(crate) subject: ExpressionHandle,
    pub(crate) pattern: ExpressionHandle,
    pub(crate) matched: bool,
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
            if let MatchPattern::Value(pattern) = arm.pattern
                && form_pattern(&remaining, pattern)
            {
                select(
                    &mut selected,
                    DispatchSelection {
                        expression,
                        subject: dispatch.subject,
                        pattern,
                        matched: true,
                    },
                );
                select(
                    &mut remaining,
                    DispatchSelection {
                        expression,
                        subject: dispatch.subject,
                        pattern,
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
            let comparison = TableBinaryExpression {
                left: selection.subject,
                operator: BinaryOperator::Equal,
                right: selection.pattern,
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
