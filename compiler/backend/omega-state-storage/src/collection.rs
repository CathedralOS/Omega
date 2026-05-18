use super::{StateLocalStorage, StateMutation, StateStoragePlan};
use crate::StateStoragePlanningContext;
use crate::mutation_kind::{mutation_kind, mutation_lowering};
use omega_checked_trees::Program;
use omega_checked_trees::machine::Machine;
use omega_checked_trees::name::ProgramName;
use omega_checked_trees::statement::StatementNode;
use omega_checked_trees::types::{TypeReferenceHandle, TypeReferenceNode};
use omega_control_flow::StateKey;
use omega_core::arena::{Arena, HandleSpan};
use omega_core::parallel::{WorkerPool, WorkerPoolHandle};
use omega_core::symbols::SymbolHandle;
use omega_state_values::simplify_state_expression;
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
    if program.machines().is_empty() {
        return StateStoragePlan::default();
    }

    let machine_count = program.machines().len();
    let machine_plans = workers.map_ordered(machine_count, move |index| {
        let machine = program
            .machines()
            .get(index)
            .expect("state-storage worker index should be in range");

        build_machine_state_storage_plan(&program, &context, machine)
    });

    let mut plan = StateStoragePlan::default();

    for machine_plan in machine_plans {
        for (_, local) in machine_plan.locals.iter() {
            plan.locals.insert(StateLocalStorage {
                invariant_names: plan.invariant_names.insert_many(
                    machine_plan
                        .invariant_names
                        .span_or_empty(local.invariant_names)
                        .iter()
                        .cloned(),
                ),
                type_reference: plan.type_references.copy_from(
                    &machine_plan.type_references,
                    &machine_plan.expressions,
                    &mut plan.expressions,
                    local.type_reference,
                ),
                ..local.clone()
            });
        }
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

    for state in program.machine_states(machine) {
        let source_key = StateKey {
            machine: machine.symbol,
            state: state.symbol,
            segment_index: 0,
        };
        let required = context.state_is_required_by_key(source_key);
        let uses_runtime_flow = context.state_uses_runtime_flow_by_key(source_key);

        let statements = program.statement_table.statements(state.statement_nodes);

        for (statement_index, statement) in statements.iter().enumerate() {
            match statement {
                StatementNode::LocalData(local_data) => {
                    if !local_data_requires_storage(
                        &program.expression_table,
                        statements,
                        statement_index,
                        local_data.symbol,
                        local_data.initial_value.is_valid(),
                        uses_runtime_flow,
                    ) {
                        continue;
                    }
                    plan.locals.insert(StateLocalStorage {
                        source_key,
                        statement_index,
                        symbol: local_data.symbol,
                        name: local_data.name.clone(),
                        type_symbol: program.type_reference_symbol(local_data.type_reference),
                        type_reference: plan.type_references.copy_from(
                            &program.type_reference_table,
                            &program.expression_table,
                            &mut plan.expressions,
                            local_data.type_reference,
                        ),
                        invariant_names: append_type_reference_invariant_names(
                            program,
                            local_data.type_reference,
                            &mut plan.invariant_names,
                        ),
                        required,
                    });
                }
                StatementNode::Assignment(assignment) => {
                    let target = plan
                        .expressions
                        .copy_from(&program.expression_table, assignment.target);
                    let value = if program
                        .expression_table
                        .expression_is_literal(assignment.value)
                    {
                        plan.expressions
                            .copy_from(&program.expression_table, assignment.value)
                    } else {
                        let simplified_value = simplify_state_expression(
                            program,
                            machine,
                            state,
                            statement_index,
                            &program.expression_table.to_tree(assignment.value),
                        );
                        plan.expressions.insert_tree(&simplified_value)
                    };
                    let mutation_kind = mutation_kind(
                        context,
                        source_key,
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

fn local_data_requires_storage(
    expressions: &omega_checked_trees::expression::ExpressionTable,
    statements: &[StatementNode],
    local_statement_index: usize,
    local_symbol: SymbolHandle,
    has_initial_value: bool,
    uses_runtime_flow: bool,
) -> bool {
    if !has_initial_value {
        return true;
    }

    if uses_runtime_flow {
        return true;
    }

    statements
        .iter()
        .skip(local_statement_index + 1)
        .any(|statement| assignment_targets_symbol(expressions, statement, local_symbol))
}

fn assignment_targets_symbol(
    expressions: &omega_checked_trees::expression::ExpressionTable,
    statement: &StatementNode,
    symbol: SymbolHandle,
) -> bool {
    let StatementNode::Assignment(assignment) = statement else {
        return false;
    };

    assignment_target_head_symbol(expressions, assignment.target) == symbol
}

fn assignment_target_head_symbol(
    expressions: &omega_checked_trees::expression::ExpressionTable,
    expression: omega_checked_trees::expression::ExpressionHandle,
) -> SymbolHandle {
    use omega_checked_trees::expression::ExpressionNode;

    match expressions.expression(expression) {
        ExpressionNode::Name(path) => {
            let head_symbol = path.head_symbol;
            head_symbol
        }
        ExpressionNode::Member(member) => {
            assignment_target_head_symbol(expressions, member.receiver)
        }
        ExpressionNode::Indexed(indexed) => {
            assignment_target_head_symbol(expressions, indexed.collection)
        }
        ExpressionNode::Mutable(inner) => assignment_target_head_symbol(expressions, *inner),
        _ => SymbolHandle::invalid(),
    }
}

fn append_type_reference_invariant_names(
    program: &Program,
    type_reference: TypeReferenceHandle,
    names: &mut Arena<ProgramName>,
) -> HandleSpan<ProgramName> {
    let mut span = HandleSpan::empty();
    collect_type_reference_invariant_names(program, type_reference, names, &mut span);
    span
}

fn collect_type_reference_invariant_names(
    program: &Program,
    type_reference: TypeReferenceHandle,
    names: &mut Arena<ProgramName>,
    span: &mut HandleSpan<ProgramName>,
) {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            collect_type_reference_invariant_names(program, *referee, names, span)
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            collect_type_reference_invariant_names(program, *base_type, names, span);

            for constraint in program.type_reference_table.constraints(*constraints) {
                let omega_checked_trees::types::TypeConstraintNode::Named(name) = constraint else {
                    continue;
                };

                if program
                    .facts
                    .invariants
                    .definitions
                    .iter()
                    .any(|(_, invariant)| invariant.name == *name)
                    && !names
                        .span_or_empty(*span)
                        .iter()
                        .any(|existing| existing == name)
                {
                    names.append_to_span(span, name.clone());
                }
            }
        }
        TypeReferenceNode::FixedArray { element_type, .. } => {
            collect_type_reference_invariant_names(program, *element_type, names, span)
        }
        TypeReferenceNode::Slice { element_type } => {
            collect_type_reference_invariant_names(program, *element_type, names, span)
        }
        TypeReferenceNode::Generic { arguments, .. } => {
            for argument in program
                .type_reference_table
                .type_reference_handles(*arguments)
            {
                collect_type_reference_invariant_names(program, *argument, names, span);
            }
        }
        TypeReferenceNode::Named { .. } | TypeReferenceNode::Unit => {}
    }
}
