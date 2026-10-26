//! Flow orderings retained by a match's common live operands.
//!
//! Runtime call results use the shared normal-value observation owner. Raw
//! signature substitution cannot relate a captured result to later storage.

use super::super::dispatch_arm_environments;
use super::{ExpressionHandle, ExpressionNode, Machine, State, ValueEnvironment};

/// The greatest floor of `left - arm` that every arm of a match on `right` proves.
pub(super) fn match_subtract_floor(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    environment: &ValueEnvironment,
    left: ExpressionHandle,
    right: ExpressionHandle,
    depth: usize,
) -> Option<i64> {
    let ExpressionNode::Match(dispatch) = program.expression_table.expression(right) else {
        return None;
    };
    let arms = program.expression_table.match_arms(dispatch.arms);
    if arms.is_empty() {
        return None;
    }
    let arm_environments =
        dispatch_arm_environments(program, machine, Some(state), right, dispatch, environment);
    if arm_environments.len() != arms.len() {
        return None;
    }
    let mut floor = None;
    for (arm, arm_environment) in arms.iter().zip(&arm_environments) {
        let arm_floor = super::subtract_floor_limited(
            program,
            machine,
            Some(state),
            arm_environment,
            left,
            arm.value,
            depth + 1,
        )?;
        floor = Some(floor.map_or(arm_floor, |existing: i64| existing.min(arm_floor)));
    }
    floor
}
