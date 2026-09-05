//! Argument-local range premises follow the authored transition schedule.

use super::*;
use crate::calls::CallFrameResolver;
use psi_typed_trees::statement::{StatementNode, TransitionGuardNode};

#[cfg(test)]
mod tests;

#[derive(Default)]
pub(crate) struct TransitionArgumentEnvironments {
    targets: Vec<(TransitionTargetHandle, Vec<ValueEnv>)>,
}

impl TransitionArgumentEnvironments {
    pub(crate) fn collect<'program>(
        program: &'program TypedTrees,
        machine: &'program Machine,
        state: &State,
        statement: &StatementNode,
        before: &ValueEnv,
        frames: Option<&CallFrameResolver<'program>>,
    ) -> Self {
        let StatementNode::Transition(transition) = statement else {
            return Self::default();
        };
        let mut result = Self::default();
        for (positive, target) in [(true, transition.target), (false, transition.continuation)] {
            if !target.is_valid() {
                continue;
            }
            let TransitionTargetNode::Named { arguments, .. } =
                program.statement_table.transition_target(target)
            else {
                continue;
            };
            let mut current = if positive {
                crate::arithmetic_domains::guard_narrowed_env(
                    program,
                    machine,
                    Some(state),
                    &transition.guard,
                    before,
                )
            } else {
                // The continuation is the false sibling, not execution after
                // the primary target. Neither sibling inherits the other's calls.
                crate::arithmetic_domains::fall_through_narrowed_env(
                    program,
                    machine,
                    Some(state),
                    &transition.guard,
                    before,
                )
            };
            if let TransitionGuardNode::When(guard) = transition.guard {
                // Guard effects reach both siblings. Its result cannot establish
                // an old bound on storage changed by a later guard child.
                cross_expression_effects(&mut current, machine, guard, frames);
            }
            let mut environments = Vec::new();
            for argument in program.statement_table.expression_handles(*arguments) {
                // Within one argument we remain conservative about nested calls;
                // later arguments never invalidate an already evaluated value.
                cross_expression_effects(&mut current, machine, *argument, frames);
                environments.push(current.clone());
            }
            result.targets.push((target, environments));
        }
        result
    }

    pub(crate) fn for_target(&self, target: TransitionTargetHandle) -> &[ValueEnv] {
        self.targets
            .iter()
            .find(|(candidate, _)| *candidate == target)
            .map_or(&[], |(_, environments)| environments.as_slice())
    }
}

fn cross_expression_effects<'program>(
    environment: &mut ValueEnv,
    machine: &'program Machine,
    expression: ExpressionHandle,
    frames: Option<&CallFrameResolver<'program>>,
) {
    if let Some(written) =
        frames.and_then(|frames| frames.expression_may_write_paths(machine, expression))
    {
        environment.invalidate_written_paths(&written);
    } else {
        environment.clear();
    }
}
