use crate::collection::build_machine_state_value_plan;
use crate::dependencies::StateValueDependencyIndex;
use crate::model::{StateValuePlan, StateValueUse};
use omega_control_flow::StateKey;
use omega_core::parallel::{WorkerPool, WorkerPoolHandle};
use omega_state_calls::StateCallPlan;
use omega_state_graph::RuntimeFlowPlan;
use psi_checked_trees::CheckedTrees;
use psi_checked_trees::expression::ExpressionTableCapacity;
use std::sync::Arc;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateValuePlanningContext {
    pub runtime_flow: Arc<RuntimeFlowPlan>,
    pub state_calls: Arc<StateCallPlan>,
}

impl StateValuePlanningContext {
    pub fn state_is_required_by_key(&self, state_key: StateKey) -> bool {
        self.runtime_flow
            .states
            .iter()
            .any(|(_, state)| state.key == state_key)
            || self.state_calls.required_state(state_key)
    }
}

pub fn build_state_value_plan(
    program: &CheckedTrees,
    context: StateValuePlanningContext,
) -> StateValuePlan {
    let workers = WorkerPool::with_available_parallelism();

    build_state_value_plan_with_workers(
        Arc::new(program.clone()),
        Arc::new(context),
        workers.handle(),
    )
}

pub fn build_state_value_plan_owned(
    program: CheckedTrees,
    context: StateValuePlanningContext,
) -> StateValuePlan {
    let workers = WorkerPool::with_available_parallelism();

    build_state_value_plan_with_workers(Arc::new(program), Arc::new(context), workers.handle())
}

pub fn build_state_value_plan_with_workers(
    program: Arc<CheckedTrees>,
    context: Arc<StateValuePlanningContext>,
    workers: WorkerPoolHandle,
) -> StateValuePlan {
    if program.machines().is_empty() {
        return StateValuePlan::default();
    }

    let dependencies = Arc::new(StateValueDependencyIndex::build(&program, &context));
    let machine_count = program.machines().len();
    let machine_plans = workers.map_ordered(machine_count, move |index| {
        let machine = program
            .machines()
            .get(index)
            .expect("state-value worker index should be in range");

        build_machine_state_value_plan(&program, &context, &dependencies, machine)
    });

    let value_count = machine_plans
        .iter()
        .map(|machine_plan| machine_plan.values.len())
        .sum();
    let expression_capacity = machine_plans.iter().fold(
        ExpressionTableCapacity::default(),
        |mut capacity, machine_plan| {
            capacity.saturating_add_assign(machine_plan.expressions.copy_capacity());
            capacity
        },
    );
    let mut plan = StateValuePlan::with_capacities(value_count, expression_capacity);

    for machine_plan in machine_plans {
        let StateValuePlan {
            expressions,
            values,
        } = machine_plan;
        for value in values.into_items() {
            plan.values.append(StateValueUse {
                source_key: value.source_key,
                statement_index: value.statement_index,
                role: value.role,
                kind: value.kind,
                expression: plan.expressions.copy_from(&expressions, value.expression),
                required: value.required,
            });
        }
    }

    plan
}
