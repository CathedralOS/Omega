use std::sync::Arc;

use omega_core::arena::{Arena, HandleSpan};
use omega_core::source::SourceMap;
use omega_core::symbols::{SymbolHandle, SymbolKind, SymbolTable};
use omega_symbol_resolved_trees::SymbolResolvedTrees;

mod symbol_table;

use lookup::{
    call_target_for_attached_data, child_indexed_symbol_by_kinds,
    child_or_attached_data_child_symbol_by_kinds, child_symbol_by_kinds, top_level_symbol_by_kinds,
};
use symbol_table::build_symbol_table;
use type_references::{
    assign_type_reference_symbol_with_self_type, assign_type_reference_symbols,
    call_target_for_type_reference,
};

mod domain_facts;
mod lookup;
mod scope;
mod scoped_paths;
mod top_level;
mod type_references;

use domain_facts::assign_domain_fact_symbols;
use scope::MachineScope;
use scoped_paths::{
    invalid_symbol_pair, resolve_state_scoped_members, resolve_state_scoped_table_path,
    resolve_state_scoped_table_path_with_indexed_last_member,
};
use top_level::assign_top_level_symbols;

pub(crate) fn assign_symbols(program: &mut SymbolResolvedTrees, sources: Option<Arc<SourceMap>>) {
    let symbols = build_symbol_table(program, sources);
    assign_top_level_symbols(program, &symbols);
    assign_type_reference_symbols(program, &symbols);
    assign_domain_fact_symbols(program, &symbols);
    assign_statement_call_symbols(program, &symbols);
    program.symbols = symbols;
}

fn assign_statement_call_symbols(program: &mut SymbolResolvedTrees, symbols: &SymbolTable) {
    let SymbolResolvedTrees {
        roots:
            omega_symbol_resolved_trees::SymbolResolvedRoots {
                data_definitions,
                machines,
                ..
            },
        tables,
        ..
    } = program;
    let data_members = &tables.declarations.data_members;
    let machine_contained_objects = &tables.declarations.machine_contained_objects;
    let machine_owned_data = &tables.declarations.machine_owned_data;
    let machine_state_handles = &tables.declarations.machine_state_handles;
    let machine_states = &mut tables.declarations.machine_states;
    let state_parameters = &tables.declarations.state_parameters;
    let statement_path_members = &tables.declarations.statement_path_members;
    let expression_table = &mut tables.bodies.expressions;
    let state_statements = &mut tables.declarations.state_statements;
    let child_type_references = &mut tables.declarations.child_type_references;
    machines.for_each_mut(|machine| {
        let machine_symbol = machine.symbol;
        let data_definition = machine.attached_data.as_ref().and_then(|attached_data| {
            data_definitions
                .iter()
                .find(|data_definition| data_definition.name == *attached_data)
        });
        let inherited_data_members = data_definition
            .map(|data_definition| data_members.span_or_empty(data_definition.members));
        let omega_symbol_resolved_trees::machine::MachineStorage {
            contains,
            owned_data,
            satisfies: _,
            terminates: _,
            decreases: _,
            decrease_order: _,
            effects: _,
            contracts: _,
            states,
        } = &mut machine.storage;
        let machine_scope = MachineScope {
            symbol: machine_symbol,
            attached_data: machine.attached_data.as_ref(),
            contains: machine_contained_objects.span_or_empty(*contains),
            inherited_data_members,
            owned_data: machine_owned_data.span_or_empty(*owned_data),
        };
        for state in machine_state_handles.span_or_empty(*states).iter().copied() {
            let state = machine_states.get_mut(state);
            let state_symbol = state.symbol;
            let parameters = state_parameters.span_or_empty(state.parameters);
            for statement in state_statements.span_mut_or_empty(state.statements) {
                assign_statement_symbols(
                    &machine_scope,
                    parameters,
                    state_symbol,
                    expression_table,
                    child_type_references,
                    statement_path_members,
                    statement,
                    symbols,
                );
            }
        }
    });
}

fn assign_statement_symbols(
    machine: &MachineScope<'_>,
    parameters: &[omega_symbol_resolved_trees::signature::StateParameter],
    state_symbol: SymbolHandle,
    expression_table: &mut omega_symbol_resolved_trees::expression::ExpressionTable,
    child_type_references: &mut omega_core::arena::Arena<
        omega_symbol_resolved_trees::types::TypeReference,
    >,
    statement_path_members: &Arena<omega_symbol_resolved_trees::name::DiagnosticName>,
    statement: &mut omega_symbol_resolved_trees::statement::Statement,
    symbols: &SymbolTable,
) {
    match statement {
        omega_symbol_resolved_trees::statement::Statement::Assignment(assignment) => {
            assign_statement_expression_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                assignment.target,
            );
            assign_statement_expression_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                assignment.value,
            );
        }
        omega_symbol_resolved_trees::statement::Statement::Call(call) => {
            assign_expression_span_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                call.arguments,
            );
            if !call.receiver.is_empty() {
                let (head_symbol, symbol) = resolve_state_scoped_members(
                    symbols,
                    machine.symbol,
                    state_symbol,
                    statement_path_members.span_or_empty(call.receiver),
                    call.receiver_starts_at_self,
                );
                if symbol.is_valid() {
                    let _ = head_symbol;
                    call.receiver_symbol = symbol;
                }
            }

            call.target_symbol = resolve_call_target_symbol(
                machine,
                parameters,
                !call.receiver.is_empty(),
                call.receiver_symbol,
                &call.target,
                child_type_references,
                symbols,
            );
        }
        omega_symbol_resolved_trees::statement::Statement::Expression(expression) => {
            assign_statement_expression_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                *expression,
            );
        }
        omega_symbol_resolved_trees::statement::Statement::LocalData(local_data) => {
            assign_type_reference_symbol_with_self_type(
                symbols,
                child_type_references,
                machine.symbol,
                &mut local_data.type_reference,
            );
            if local_data.initial_value.is_valid() {
                assign_statement_expression_symbols(
                    symbols,
                    machine,
                    parameters,
                    state_symbol,
                    expression_table,
                    child_type_references,
                    local_data.initial_value,
                );
            }
        }
        omega_symbol_resolved_trees::statement::Statement::Transition(transition) => {
            if let omega_symbol_resolved_trees::statement::TransitionGuard::When(expression) =
                &mut transition.guard
            {
                assign_statement_expression_symbols(
                    symbols,
                    machine,
                    parameters,
                    state_symbol,
                    expression_table,
                    child_type_references,
                    *expression,
                );
            }
            assign_transition_target_symbols(
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                statement_path_members,
                &mut transition.target,
                symbols,
            );
            if let Some(continuation) = &mut transition.continuation {
                assign_transition_target_symbols(
                    machine,
                    parameters,
                    state_symbol,
                    expression_table,
                    child_type_references,
                    statement_path_members,
                    continuation,
                    symbols,
                );
            }
        }
    }
}

fn assign_statement_expression_symbols(
    symbols: &SymbolTable,
    machine: &MachineScope<'_>,
    parameters: &[omega_symbol_resolved_trees::signature::StateParameter],
    state_symbol: SymbolHandle,
    expression_table: &mut omega_symbol_resolved_trees::expression::ExpressionTable,
    child_type_references: &mut omega_core::arena::Arena<
        omega_symbol_resolved_trees::types::TypeReference,
    >,
    expression: omega_symbol_resolved_trees::expression::ExpressionHandle,
) {
    if !expression.is_valid() {
        return;
    }

    assign_expression_table_symbols(
        symbols,
        machine,
        parameters,
        state_symbol,
        expression_table,
        child_type_references,
        expression,
    );
}

fn assign_expression_span_symbols(
    symbols: &SymbolTable,
    machine: &MachineScope<'_>,
    parameters: &[omega_symbol_resolved_trees::signature::StateParameter],
    state_symbol: SymbolHandle,
    expression_table: &mut omega_symbol_resolved_trees::expression::ExpressionTable,
    child_type_references: &mut omega_core::arena::Arena<
        omega_symbol_resolved_trees::types::TypeReference,
    >,
    expressions: HandleSpan<omega_symbol_resolved_trees::expression::ExpressionHandle>,
) {
    let count = expressions.count();
    for offset in 0..count {
        let expression = expression_table.expression_handles(expressions)[offset as usize];
        assign_expression_table_symbols(
            symbols,
            machine,
            parameters,
            state_symbol,
            expression_table,
            child_type_references,
            expression,
        );
    }
}

fn assign_expression_table_symbols(
    symbols: &SymbolTable,
    machine: &MachineScope<'_>,
    parameters: &[omega_symbol_resolved_trees::signature::StateParameter],
    state_symbol: SymbolHandle,
    expression_table: &mut omega_symbol_resolved_trees::expression::ExpressionTable,
    child_type_references: &mut omega_core::arena::Arena<
        omega_symbol_resolved_trees::types::TypeReference,
    >,
    expression: omega_symbol_resolved_trees::expression::ExpressionHandle,
) {
    if !expression.is_valid() {
        return;
    }

    match expression_table.expression(expression).clone() {
        omega_symbol_resolved_trees::expression::ExpressionNode::ArrayLiteral(values) => {
            assign_expression_span_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                values,
            );
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Binary(binary) => {
            assign_expression_table_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                binary.left,
            );
            assign_expression_table_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                binary.right,
            );
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Boolean(_)
        | omega_symbol_resolved_trees::expression::ExpressionNode::Float(_)
        | omega_symbol_resolved_trees::expression::ExpressionNode::Integer(_)
        | omega_symbol_resolved_trees::expression::ExpressionNode::String(_) => {}
        omega_symbol_resolved_trees::expression::ExpressionNode::Cast(cast) => {
            assign_expression_table_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                cast.value,
            );
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Call(call) => {
            if call.receiver.is_valid() {
                assign_expression_table_symbols(
                    symbols,
                    machine,
                    parameters,
                    state_symbol,
                    expression_table,
                    child_type_references,
                    call.receiver,
                );
            }
            assign_expression_span_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                call.arguments,
            );
            let (head_symbol, symbol) = resolve_expression_table_receiver_path_symbols(
                symbols,
                machine.symbol,
                state_symbol,
                expression_table,
                call.receiver,
            );
            if symbol.is_valid() {
                stamp_receiver_path_symbols_in_table(
                    expression_table,
                    call.receiver,
                    head_symbol,
                    symbol,
                );
            }
            let target_symbol = resolve_expression_table_call_target_symbol(
                machine,
                parameters,
                state_symbol,
                &call,
                expression_table,
                child_type_references,
                symbols,
            );
            if let omega_symbol_resolved_trees::expression::ExpressionNode::Call(call) =
                expression_table.expression_mut(expression)
            {
                call.target_symbol = target_symbol;
            }
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Indexed(indexed) => {
            assign_expression_table_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                indexed.collection,
            );
            assign_expression_table_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                indexed.index,
            );
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Range(range) => {
            if range.start.is_valid() {
                assign_expression_table_symbols(
                    symbols,
                    machine,
                    parameters,
                    state_symbol,
                    expression_table,
                    child_type_references,
                    range.start,
                );
            }
            if range.end.is_valid() {
                assign_expression_table_symbols(
                    symbols,
                    machine,
                    parameters,
                    state_symbol,
                    expression_table,
                    child_type_references,
                    range.end,
                );
            }
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Member(member) => {
            assign_expression_table_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                member.receiver,
            );
            let member_symbol = resolve_expression_table_member_symbol(
                symbols,
                machine.symbol,
                state_symbol,
                expression_table,
                member.receiver,
                &member.member,
            );
            if let (symbol, omega_symbol_resolved_trees::expression::ExpressionNode::Member(member)) =
                (member_symbol, expression_table.expression_mut(expression))
                && symbol.is_valid()
            {
                member.member_symbol = symbol;
            }
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Membership(membership) => {
            assign_expression_table_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                membership.value,
            );
            let name = expression_table
                .name_path_members(membership.domain)
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join("::");
            if let omega_symbol_resolved_trees::expression::ExpressionNode::Membership(membership) =
                expression_table.expression_mut(expression)
            {
                membership.domain_symbol = symbols
                    .find_child_by_name_and_kind(symbols.root(), &name, SymbolKind::Domain)
                    .unwrap_or_else(SymbolHandle::invalid);
            }
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Mutable(inner) => {
            assign_expression_table_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                inner,
            );
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Name(path) => {
            let (head_symbol, symbol) = resolve_state_scoped_table_path(
                symbols,
                machine.symbol,
                state_symbol,
                expression_table,
                &path,
            );
            if symbol.is_valid() {
                if let omega_symbol_resolved_trees::expression::ExpressionNode::Name(path) =
                    expression_table.expression_mut(expression)
                {
                    path.head_symbol = head_symbol;
                    path.symbol = symbol;
                }
            }
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::StructLiteral(struct_literal) => {
            let count = struct_literal.fields.count();
            for offset in 0..count {
                let field = &expression_table.struct_fields(struct_literal.fields)[offset as usize];
                assign_expression_table_symbols(
                    symbols,
                    machine,
                    parameters,
                    state_symbol,
                    expression_table,
                    child_type_references,
                    field.value,
                );
            }
        }
    }
}

fn stamp_receiver_path_symbols_in_table(
    expression_table: &mut omega_symbol_resolved_trees::expression::ExpressionTable,
    expression: omega_symbol_resolved_trees::expression::ExpressionHandle,
    head_symbol: SymbolHandle,
    symbol: SymbolHandle,
) {
    match expression_table.expression(expression).clone() {
        omega_symbol_resolved_trees::expression::ExpressionNode::Name(_) => {
            if let omega_symbol_resolved_trees::expression::ExpressionNode::Name(path) =
                expression_table.expression_mut(expression)
            {
                path.head_symbol = head_symbol;
                path.symbol = symbol;
            }
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Indexed(indexed) => {
            stamp_receiver_path_head_symbol_in_table(
                expression_table,
                indexed.collection,
                head_symbol,
            );
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Member(member) => {
            stamp_receiver_path_head_symbol_in_table(
                expression_table,
                member.receiver,
                head_symbol,
            );
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Mutable(inner) => {
            stamp_receiver_path_symbols_in_table(expression_table, inner, head_symbol, symbol);
        }
        _ => {}
    }
}

fn stamp_receiver_path_head_symbol_in_table(
    expression_table: &mut omega_symbol_resolved_trees::expression::ExpressionTable,
    expression: omega_symbol_resolved_trees::expression::ExpressionHandle,
    head_symbol: SymbolHandle,
) {
    match expression_table.expression(expression).clone() {
        omega_symbol_resolved_trees::expression::ExpressionNode::Name(_) => {
            if let omega_symbol_resolved_trees::expression::ExpressionNode::Name(path) =
                expression_table.expression_mut(expression)
            {
                path.head_symbol = head_symbol;
                path.symbol = head_symbol;
            }
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Indexed(indexed) => {
            stamp_receiver_path_head_symbol_in_table(
                expression_table,
                indexed.collection,
                head_symbol,
            );
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Member(member) => {
            stamp_receiver_path_head_symbol_in_table(
                expression_table,
                member.receiver,
                head_symbol,
            );
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Mutable(inner) => {
            stamp_receiver_path_head_symbol_in_table(expression_table, inner, head_symbol);
        }
        _ => {}
    }
}

fn resolve_expression_table_call_target_symbol(
    machine: &MachineScope<'_>,
    parameters: &[omega_symbol_resolved_trees::signature::StateParameter],
    state_symbol: SymbolHandle,
    call: &omega_symbol_resolved_trees::expression::TableCallExpression,
    expression_table: &omega_symbol_resolved_trees::expression::ExpressionTable,
    child_type_references: &omega_core::arena::Arena<
        omega_symbol_resolved_trees::types::TypeReference,
    >,
    symbols: &SymbolTable,
) -> SymbolHandle {
    if call.receiver.is_valid() {
        let receiver_symbol = resolve_expression_table_receiver_symbol(
            symbols,
            machine.symbol,
            state_symbol,
            expression_table,
            call.receiver,
        );
        return resolve_call_target_symbol(
            machine,
            parameters,
            true,
            receiver_symbol,
            &call.target,
            child_type_references,
            symbols,
        );
    }

    resolve_call_target_symbol(
        machine,
        parameters,
        false,
        SymbolHandle::invalid(),
        &call.target,
        child_type_references,
        symbols,
    )
}

fn resolve_expression_table_member_symbol(
    symbols: &SymbolTable,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    expression_table: &omega_symbol_resolved_trees::expression::ExpressionTable,
    receiver: omega_symbol_resolved_trees::expression::ExpressionHandle,
    member: &omega_symbol_resolved_trees::name::DiagnosticName,
) -> SymbolHandle {
    let receiver_symbol = resolve_expression_table_receiver_symbol(
        symbols,
        machine_symbol,
        state_symbol,
        expression_table,
        receiver,
    );
    if !receiver_symbol.is_valid() {
        return SymbolHandle::invalid();
    }
    let member_symbol = child_or_attached_data_child_symbol_by_kinds(
        symbols,
        receiver_symbol,
        &[
            SymbolKind::Field,
            SymbolKind::Object,
            SymbolKind::State,
            SymbolKind::Parameter,
            SymbolKind::Variant,
        ],
        member.as_str(),
    );

    member_symbol
}

fn resolve_expression_table_receiver_path_symbols(
    symbols: &SymbolTable,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    expression_table: &omega_symbol_resolved_trees::expression::ExpressionTable,
    receiver: omega_symbol_resolved_trees::expression::ExpressionHandle,
) -> (SymbolHandle, SymbolHandle) {
    match expression_table.expression(receiver) {
        omega_symbol_resolved_trees::expression::ExpressionNode::Name(path) => {
            resolve_state_scoped_table_path(
                symbols,
                machine_symbol,
                state_symbol,
                expression_table,
                path,
            )
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Member(member) => {
            let (head_symbol, receiver_symbol) = resolve_expression_table_receiver_path_symbols(
                symbols,
                machine_symbol,
                state_symbol,
                expression_table,
                member.receiver,
            );
            if !receiver_symbol.is_valid() {
                return invalid_symbol_pair();
            }
            let member_symbol = child_or_attached_data_child_symbol_by_kinds(
                symbols,
                receiver_symbol,
                &[
                    SymbolKind::Field,
                    SymbolKind::Object,
                    SymbolKind::State,
                    SymbolKind::Parameter,
                    SymbolKind::Variant,
                ],
                member.member.as_str(),
            );

            if member_symbol.is_valid() {
                (head_symbol, member_symbol)
            } else {
                invalid_symbol_pair()
            }
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Mutable(inner) => {
            resolve_expression_table_receiver_path_symbols(
                symbols,
                machine_symbol,
                state_symbol,
                expression_table,
                *inner,
            )
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Indexed(indexed) => {
            let omega_symbol_resolved_trees::expression::ExpressionNode::Integer(index) =
                expression_table.expression(indexed.index)
            else {
                return invalid_symbol_pair();
            };

            resolve_indexed_expression_table_receiver_path_symbols(
                symbols,
                machine_symbol,
                state_symbol,
                expression_table,
                indexed.collection,
                *index,
            )
        }
        _ => invalid_symbol_pair(),
    }
}

fn resolve_indexed_expression_table_receiver_path_symbols(
    symbols: &SymbolTable,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    expression_table: &omega_symbol_resolved_trees::expression::ExpressionTable,
    collection: omega_symbol_resolved_trees::expression::ExpressionHandle,
    index: i64,
) -> (SymbolHandle, SymbolHandle) {
    match expression_table.expression(collection) {
        omega_symbol_resolved_trees::expression::ExpressionNode::Name(path) => {
            resolve_state_scoped_table_path_with_indexed_last_member(
                symbols,
                machine_symbol,
                state_symbol,
                expression_table,
                path,
                index,
            )
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Member(member) => {
            let (head_symbol, receiver_symbol) = resolve_expression_table_receiver_path_symbols(
                symbols,
                machine_symbol,
                state_symbol,
                expression_table,
                member.receiver,
            );
            if !receiver_symbol.is_valid() {
                return invalid_symbol_pair();
            }
            let member_symbol = child_indexed_symbol_by_kinds(
                symbols,
                receiver_symbol,
                &[
                    SymbolKind::Field,
                    SymbolKind::Object,
                    SymbolKind::State,
                    SymbolKind::Parameter,
                    SymbolKind::Variant,
                ],
                member.member.as_str(),
                index,
            );

            if member_symbol.is_valid() {
                (head_symbol, member_symbol)
            } else {
                invalid_symbol_pair()
            }
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Mutable(inner) => {
            resolve_indexed_expression_table_receiver_path_symbols(
                symbols,
                machine_symbol,
                state_symbol,
                expression_table,
                *inner,
                index,
            )
        }
        _ => invalid_symbol_pair(),
    }
}

fn resolve_expression_table_receiver_symbol(
    symbols: &SymbolTable,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    expression_table: &omega_symbol_resolved_trees::expression::ExpressionTable,
    receiver: omega_symbol_resolved_trees::expression::ExpressionHandle,
) -> SymbolHandle {
    match expression_table.expression(receiver) {
        omega_symbol_resolved_trees::expression::ExpressionNode::Name(path) => {
            let (_, symbol) = resolve_state_scoped_table_path(
                symbols,
                machine_symbol,
                state_symbol,
                expression_table,
                path,
            );
            symbol
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Member(member) => {
            resolve_expression_table_member_symbol(
                symbols,
                machine_symbol,
                state_symbol,
                expression_table,
                member.receiver,
                &member.member,
            )
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Mutable(inner) => {
            resolve_expression_table_receiver_symbol(
                symbols,
                machine_symbol,
                state_symbol,
                expression_table,
                *inner,
            )
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Indexed(_) => {
            let (_, symbol) = resolve_expression_table_receiver_path_symbols(
                symbols,
                machine_symbol,
                state_symbol,
                expression_table,
                receiver,
            );
            symbol
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Range(_) => {
            SymbolHandle::invalid()
        }
        _ => SymbolHandle::invalid(),
    }
}

fn assign_transition_target_symbols(
    machine: &MachineScope<'_>,
    parameters: &[omega_symbol_resolved_trees::signature::StateParameter],
    state_symbol: SymbolHandle,
    expression_table: &mut omega_symbol_resolved_trees::expression::ExpressionTable,
    child_type_references: &mut omega_core::arena::Arena<
        omega_symbol_resolved_trees::types::TypeReference,
    >,
    statement_path_members: &Arena<omega_symbol_resolved_trees::name::DiagnosticName>,
    target: &mut omega_symbol_resolved_trees::statement::TransitionTarget,
    symbols: &SymbolTable,
) {
    let omega_symbol_resolved_trees::statement::TransitionTarget::Named(named) = target else {
        if let omega_symbol_resolved_trees::statement::TransitionTarget::Value(expression) = target
        {
            assign_statement_expression_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                *expression,
            );
        }
        return;
    };

    assign_expression_span_symbols(
        symbols,
        machine,
        parameters,
        state_symbol,
        expression_table,
        child_type_references,
        named.arguments,
    );

    let path = statement_path_members.span_or_empty(named.path);
    let target_name = path.last().cloned();
    let (head_symbol, symbol) = resolve_state_scoped_members(
        symbols,
        machine.symbol,
        state_symbol,
        path,
        named.path_starts_at_self,
    );
    if symbol.is_valid() {
        named.head_symbol = head_symbol;
        named.symbol = symbol;
        return;
    }

    let Some(target_name) = target_name else {
        return;
    };

    if path.len() <= 2 {
        let target_symbol = child_symbol_by_kinds(
            symbols,
            machine.symbol,
            &[SymbolKind::State],
            target_name.as_str(),
        );
        if target_symbol.is_valid() {
            named.head_symbol = target_symbol;
            named.symbol = target_symbol;
            return;
        }

        if named.path_starts_at_self
            && let Some(attached_data) = machine.attached_data
        {
            let target_symbol = call_target_for_attached_data(
                symbols,
                attached_data.as_str(),
                target_name.as_str(),
            );
            if target_symbol.is_valid() {
                named.head_symbol = machine.symbol;
                named.symbol = target_symbol;
            }
        }
    }
}

fn resolve_call_target_symbol(
    machine: &MachineScope<'_>,
    parameters: &[omega_symbol_resolved_trees::signature::StateParameter],
    has_receiver: bool,
    receiver_symbol: SymbolHandle,
    target: &omega_symbol_resolved_trees::name::DiagnosticName,
    child_type_references: &omega_core::arena::Arena<
        omega_symbol_resolved_trees::types::TypeReference,
    >,
    symbols: &SymbolTable,
) -> SymbolHandle {
    if has_receiver {
        if receiver_symbol.is_valid() {
            if let Some(contained) = machine
                .contains
                .iter()
                .find(|contained| contained.symbol == receiver_symbol)
            {
                return child_symbol_by_kinds(
                    symbols,
                    contained.type_symbol,
                    &[SymbolKind::State],
                    target.as_str(),
                );
            }

            if let Some(field_type_reference) =
                machine.field_type_reference(symbols, receiver_symbol)
            {
                let symbol = call_target_for_type_reference(
                    symbols,
                    child_type_references,
                    field_type_reference,
                    target.as_str(),
                );
                return symbol;
            }

            let receiver_kind = symbols.get(receiver_symbol).kind;
            if let Some(parameter) = parameters
                .iter()
                .find(|parameter| parameter.symbol == receiver_symbol)
            {
                let direct = call_target_for_type_reference(
                    symbols,
                    child_type_references,
                    &parameter.type_reference,
                    target.as_str(),
                );
                if direct.is_valid() {
                    return direct;
                }
            }
            if matches!(receiver_kind, SymbolKind::BuiltinType) {
                return child_symbol_by_kinds(
                    symbols,
                    receiver_symbol,
                    &[SymbolKind::BuiltinFunction],
                    target.as_str(),
                );
            }
            if matches!(
                receiver_kind,
                SymbolKind::Machine | SymbolKind::Platform | SymbolKind::Trait
            ) {
                if receiver_symbol == machine.symbol
                    && let Some(attached_data) = machine.attached_data
                {
                    let target_symbol = call_target_for_attached_data(
                        symbols,
                        attached_data.as_str(),
                        target.as_str(),
                    );
                    if target_symbol.is_valid() {
                        return target_symbol;
                    }
                }

                return child_symbol_by_kinds(
                    symbols,
                    receiver_symbol,
                    &[SymbolKind::State],
                    target.as_str(),
                );
            }
        }
    }

    let machine_state = child_symbol_by_kinds(
        symbols,
        machine.symbol,
        &[SymbolKind::State],
        target.as_str(),
    );
    if machine_state.is_valid() {
        return machine_state;
    }

    top_level_symbol_by_kinds(symbols, &[SymbolKind::BuiltinFunction], target.as_str())
}
