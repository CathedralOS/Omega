use std::sync::Arc;

use omega_core::arena::{Arena, HandleSpan};
use omega_core::source::SourceMap;
use omega_core::symbols::{SymbolHandle, SymbolKind, SymbolTable};
use omega_symbol_resolved_trees::SymbolResolvedTrees;

mod symbol_table;

use lookup::{call_target_for_attached_data, child_symbol_by_kinds, top_level_symbol_by_kinds};
use symbol_table::build_symbol_table;
use type_references::{assign_type_reference_symbols, call_target_for_type_reference};

mod domain_facts;
mod expression_paths;
mod lookup;
mod scope;
mod scoped_paths;
mod statements;
mod top_level;
mod type_references;

use domain_facts::assign_domain_fact_symbols;
use expression_paths::{
    resolve_expression_table_call_target_symbol, resolve_expression_table_member_symbol,
    resolve_expression_table_receiver_path_symbols, stamp_receiver_path_symbols_in_table,
};
use scope::MachineScope;
use scoped_paths::{resolve_state_scoped_members, resolve_state_scoped_table_path};
use statements::assign_statement_call_symbols;
use top_level::assign_top_level_symbols;

pub(crate) fn assign_symbols(program: &mut SymbolResolvedTrees, sources: Option<Arc<SourceMap>>) {
    let symbols = build_symbol_table(program, sources);
    assign_top_level_symbols(program, &symbols);
    assign_type_reference_symbols(program, &symbols);
    assign_domain_fact_symbols(program, &symbols);
    assign_statement_call_symbols(program, &symbols);
    program.symbols = symbols;
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
