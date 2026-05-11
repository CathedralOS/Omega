mod host_calls;
mod static_strings;

use host_calls::{collect_host_call_data, collect_newline_data};
use omega_platform_interface::HostCallPlan;
use omega_state_storage::StateStoragePlan;
use omega_target_operations::TargetDataPlan;
use static_strings::collect_static_string_assignment_data;

pub fn build_target_data_plan(
    host_calls: &HostCallPlan,
    state_storage: &StateStoragePlan,
) -> TargetDataPlan {
    let mut data_plan = TargetDataPlan::default();

    for (_, host_call) in host_calls.calls.iter() {
        collect_host_call_data(host_calls, host_call, &mut data_plan);
    }
    collect_newline_data(host_calls, &mut data_plan);
    collect_static_string_assignment_data(state_storage, &mut data_plan);

    data_plan
}
