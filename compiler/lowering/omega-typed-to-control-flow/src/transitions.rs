use omega_core::diagnostics::Diagnostic;
use omega_typed_program::expression::display_name_path;
use omega_typed_program::statement::{Transition, TransitionTarget};

use omega_control_flow::{PlannedTransitionTarget, StateKey, TransitionFlow};

pub(super) fn plan_transition(
    state_indexes: &[(StateKey, usize)],
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
    state_indexes: &[(StateKey, usize)],
    target: &TransitionTarget,
) -> Result<PlannedTransitionTarget, Diagnostic> {
    match target {
        TransitionTarget::Named {
            path, arguments, ..
        } if path.len() == 1 || path.len() == 2 && path[0] == "self" => {
            let name = path.last().expect("named transition has a state").clone();
            let symbol = path.symbol();
            let target = symbol.is_valid().then(|| {
                state_indexes
                    .iter()
                    .find(|(key, _)| key.state == symbol && key.segment_index == 0)
            });
            let (key, index) = target.flatten().ok_or_else(|| {
                Diagnostic::error(format!("unknown state transition target `{name}`"))
            })?;

            Ok(PlannedTransitionTarget::State {
                index: *index,
                key: *key,
                name,
                arguments: arguments.clone(),
            })
        }
        TransitionTarget::Named {
            path, arguments, ..
        } if path.len() == 2 => Ok(PlannedTransitionTarget::Nested {
            receiver_symbol: path.head_symbol(),
            state_symbol: path.symbol(),
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
