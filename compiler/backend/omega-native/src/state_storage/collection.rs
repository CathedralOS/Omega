use super::{StateLocalStorage, StateMutation, StateStoragePlan};
use crate::plan::NativePlan;
use crate::state_analysis::StateAnalysisContext;
use crate::state_storage::mutation_kind::{mutation_kind, mutation_lowering};
use omega_core::parallel::{WorkerPool, WorkerPoolHandle};
use omega_typed_program::Program;
use omega_typed_program::machine::Machine;
use omega_typed_program::statement::Statement;
use std::sync::Arc;

pub fn build_state_storage_plan(program: &Program, native_plan: &NativePlan) -> StateStoragePlan {
    let workers = WorkerPool::with_available_parallelism();

    build_state_storage_plan_with_workers(
        Arc::new(program.clone()),
        Arc::new(StateAnalysisContext::from_native_plan(native_plan)),
        workers.handle(),
    )
}

pub fn build_state_storage_plan_with_workers(
    program: Arc<Program>,
    context: Arc<StateAnalysisContext>,
    workers: WorkerPoolHandle,
) -> StateStoragePlan {
    if program.machines.is_empty() {
        return StateStoragePlan::default();
    }

    let machine_count = program.machines.len();
    let machine_plans = workers.map_ordered(machine_count, move |index| {
        let machine = program
            .machines
            .get(index)
            .expect("state-storage worker index should be in range");

        build_machine_state_storage_plan(&program, &context, machine)
    });

    let mut plan = StateStoragePlan::default();

    for machine_plan in machine_plans {
        plan.locals
            .insert_many(machine_plan.locals.iter().map(|(_, local)| local.clone()));
        plan.mutations.insert_many(
            machine_plan
                .mutations
                .iter()
                .map(|(_, mutation)| mutation.clone()),
        );
    }

    plan
}

fn build_machine_state_storage_plan(
    program: &Program,
    context: &StateAnalysisContext,
    machine: &Machine,
) -> StateStoragePlan {
    let mut plan = StateStoragePlan::default();

    for state in &machine.states {
        let required = context.state_is_required(&machine.name, &state.name);
        let source_key = context
            .state_key(&machine.name, &state.name)
            .unwrap_or_default();

        for (statement_index, statement) in state.statements.iter().enumerate() {
            match statement {
                Statement::LocalData(local_data) => {
                    plan.locals.insert(StateLocalStorage {
                        source_key,
                        machine: machine.name.clone(),
                        state: state.name.clone(),
                        statement_index,
                        name: local_data.name.clone(),
                        type_name: local_data.type_reference.display_name(),
                        required,
                    });
                }
                Statement::Assignment(assignment) => {
                    let mutation_kind =
                        mutation_kind(program, &machine.name, state, &assignment.target);
                    plan.mutations.insert(StateMutation {
                        source_key,
                        machine: machine.name.clone(),
                        state: state.name.clone(),
                        statement_index,
                        target: assignment.target.clone(),
                        value: assignment.value.clone(),
                        mutation_kind,
                        lowering: mutation_lowering(
                            context,
                            source_key,
                            statement_index,
                            mutation_kind,
                        ),
                        required,
                    });
                }
                _ => {}
            }
        }
    }

    plan
}
