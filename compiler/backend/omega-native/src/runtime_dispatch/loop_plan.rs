mod collection;
mod context;
mod guards;
mod inputs;
mod model;

use crate::plan::NativePlan;
use collection::build_runtime_dispatch_loop_case;
pub use context::RuntimeDispatchLoopContext;
pub use inputs::{RuntimeDispatchLoopCaseInput, runtime_dispatch_loop_inputs};
pub use model::{
    RuntimeDispatchLoopAction, RuntimeDispatchLoopCase, RuntimeDispatchLoopEdge,
    RuntimeDispatchLoopPlan,
};
use omega_core::arena::Arena;
use omega_core::parallel::{WorkerPool, WorkerPoolHandle};
use std::sync::Arc;

pub fn build_runtime_dispatch_loop_plan(native_plan: &NativePlan) -> RuntimeDispatchLoopPlan {
    let workers = WorkerPool::with_available_parallelism();

    build_runtime_dispatch_loop_plan_with_workers(
        Arc::new(RuntimeDispatchLoopContext::from_native_plan(native_plan)),
        runtime_dispatch_loop_inputs(native_plan),
        workers.handle(),
    )
}

pub fn build_runtime_dispatch_loop_plan_with_workers(
    context: Arc<RuntimeDispatchLoopContext>,
    case_inputs: Vec<RuntimeDispatchLoopCaseInput>,
    workers: WorkerPoolHandle,
) -> RuntimeDispatchLoopPlan {
    let mut plan = RuntimeDispatchLoopPlan {
        needed: context.needed,
        entry_dispatch_index: context.entry_dispatch_index,
        terminal_dispatch_index: 0,
        current_state_slot: "omega_current_state".to_owned(),
        next_state_slot: "omega_next_state".to_owned(),
        cases: Arena::new(),
        edges: Arena::new(),
    };

    if !plan.needed || case_inputs.is_empty() {
        return plan;
    }

    let case_inputs = Arc::new(case_inputs);
    let case_count = case_inputs.len();
    let context_for_cases = Arc::clone(&context);
    let cases = workers.map_ordered(case_count, move |index| {
        let case_input = case_inputs
            .get(index)
            .expect("runtime-dispatch-loop worker index should be in range");

        build_runtime_dispatch_loop_case(&context_for_cases, case_input)
    });

    for case in cases {
        let edges = plan.edges.insert_many(case.edges);

        plan.cases.insert(RuntimeDispatchLoopCase {
            key: case.key,
            dispatch_index: case.dispatch_index,
            label: case.label,
            operation_count: case.operation_count,
            edges,
        });
    }

    plan
}
