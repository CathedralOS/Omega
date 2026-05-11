use super::{StateLocalStorage, StateMutation, StateStoragePlan};
use crate::StateStoragePlanningContext;
use crate::mutation_kind::{mutation_kind, mutation_lowering};
use omega_control_flow::StateKey;
use omega_core::parallel::{WorkerPool, WorkerPoolHandle};
use omega_core::symbols::SymbolHandle;
use omega_typed_trees::Program;
use omega_typed_trees::machine::Machine;
use omega_typed_trees::statement::StatementNode;
use omega_typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};
use std::fmt::Write;
use std::sync::Arc;

pub fn build_state_storage_plan(
    program: &Program,
    context: StateStoragePlanningContext,
) -> StateStoragePlan {
    let workers = WorkerPool::with_available_parallelism();

    build_state_storage_plan_with_workers(
        Arc::new(program.clone()),
        Arc::new(context),
        workers.handle(),
    )
}

pub fn build_state_storage_plan_with_workers(
    program: Arc<Program>,
    context: Arc<StateStoragePlanningContext>,
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
        for (_, mutation) in machine_plan.mutations.iter() {
            plan.mutations.append(StateMutation {
                target: plan
                    .expressions
                    .copy_from(&machine_plan.expressions, mutation.target),
                value: plan
                    .expressions
                    .copy_from(&machine_plan.expressions, mutation.value),
                ..mutation.clone()
            });
        }
    }

    plan
}

fn build_machine_state_storage_plan(
    program: &Program,
    context: &StateStoragePlanningContext,
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

        let statements = program.statement_table.statements(state.statement_nodes);

        for (statement_index, statement) in statements.iter().enumerate() {
            match statement {
                StatementNode::LocalData(local_data) => {
                    plan.locals.insert(StateLocalStorage {
                        source_key,
                        statement_index,
                        symbol: local_data.symbol,
                        name: local_data.name.clone(),
                        type_symbol: type_reference_symbol(program, local_data.type_reference),
                        type_name: type_reference_display_name(program, local_data.type_reference),
                        required,
                    });
                }
                StatementNode::Assignment(assignment) => {
                    let target = plan
                        .expressions
                        .copy_from(&program.expression_table, assignment.target);
                    let value = plan
                        .expressions
                        .copy_from(&program.expression_table, assignment.value);
                    let mutation_kind = mutation_kind(
                        machine,
                        state,
                        statements,
                        &program.expression_table,
                        assignment.target,
                    );
                    plan.mutations.insert(StateMutation {
                        source_key,
                        statement_index,
                        target,
                        value,
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

fn type_reference_symbol(program: &Program, type_reference: TypeReferenceHandle) -> SymbolHandle {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Constrained { base_type, .. } => {
            type_reference_symbol(program, *base_type)
        }
        TypeReferenceNode::FixedArray { element_type, .. } => {
            type_reference_symbol(program, *element_type)
        }
        TypeReferenceNode::Slice { element_type } => type_reference_symbol(program, *element_type),
        TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            ..
        } => {
            if PrimitiveType::from_name(base_name.as_str()).is_some() {
                return SymbolHandle::invalid();
            }
            if base_symbol.is_valid() {
                return *base_symbol;
            }

            SymbolHandle::invalid()
        }
        TypeReferenceNode::Named { symbol, name } => {
            if PrimitiveType::from_name(name.as_str()).is_some() {
                return SymbolHandle::invalid();
            }
            if symbol.is_valid() {
                return *symbol;
            }

            SymbolHandle::invalid()
        }
        TypeReferenceNode::Unit => SymbolHandle::invalid(),
    }
}

fn type_reference_display_name(program: &Program, type_reference: TypeReferenceHandle) -> String {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            format!(
                "{}[{}]",
                type_reference_display_name(program, *base_type),
                match constraints.count() {
                    1 => "1 constraint".to_owned(),
                    count => format!("{count} constraints"),
                }
            )
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length,
        } => format!(
            "[{}; {}]",
            type_reference_display_name(program, *element_type),
            length
        ),
        TypeReferenceNode::Slice { element_type } => {
            format!("[{}]", type_reference_display_name(program, *element_type))
        }
        TypeReferenceNode::Generic {
            base_name,
            arguments,
            ..
        } => {
            let mut display = String::new();
            display.push_str(base_name.as_str());
            display.push('<');
            for (index, argument) in program
                .type_reference_table
                .type_reference_handles(*arguments)
                .iter()
                .enumerate()
            {
                if index > 0 {
                    display.push_str(", ");
                }
                write!(
                    display,
                    "{}",
                    type_reference_display_name(program, *argument)
                )
                .expect("writing type display into string should not fail");
            }
            display.push('>');
            display
        }
        TypeReferenceNode::Named { name, .. } => name.to_string(),
        TypeReferenceNode::Unit => "()".to_owned(),
    }
}
