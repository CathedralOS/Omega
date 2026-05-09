use omega_core::arena::Arena;
use omega_native::plan::NativePlan;
use omega_native::state_guards::StateGuardLowering;
use omega_state_graph::RuntimeTransitionTarget;

use super::{EmissionBlocker, blocker};

pub(super) fn collect_state_guard_blockers(
    native_plan: &NativePlan,
    blockers: &mut Arena<EmissionBlocker>,
) {
    for (_, guard) in native_plan.state_guards.guards.iter() {
        if matches!(
            guard.lowering,
            StateGuardLowering::NoOp | StateGuardLowering::CompareStaticValue
        ) {
            continue;
        }

        let machine_name = native_plan
            .control_flow
            .machine_by_symbol(guard.source.machine)
            .map(|machine| machine.name.as_str())
            .unwrap_or("<unknown>");
        let state_name = native_plan
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
                runtime_transition_target_name(native_plan, &guard.target),
                guard.kind,
                guard.lowering,
                guard.expression.display_name()
            ),
        ));
    }
}

fn runtime_transition_target_name(
    native_plan: &NativePlan,
    target: &RuntimeTransitionTarget,
) -> String {
    match target {
        RuntimeTransitionTarget::State { key } => native_plan
            .control_flow
            .state_names_by_key(*key)
            .map(|(machine, state)| format!("{machine}.{state}"))
            .unwrap_or_else(|| "<unknown>.<unknown>".to_owned()),
        RuntimeTransitionTarget::Terminal => "terminal".to_owned(),
        RuntimeTransitionTarget::None => "none".to_owned(),
        RuntimeTransitionTarget::Unknown { name } => format!("unknown {name}"),
    }
}
