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
    let mut remaining = environment.clone();
    program
        .expression_table
        .match_arms(dispatch.arms)
        .iter()
        .map(|arm| {
            let mut selected = remaining.clone();
            if let MatchPattern::Value(pattern) = arm.pattern {
                let comparison = TableBinaryExpression {
                    left: dispatch.subject,
                    operator: BinaryOperator::Equal,
                    right: pattern,
                };
                guard_narrowing::narrow_environment_by_comparison(
                    program,
                    machine,
                    state,
                    &mut selected,
                    expression,
                    &comparison,
                    true,
                );
                guard_narrowing::narrow_environment_by_comparison(
                    program,
                    machine,
                    state,
                    &mut remaining,
                    expression,
                    &comparison,
                    false,
                );
            }
            match &writes {
                Some(writes) => selected.invalidate_call_paths(program, state, writes),
                None => selected.invalidate_unknown_writes(program, state),
            }
            selected
        })
        .collect()
}
