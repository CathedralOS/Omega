use crate::control_flow::ControlFlowPlan;
use crate::host_calls::HostCallPlan;

pub(super) fn attach_host_call_state_keys_to_plan(
    plan: &mut HostCallPlan,
    control_flow: &ControlFlowPlan,
) {
    plan.calls.for_each_mut(|_, call| {
        call.source_key = control_flow
            .state_key_by_names(&call.machine, &call.state)
            .unwrap_or_default();
    });

    plan.unsupported_calls.for_each_mut(|_, call| {
        call.source_key = control_flow
            .state_key_by_names(&call.machine, &call.state)
            .unwrap_or_default();
    });
}
