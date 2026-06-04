use super::{StateLocalStorage, StateMutation, StateStoragePlan};
use crate::StateStoragePlanningContext;
use crate::mutation_kind::{mutation_kind, mutation_lowering};
use omega_checked_trees::CheckedTrees;
use omega_checked_trees::expression::{ExpressionHandle, ExpressionNode, ExpressionTableCapacity};
use omega_checked_trees::machine::Machine;
use omega_checked_trees::name::Identifier;
use omega_checked_trees::statement::{
    StatementNode, StatementTable, TransitionGuardNode, TransitionTargetNode,
};
use omega_checked_trees::types::{TypeReferenceHandle, TypeReferenceNode};
use omega_control_flow::StateKey;
use omega_core::arena::{Arena, HandleSpan};
use omega_core::parallel::{WorkerPool, WorkerPoolHandle};
use omega_core::symbols::SymbolHandle;
use omega_state_values::simplify_state_expression;
use std::sync::Arc;

pub fn build_state_storage_plan(
    program: &CheckedTrees,
    context: StateStoragePlanningContext,
) -> StateStoragePlan {
    let workers = WorkerPool::with_available_parallelism();

    build_state_storage_plan_with_workers(
        Arc::new(program.clone()),
        Arc::new(context),
        workers.handle(),
    )
}

pub fn build_state_storage_plan_owned(
    program: CheckedTrees,
    context: StateStoragePlanningContext,
) -> StateStoragePlan {
    let workers = WorkerPool::with_available_parallelism();

    build_state_storage_plan_with_workers(Arc::new(program), Arc::new(context), workers.handle())
}

pub fn build_state_storage_plan_with_workers(
    program: Arc<CheckedTrees>,
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

    let local_count = machine_plans
        .iter()
        .map(|machine_plan| machine_plan.locals.len())
        .sum();
    let mutation_count = machine_plans
        .iter()
        .map(|machine_plan| machine_plan.mutations.len())
        .sum();
    let expression_capacity = machine_plans.iter().fold(
        ExpressionTableCapacity::default(),
        |mut capacity, machine_plan| {
            capacity.saturating_add_assign(machine_plan.expressions.copy_capacity());
            capacity
        },
    );
    let mut plan =
        StateStoragePlan::with_capacities(local_count, mutation_count, expression_capacity);

    for machine_plan in machine_plans {
        let StateStoragePlan {
            expressions,
            invariant_names,
            locals,
            mutations,
            type_references,
        } = machine_plan;
        for local in locals.into_items() {
            let local_invariant_names = plan.invariant_names.insert_many(
                invariant_names
                    .span_or_empty(local.invariant_names)
                    .iter()
                    .cloned(),
            );
            let local_type_reference = plan.type_references.copy_from(
                &type_references,
                &expressions,
                &mut plan.expressions,
                local.type_reference,
            );
            plan.locals.insert(StateLocalStorage {
                source_key: local.source_key,
                statement_index: local.statement_index,
                symbol: local.symbol,
                name: local.name,
                type_symbol: local.type_symbol,
                type_reference: local_type_reference,
                invariant_names: local_invariant_names,
                required: local.required,
            });
        }
        for mutation in mutations.into_items() {
            plan.mutations.append(StateMutation {
                source_key: mutation.source_key,
                statement_index: mutation.statement_index,
                target: plan.expressions.copy_from(&expressions, mutation.target),
                value: plan.expressions.copy_from(&expressions, mutation.value),
                mutation_kind: mutation.mutation_kind,
                lowering: mutation.lowering,
                required: mutation.required,
            });
        }
    }

    plan
}

fn build_machine_state_storage_plan(
    program: &CheckedTrees,
    context: &StateStoragePlanningContext,
    machine: &Machine,
) -> StateStoragePlan {
    let (local_capacity, mutation_capacity) = estimated_machine_storage_capacity(program, machine);
    let mut plan = StateStoragePlan::with_capacities(
        local_capacity,
        mutation_capacity,
        ExpressionTableCapacity {
            expressions: mutation_capacity.saturating_mul(2),
            ..ExpressionTableCapacity::default()
        },
    );

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
                        &program.statement_table,
                        statements,
                        statement_index,
                        local_data.symbol,
                        &local_data.name,
                        local_data.initial_value.is_valid(),
                        uses_runtime_flow || required,
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
                        || (program
                            .expression_table
                            .expression_is_direct_place_path(assignment.value)
                            && !state_has_initialized_locals_before(
                                program,
                                state,
                                statement_index,
                            )) {
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
                        program,
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

fn estimated_machine_storage_capacity(program: &CheckedTrees, machine: &Machine) -> (usize, usize) {
    program
        .machine_states(machine)
        .iter()
        .fold((0usize, 0usize), |(locals, mutations), state| {
            let statements = program.statement_table.statements(state.statement_nodes);
            let state_locals = statements
                .iter()
                .filter(|statement| matches!(statement, StatementNode::LocalData(_)))
                .count();
            let state_mutations = statements
                .iter()
                .filter(|statement| matches!(statement, StatementNode::Assignment(_)))
                .count();

            (
                locals.saturating_add(state_locals),
                mutations.saturating_add(state_mutations),
            )
        })
}

fn state_has_initialized_locals_before(
    program: &CheckedTrees,
    state: &omega_checked_trees::state::State,
    statement_index: usize,
) -> bool {
    program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .take(statement_index)
        .any(|statement| {
            matches!(
                statement,
                StatementNode::LocalData(local_data) if local_data.initial_value.is_valid()
            )
        })
}

fn local_data_requires_storage(
    expressions: &omega_checked_trees::expression::ExpressionTable,
    statement_table: &StatementTable,
    statements: &[StatementNode],
    local_statement_index: usize,
    local_symbol: SymbolHandle,
    local_name: &Identifier,
    has_initial_value: bool,
    uses_runtime_flow: bool,
) -> bool {
    if !has_initial_value {
        return true;
    }

    statements
        .iter()
        .skip(local_statement_index + 1)
        .any(|statement| {
            (uses_runtime_flow
                && statement_references_symbol_in_transition(
                    expressions,
                    statement_table,
                    statement,
                    local_symbol,
                    local_name,
                ))
                || assignment_target_references_symbol(
                    expressions,
                    statement,
                    local_symbol,
                    local_name,
                )
                || assignment_targets_symbol(expressions, statement, local_symbol, local_name)
                || statement_uses_symbol_mutably(
                    expressions,
                    statement_table,
                    statement,
                    local_symbol,
                    local_name,
                )
        })
}

fn statement_references_symbol_in_transition(
    expressions: &omega_checked_trees::expression::ExpressionTable,
    statement_table: &StatementTable,
    statement: &StatementNode,
    symbol: SymbolHandle,
    local_name: &Identifier,
) -> bool {
    let StatementNode::Transition(transition) = statement else {
        return false;
    };

    transition_guard_references_symbol(expressions, transition.guard, symbol, local_name)
        || transition_target_references_symbol(
            expressions,
            statement_table,
            transition.target,
            symbol,
            local_name,
        )
        || transition_target_references_symbol(
            expressions,
            statement_table,
            transition.continuation,
            symbol,
            local_name,
        )
}

fn assignment_targets_symbol(
    expressions: &omega_checked_trees::expression::ExpressionTable,
    statement: &StatementNode,
    symbol: SymbolHandle,
    local_name: &Identifier,
) -> bool {
    let StatementNode::Assignment(assignment) = statement else {
        return false;
    };

    assignment_target_head_symbol(expressions, assignment.target) == symbol
        || assignment_target_head_name(expressions, assignment.target)
            .is_some_and(|name| name == local_name)
}

fn assignment_target_references_symbol(
    expressions: &omega_checked_trees::expression::ExpressionTable,
    statement: &StatementNode,
    symbol: SymbolHandle,
    local_name: &Identifier,
) -> bool {
    let StatementNode::Assignment(assignment) = statement else {
        return false;
    };

    expression_references_symbol(expressions, assignment.target, symbol, local_name)
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

fn assignment_target_head_name(
    expressions: &omega_checked_trees::expression::ExpressionTable,
    expression: omega_checked_trees::expression::ExpressionHandle,
) -> Option<&Identifier> {
    use omega_checked_trees::expression::ExpressionNode;

    match expressions.expression(expression) {
        ExpressionNode::Name(path) => expressions.name_path_members(path.members).first(),
        ExpressionNode::Member(member) => assignment_target_head_name(expressions, member.receiver),
        ExpressionNode::Indexed(indexed) => {
            assignment_target_head_name(expressions, indexed.collection)
        }
        ExpressionNode::Mutable(inner) => assignment_target_head_name(expressions, *inner),
        _ => None,
    }
}

fn statement_uses_symbol_mutably(
    expressions: &omega_checked_trees::expression::ExpressionTable,
    statement_table: &StatementTable,
    statement: &StatementNode,
    symbol: SymbolHandle,
    local_name: &Identifier,
) -> bool {
    match statement {
        StatementNode::Assignment(assignment) => {
            expression_uses_symbol_mutably(expressions, assignment.value, symbol, local_name)
        }
        StatementNode::Call(call) => statement_table
            .expression_handles(call.arguments)
            .iter()
            .copied()
            .any(|expression| {
                expression_uses_symbol_mutably(expressions, expression, symbol, local_name)
            }),
        StatementNode::Expression(expression) => {
            expression_uses_symbol_mutably(expressions, *expression, symbol, local_name)
        }
        StatementNode::LocalData(local_data) => {
            local_data.initial_value.is_valid()
                && expression_uses_symbol_mutably(
                    expressions,
                    local_data.initial_value,
                    symbol,
                    local_name,
                )
        }
        StatementNode::Transition(transition) => {
            transition_guard_uses_symbol_mutably(expressions, transition.guard, symbol, local_name)
                || transition_target_uses_symbol_mutably(
                    expressions,
                    statement_table,
                    transition.target,
                    symbol,
                    local_name,
                )
                || transition_target_uses_symbol_mutably(
                    expressions,
                    statement_table,
                    transition.continuation,
                    symbol,
                    local_name,
                )
        }
    }
}

fn transition_guard_uses_symbol_mutably(
    expressions: &omega_checked_trees::expression::ExpressionTable,
    guard: TransitionGuardNode,
    symbol: SymbolHandle,
    local_name: &Identifier,
) -> bool {
    match guard {
        TransitionGuardNode::Always => false,
        TransitionGuardNode::When(expression) => {
            expression_uses_symbol_mutably(expressions, expression, symbol, local_name)
        }
    }
}

fn transition_guard_references_symbol(
    expressions: &omega_checked_trees::expression::ExpressionTable,
    guard: TransitionGuardNode,
    symbol: SymbolHandle,
    local_name: &Identifier,
) -> bool {
    match guard {
        TransitionGuardNode::Always => false,
        TransitionGuardNode::When(expression) => {
            expression_references_symbol(expressions, expression, symbol, local_name)
        }
    }
}

fn transition_target_uses_symbol_mutably(
    expressions: &omega_checked_trees::expression::ExpressionTable,
    statement_table: &StatementTable,
    target: omega_checked_trees::statement::TransitionTargetHandle,
    symbol: SymbolHandle,
    local_name: &Identifier,
) -> bool {
    if !target.is_valid() {
        return false;
    }

    match statement_table.transition_target(target) {
        TransitionTargetNode::Named { arguments, .. } => statement_table
            .expression_handles(*arguments)
            .iter()
            .copied()
            .any(|expression| {
                expression_uses_symbol_mutably(expressions, expression, symbol, local_name)
            }),
        TransitionTargetNode::Value(expression) => {
            expression_uses_symbol_mutably(expressions, *expression, symbol, local_name)
        }
        TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => false,
    }
}

fn transition_target_references_symbol(
    expressions: &omega_checked_trees::expression::ExpressionTable,
    statement_table: &StatementTable,
    target: omega_checked_trees::statement::TransitionTargetHandle,
    symbol: SymbolHandle,
    local_name: &Identifier,
) -> bool {
    if !target.is_valid() {
        return false;
    }

    match statement_table.transition_target(target) {
        TransitionTargetNode::Named { arguments, .. } => statement_table
            .expression_handles(*arguments)
            .iter()
            .copied()
            .any(|expression| {
                expression_references_symbol(expressions, expression, symbol, local_name)
            }),
        TransitionTargetNode::Value(expression) => {
            expression_references_symbol(expressions, *expression, symbol, local_name)
        }
        TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => false,
    }
}

fn expression_uses_symbol_mutably(
    expressions: &omega_checked_trees::expression::ExpressionTable,
    expression: ExpressionHandle,
    symbol: SymbolHandle,
    local_name: &Identifier,
) -> bool {
    match expressions.expression(expression) {
        ExpressionNode::Mutable(inner) => {
            expression_references_symbol(expressions, *inner, symbol, local_name)
        }
        ExpressionNode::ArrayLiteral(items) => expressions
            .expression_handles(*items)
            .iter()
            .copied()
            .any(|item| expression_uses_symbol_mutably(expressions, item, symbol, local_name)),
        ExpressionNode::Binary(binary) => {
            expression_uses_symbol_mutably(expressions, binary.left, symbol, local_name)
                || expression_uses_symbol_mutably(expressions, binary.right, symbol, local_name)
        }
        ExpressionNode::Call(call) => {
            (call.receiver.is_valid()
                && expression_uses_symbol_mutably(expressions, call.receiver, symbol, local_name))
                || expressions
                    .expression_handles(call.arguments)
                    .iter()
                    .copied()
                    .any(|argument| {
                        expression_uses_symbol_mutably(expressions, argument, symbol, local_name)
                    })
        }
        ExpressionNode::Cast(cast) => {
            expression_uses_symbol_mutably(expressions, cast.value, symbol, local_name)
        }
        ExpressionNode::Indexed(indexed) => {
            expression_uses_symbol_mutably(expressions, indexed.collection, symbol, local_name)
                || expression_uses_symbol_mutably(expressions, indexed.index, symbol, local_name)
        }
        ExpressionNode::Range(range) => {
            expression_uses_symbol_mutably(expressions, range.start, symbol, local_name)
                || expression_uses_symbol_mutably(expressions, range.end, symbol, local_name)
        }
        ExpressionNode::Member(member) => {
            expression_uses_symbol_mutably(expressions, member.receiver, symbol, local_name)
        }
        ExpressionNode::StructLiteral(struct_literal) => expressions
            .struct_fields(struct_literal.fields)
            .iter()
            .any(|field| {
                expression_uses_symbol_mutably(expressions, field.value, symbol, local_name)
            }),
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_) => false,
    }
}

fn expression_references_symbol(
    expressions: &omega_checked_trees::expression::ExpressionTable,
    expression: ExpressionHandle,
    symbol: SymbolHandle,
    local_name: &Identifier,
) -> bool {
    match expressions.expression(expression) {
        ExpressionNode::Name(path) => {
            path.head_symbol == symbol
                || expressions
                    .name_path_members(path.members)
                    .first()
                    .is_some_and(|name| name == local_name)
        }
        ExpressionNode::Mutable(inner) => {
            expression_references_symbol(expressions, *inner, symbol, local_name)
        }
        ExpressionNode::ArrayLiteral(items) => expressions
            .expression_handles(*items)
            .iter()
            .copied()
            .any(|item| expression_references_symbol(expressions, item, symbol, local_name)),
        ExpressionNode::Binary(binary) => {
            expression_references_symbol(expressions, binary.left, symbol, local_name)
                || expression_references_symbol(expressions, binary.right, symbol, local_name)
        }
        ExpressionNode::Call(call) => {
            (call.receiver.is_valid()
                && expression_references_symbol(expressions, call.receiver, symbol, local_name))
                || expressions
                    .expression_handles(call.arguments)
                    .iter()
                    .copied()
                    .any(|argument| {
                        expression_references_symbol(expressions, argument, symbol, local_name)
                    })
        }
        ExpressionNode::Cast(cast) => {
            expression_references_symbol(expressions, cast.value, symbol, local_name)
        }
        ExpressionNode::Indexed(indexed) => {
            expression_references_symbol(expressions, indexed.collection, symbol, local_name)
                || expression_references_symbol(expressions, indexed.index, symbol, local_name)
        }
        ExpressionNode::Range(range) => {
            expression_references_symbol(expressions, range.start, symbol, local_name)
                || expression_references_symbol(expressions, range.end, symbol, local_name)
        }
        ExpressionNode::Member(member) => {
            expression_references_symbol(expressions, member.receiver, symbol, local_name)
        }
        ExpressionNode::StructLiteral(struct_literal) => expressions
            .struct_fields(struct_literal.fields)
            .iter()
            .any(|field| {
                expression_references_symbol(expressions, field.value, symbol, local_name)
            }),
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::String(_) => false,
    }
}

fn append_type_reference_invariant_names(
    program: &CheckedTrees,
    type_reference: TypeReferenceHandle,
    names: &mut Arena<Identifier>,
) -> HandleSpan<Identifier> {
    let mut span = HandleSpan::empty();
    collect_type_reference_invariant_names(program, type_reference, names, &mut span);
    span
}

fn collect_type_reference_invariant_names(
    program: &CheckedTrees,
    type_reference: TypeReferenceHandle,
    names: &mut Arena<Identifier>,
    span: &mut HandleSpan<Identifier>,
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
