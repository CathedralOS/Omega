//! Target-local range premises follow the authored transition schedule.
use super::{
    ExpressionHandle, Machine, State, TransitionTargetHandle, TransitionTargetNode,
    ValueEnvironment,
};
use crate::machine_calls::calls::{CallFrameResolver, expression_has_calls};
use typed_trees::statement::StatementNode;

#[derive(Default)]
pub(crate) struct TransitionValueEnvironments {
    targets: Vec<(TransitionTargetHandle, Vec<ValueEnvironment>)>,
}

impl TransitionValueEnvironments {
    pub(crate) fn collect<'program>(
        program: &'program dyn crate::CheckingInput,
        machine: &'program Machine,
        state: &State,
        statement: &StatementNode,
        before: &ValueEnvironment,
        frames: Option<&CallFrameResolver<'program>>,
    ) -> Self {
        let Some(transition) = statement.as_transition() else {
            return Self::default();
        };
        let mut result = Self::default();
        for (positive, target) in [(true, transition.target), (false, transition.continuation)] {
            if !target.is_valid() {
                continue;
            }
            let expressions = match program.statement_table.transition_target(target) {
                TransitionTargetNode::Named { arguments, .. } => {
                    program.statement_table.expression_handles(*arguments)
                }
                TransitionTargetNode::Value(expression) => std::slice::from_ref(expression),
                TransitionTargetNode::Terminal => continue,
            };
            let mut current = if positive {
                crate::proof_contracts::arithmetic_domains::guard_narrowed_environment(
                    program,
                    machine,
                    Some(state),
                    &transition.guard,
                    before,
                )
            } else {
                // The continuation is the false sibling, not execution after
                // the primary target. Neither sibling inherits the other's calls.
                crate::proof_contracts::arithmetic_domains::fall_through_narrowed_environment(
                    program,
                    machine,
                    Some(state),
                    &transition.guard,
                    before,
                )
            };
            // The guard owner already crossed its effects in operand order.
            // Reapplying its aggregate write frame would erase a comparison
            // made after the last write.
            let mut environments = Vec::new();
            for expression in expressions {
                // Within one value we remain conservative about nested calls;
                // later arguments never invalidate an already evaluated value.
                cross_expression_effects(
                    &mut current,
                    program,
                    machine,
                    state,
                    *expression,
                    frames,
                );
                environments.push(current.clone());
            }
            result.targets.push((target, environments));
        }
        result
    }

    pub(crate) fn for_target(&self, target: TransitionTargetHandle) -> &[ValueEnvironment] {
        self.targets
            .iter()
            .find(|(candidate, _)| *candidate == target)
            .map_or(&[], |(_, environments)| environments.as_slice())
    }
}

fn cross_expression_effects<'program>(
    environment: &mut ValueEnvironment,
    program: &'program dyn crate::CheckingInput,
    machine: &'program Machine,
    state: &State,
    expression: ExpressionHandle,
    frames: Option<&CallFrameResolver<'program>>,
) {
    if let Some(written) =
        frames.and_then(|frames| frames.expression_may_write_paths(machine, expression))
    {
        environment.invalidate_call_paths(program, Some(state), &written);
        return;
    }
    // NO RESOLVER AT ALL is not the same verdict as an unresolved call.
    // `CallFrameResolver::new` refuses whole-program when top-level
    // declarations conflict, so one duplicate `data` anywhere used to clear
    // the environment at EVERY transition in the program -- including
    // bundled library code, which then reported its own guard-narrowed
    // ranged arguments as unprovable. A call-free expression writes nothing
    // whether or not a resolver exists: that is the very answer the resolver
    // gives for it (`expression_write_frame_uncached` returns a complete
    // empty frame before it resolves anything). Keep the facts.
    if frames.is_none() && !expression_has_calls(program, expression) {
        return;
    }
    environment.invalidate_unknown_writes(program, Some(state));
}
