mod arguments;
mod collection;
mod lookups;
mod lowering;
mod model;
mod required;

use crate::plan::NativePlan;
use crate::state_analysis::StateAnalysisContext;
use arguments::build_call_arguments;
use collection::collect_machine_state_calls;
use lowering::state_call_lowering;
use omega_core::parallel::{WorkerPool, WorkerPoolHandle};
use required::mark_required_state_calls;
use std::sync::Arc;

pub use model::{
    StateCall, StateCallArgument, StateCallArgumentKind, StateCallLowering, StateCallPlan,
    StateCallResolution,
};

pub fn build_state_call_plan(native_plan: &NativePlan) -> StateCallPlan {
    let workers = WorkerPool::with_available_parallelism();

    build_state_call_plan_with_workers(
        Arc::new(StateAnalysisContext::from_native_plan(native_plan)),
        workers.handle(),
    )
}

pub fn build_state_call_plan_with_workers(
    context: Arc<StateAnalysisContext>,
    workers: WorkerPoolHandle,
) -> StateCallPlan {
    let machines = Arc::new(
        context
            .control_flow
            .machines
            .iter()
            .map(|(_, machine)| machine.clone())
            .collect::<Vec<_>>(),
    );
    let machine_count = machines.len();
    let context_for_collection = Arc::clone(&context);
    let machine_calls = workers.map_ordered(machine_count, move |index| {
        let machine = machines
            .get(index)
            .expect("state-call worker index should be in range");

        collect_machine_state_calls(&context_for_collection, machine)
    });

    let mut calls = machine_calls.into_iter().flatten().collect::<Vec<_>>();

    mark_required_state_calls(&context, &mut calls);

    let mut plan = StateCallPlan::default();
    for call in calls {
        let lowering = state_call_lowering(&context, &call);
        let arguments = plan.arguments.insert_many(build_call_arguments(
            &context,
            call.target_key,
            call.required,
            &call.raw_arguments,
        ));

        plan.calls.insert(StateCall {
            source_key: call.source_key,
            statement_index: call.statement_index,
            receiver: call.receiver,
            target_key: call.target_key,
            target_machine: call.target_machine,
            target_state: call.target_state,
            argument_count: arguments.len(),
            arguments,
            reachable: call.reachable,
            required: call.required,
            lowering,
            resolution: call.resolution,
        });
    }

    plan
}
