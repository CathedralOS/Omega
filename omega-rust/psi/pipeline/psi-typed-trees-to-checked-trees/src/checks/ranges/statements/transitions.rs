use psi_diagnostics::Diagnostic;
use psi_typed_trees::machine::Machine;
use psi_typed_trees::state::State;
use psi_typed_trees::statement::{TransitionTargetHandle, TransitionTargetNode};

use super::super::facts::RangeFacts;
use super::super::indexes::check_expression;

pub(super) fn check_transition_target<'program>(
    program: &'program psi_typed_trees::TypedTrees,
    machine: &'program Machine,
    state: &State,
    call_frames: Option<&psi_validation::CallFrameResolver<'program>>,
    facts: &mut RangeFacts<'_>,
    target: TransitionTargetHandle,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !target.is_valid() {
        return;
    }

    match program.statement_table.transition_target(target) {
        TransitionTargetNode::Named { arguments, .. } => {
            for argument in program.statement_table.expression_handles(*arguments) {
                check_expression(
                    program,
                    machine,
                    state,
                    call_frames,
                    facts,
                    *argument,
                    diagnostics,
                );
            }
        }
        TransitionTargetNode::Value(value) => check_expression(
            program,
            machine,
            state,
            call_frames,
            facts,
            *value,
            diagnostics,
        ),
        TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => {}
    }
}
