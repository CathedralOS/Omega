use crate::plan::NativePlan;
use crate::runtime_flow::RuntimeTransitionTarget;
use crate::state_guards::StateGuardLowering;
use omega_core::arena::Arena;

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

        blockers.insert(blocker(
            "state guards",
            &format!(
                "#{} {}.{} edge {} -> #{} {} {:?}/{:?} `{}` needs runtime guard lowering",
                guard.source_dispatch_index,
                guard.source_machine,
                guard.source_state,
                guard.statement_order,
                guard.target_dispatch_index,
                runtime_transition_target_name(&guard.target),
                guard.kind,
                guard.lowering,
                guard.expression.display_name()
            ),
        ));
    }
}

fn runtime_transition_target_name(target: &RuntimeTransitionTarget) -> String {
    match target {
        RuntimeTransitionTarget::State { machine, state, .. } => format!("{machine}.{state}"),
        RuntimeTransitionTarget::Terminal => "terminal".to_owned(),
        RuntimeTransitionTarget::None => "none".to_owned(),
        RuntimeTransitionTarget::Unknown { name } => format!("unknown {name}"),
    }
}
