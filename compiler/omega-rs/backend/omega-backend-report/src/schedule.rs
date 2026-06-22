use crate::BackendReportInput;
use omega_state_schedule::{
    StateScheduleContext, build_entry_state_schedule, scheduled_state_flow,
};

pub(super) fn write_state_schedule(output: &mut String, backend_plan: &BackendReportInput<'_>) {
    output.push_str("## State Schedule\n");
    let schedule_context = StateScheduleContext::new(
        &backend_plan.control_flow,
        &backend_plan.host_calls,
        &backend_plan.state_calls,
    );
    match build_entry_state_schedule(&schedule_context, backend_plan.entry_key) {
        Ok(schedule) if schedule.is_empty() => output.push_str("states: 0\nnone\n"),
        Ok(schedule) => {
            output.push_str(&format!("states: {}\n", schedule.len()));
            for scheduled_state in schedule {
                if let Some(state_flow) = scheduled_state_flow(&schedule_context, &scheduled_state)
                {
                    output.push_str(&format!(
                        "- {}.{}#{}\n",
                        backend_plan
                            .control_flow
                            .machines
                            .iter()
                            .find(|(_, machine)| machine.symbol == state_flow.key.machine)
                            .map(|(_, machine)| machine.name.as_str())
                            .unwrap_or("<missing-machine>"),
                        state_flow.name,
                        state_flow.key.segment_index
                    ));
                } else {
                    output.push_str(&format!(
                        "- symbol {}.{}#{}\n",
                        scheduled_state.key.machine.arena_index(),
                        scheduled_state.key.state.arena_index(),
                        scheduled_state.key.segment_index
                    ));
                }
            }
        }
        Err(reason) => {
            output.push_str("status: blocked\n");
            output.push_str(&format!("reason: {reason}\n"));
        }
    }
}
