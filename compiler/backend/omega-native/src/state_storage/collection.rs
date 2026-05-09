use super::{StateLocalStorage, StateMutation, StateStoragePlan};
use crate::control_flow::StateKey;
use crate::plan::NativePlan;
use crate::state_analysis::StateAnalysisContext;
use crate::state_storage::mutation_kind::{mutation_kind, mutation_lowering};
use omega_core::parallel::{WorkerPool, WorkerPoolHandle};
use omega_core::symbols::SymbolHandle;
use omega_typed_program::Program;
use omega_typed_program::machine::Machine;
use omega_typed_program::statement::Statement;
use omega_typed_program::types::{PrimitiveType, TypeReference};
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
        let source_key = StateKey {
            machine: machine.symbol,
            state: state.symbol,
            segment_index: 0,
        };
        let required = context.state_is_required_by_key(source_key);

        for (statement_index, statement) in state.statements.iter().enumerate() {
            match statement {
                Statement::LocalData(local_data) => {
                    plan.locals.insert(StateLocalStorage {
                        source_key,
                        statement_index,
                        symbol: local_data.symbol,
                        name: local_data.name.clone(),
                        type_symbol: type_reference_symbol(&program, &local_data.type_reference),
                        type_name: local_data.type_reference.display_name(),
                        required,
                    });
                }
                Statement::Assignment(assignment) => {
                    let mutation_kind =
                        mutation_kind(program, &machine.name, state, &assignment.target);
                    plan.mutations.insert(StateMutation {
                        source_key,
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

fn type_reference_symbol(program: &Program, type_reference: &TypeReference) -> SymbolHandle {
    match type_reference {
        TypeReference::Constrained { base_type, .. } => type_reference_symbol(program, base_type),
        TypeReference::FixedArray { element_type, .. } => {
            type_reference_symbol(program, element_type)
        }
        TypeReference::Generic { base_name, .. } | TypeReference::Named(base_name) => {
            if PrimitiveType::from_name(base_name).is_some() {
                return SymbolHandle::invalid();
            }

            program
                .data_definitions
                .iter()
                .find(|definition| definition.name == *base_name)
                .map(|definition| definition.symbol)
                .or_else(|| {
                    program
                        .machines
                        .iter()
                        .find(|machine| machine.name == *base_name)
                        .map(|machine| machine.symbol)
                })
                .unwrap_or_else(SymbolHandle::invalid)
        }
        TypeReference::Unit => SymbolHandle::invalid(),
    }
}
