use crate::EmissionPlanningInput;
use omega_core::arena::Arena;
use omega_state_graph::RuntimeTransitionTarget;
use omega_state_guards::StateGuardLowering;

use super::{EmissionBlocker, blocker};

pub(super) fn collect_state_guard_blockers(
    input: &EmissionPlanningInput<'_>,
    blockers: &mut Arena<EmissionBlocker>,
) {
    for (_, guard) in input.state_guards.guards.iter() {
        if matches!(
            guard.lowering,
            StateGuardLowering::NoOp
                | StateGuardLowering::CompareStaticValue
                | StateGuardLowering::CompareRuntimeValue
        ) {
            continue;
        }

        let machine_name = input
            .control_flow
            .machine_by_symbol(guard.source.machine)
            .map(|machine| machine.name.as_str())
            .unwrap_or("<unknown>");
        let state_name = input
            .control_flow
            .state_by_key(guard.source)
            .map(|state| state.name.as_str())
            .unwrap_or("<unknown>");

        blockers.insert(blocker(
            "state guards",
            &format!(
                "#{} {}.{} edge {} -> #{} {} {:?}/{:?} `{}` needs runtime guard lowering",
                guard.source_dispatch_index,
                machine_name,
                state_name,
                guard.statement_order,
                guard.target_dispatch_index,
                runtime_transition_target_name(input, &guard.target),
                guard.kind,
                guard.lowering,
                input
                    .state_guards
                    .expressions
                    .display_name(guard.expression)
            ),
        ));
    }
}

fn runtime_transition_target_name(
    input: &EmissionPlanningInput<'_>,
    target: &RuntimeTransitionTarget,
) -> String {
    match target {
        RuntimeTransitionTarget::State { key } => input
            .control_flow
            .state_names_by_key(*key)
            .map(|(machine, state)| format!("{machine}.{state}"))
            .unwrap_or_else(|| "<unknown>.<unknown>".to_owned()),
        RuntimeTransitionTarget::Terminal => "terminal".to_owned(),
        RuntimeTransitionTarget::None => "none".to_owned(),
        RuntimeTransitionTarget::Unknown { name } => format!("unknown {name}"),
    }
}
