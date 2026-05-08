mod classify;
mod collection;
mod model;

pub use model::{StateValueKind, StateValuePlan, StateValueRole, StateValueUse};

use crate::plan::NativePlan;
use crate::state_analysis::StateAnalysisContext;
use collection::build_machine_state_value_plan;
use omega_core::parallel::{WorkerPool, WorkerPoolHandle};
use omega_typed_program::Program;
use std::sync::Arc;

pub fn build_state_value_plan(program: &Program, native_plan: &NativePlan) -> StateValuePlan {
    let workers = WorkerPool::with_available_parallelism();

    build_state_value_plan_with_workers(
        Arc::new(program.clone()),
        Arc::new(StateAnalysisContext::from_native_plan(native_plan)),
        workers.handle(),
    )
}

pub fn build_state_value_plan_with_workers(
    program: Arc<Program>,
    context: Arc<StateAnalysisContext>,
    workers: WorkerPoolHandle,
) -> StateValuePlan {
    if program.machines.is_empty() {
        return StateValuePlan::default();
    }

    let machine_count = program.machines.len();
    let machine_plans = workers.map_ordered(machine_count, move |index| {
        let machine = program
            .machines
            .get(index)
            .expect("state-value worker index should be in range");

        build_machine_state_value_plan(&context, machine)
    });

    let mut plan = StateValuePlan::default();

    for machine_plan in machine_plans {
        plan.values
            .insert_many(machine_plan.values.iter().map(|(_, value)| value.clone()));
    }

    plan
}
