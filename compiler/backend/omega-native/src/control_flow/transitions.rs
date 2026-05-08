use omega_core::diagnostics::Diagnostic;
use omega_typed_program::expression::display_name_path;
use omega_typed_program::statement::{Transition, TransitionTarget};

use super::{PlannedTransitionTarget, StateKey, TransitionFlow};

pub(super) fn plan_transition(
    state_indexes: &[(&str, StateKey, usize)],
    transition: &Transition,
) -> Result<TransitionFlow, Diagnostic> {
    Ok(TransitionFlow {
        target: plan_transition_target(state_indexes, &transition.target)?,
        continuation: transition
            .continuation
            .as_ref()
            .map(|target| plan_transition_target(state_indexes, target))
            .transpose()?,
        guard: transition.guard.clone(),
    })
}

fn plan_transition_target(
    state_indexes: &[(&str, StateKey, usize)],
    target: &TransitionTarget,
) -> Result<PlannedTransitionTarget, Diagnostic> {
    match target {
        TransitionTarget::Named {
            path, arguments, ..
        } if path.len() == 1 || path.len() == 2 && path[0] == "self" => {
            let name = path.last().expect("named transition has a state").clone();
            let index = state_indexes
                .iter()
                .find(|(state_name, _, _)| *state_name == name)
                .map(|(_, _, index)| *index)
                .ok_or_else(|| {
                    Diagnostic::error(format!("unknown state transition target `{name}`"))
                })?;
            let key = state_indexes
                .iter()
                .find(|(state_name, _, _)| *state_name == name)
                .map(|(_, key, _)| *key)
                .unwrap_or_default();

            Ok(PlannedTransitionTarget::State {
                index,
                key,
                name,
                arguments: arguments.clone(),
            })
        }
        TransitionTarget::Named {
            path, arguments, ..
        } if path.len() == 2 => Ok(PlannedTransitionTarget::Nested {
            receiver: path[0].clone(),
            state: path[1].clone(),
            arguments: arguments.clone(),
        }),
        TransitionTarget::Named { path, .. } => Err(Diagnostic::error(format!(
            "unsupported transition target `{}`",
            display_name_path(path, ".")
        ))),
        TransitionTarget::SelfTarget => Ok(PlannedTransitionTarget::SelfTarget),
        TransitionTarget::Terminal => Ok(PlannedTransitionTarget::Terminal),
    }
}
