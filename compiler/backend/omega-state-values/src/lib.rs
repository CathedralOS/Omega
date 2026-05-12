mod classify;
mod collection;
mod model;
mod simplify;

pub use model::{StateValueKind, StateValuePlan, StateValueRole, StateValueUse};
pub use simplify::simplify_expression;

use collection::build_machine_state_value_plan;
use omega_control_flow::StateKey;
use omega_core::parallel::{WorkerPool, WorkerPoolHandle};
use omega_state_calls::{StateCallPlan, StateCallRole};
use omega_state_graph::RuntimeFlowPlan;
use omega_typed_trees::Program;
use std::sync::Arc;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateValuePlanningContext {
    pub runtime_flow: RuntimeFlowPlan,
    pub state_calls: StateCallPlan,
}

impl StateValuePlanningContext {
    pub fn state_is_required_by_key(&self, state_key: StateKey) -> bool {
        self.runtime_flow
            .states
            .iter()
            .any(|(_, state)| state.key == state_key)
            || self.state_calls.calls.iter().any(|(_, state_call)| {
                state_call.required
                    && (state_call.source_key == state_key
                        || (state_call.role == StateCallRole::Statement
                            && state_call.target_key == state_key))
            })
    }
}

pub fn build_state_value_plan(
    program: &Program,
    context: StateValuePlanningContext,
) -> StateValuePlan {
    let workers = WorkerPool::with_available_parallelism();

    build_state_value_plan_with_workers(
        Arc::new(program.clone()),
        Arc::new(context),
        workers.handle(),
    )
}

pub fn build_state_value_plan_with_workers(
    program: Arc<Program>,
    context: Arc<StateValuePlanningContext>,
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

        build_machine_state_value_plan(&program, &context, machine)
    });

    let mut plan = StateValuePlan::default();

    for machine_plan in machine_plans {
        for (_, value) in machine_plan.values.iter() {
            plan.values.append(StateValueUse {
                expression: plan
                    .expressions
                    .copy_from(&machine_plan.expressions, value.expression),
                ..value.clone()
            });
        }
    }

    plan
}
