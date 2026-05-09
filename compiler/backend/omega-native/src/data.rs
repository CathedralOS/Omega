mod host_calls;
mod model;
mod static_strings;

pub use model::{NativeDataObject, NativeDataPlan};

use crate::data::host_calls::{collect_host_call_data, collect_newline_data};
use crate::data::static_strings::collect_static_string_assignment_data;
use crate::state_storage::StateStoragePlan;
use omega_platform_interface::HostCallPlan;

pub fn build_native_data_plan(
    host_calls: &HostCallPlan,
    state_storage: &StateStoragePlan,
) -> NativeDataPlan {
    let mut data_plan = NativeDataPlan::default();

    for (_, host_call) in host_calls.calls.iter() {
        collect_host_call_data(host_calls, host_call, &mut data_plan);
    }
    collect_newline_data(host_calls, &mut data_plan);
    collect_static_string_assignment_data(state_storage, &mut data_plan);

    data_plan
}
