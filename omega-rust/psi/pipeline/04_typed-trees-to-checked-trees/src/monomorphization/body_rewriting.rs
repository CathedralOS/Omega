//! Substitute instance types and rewrite calls with exact lexical subjects.
//!
//! This file rewrites one selected call and its runtime subjects.
//! `type_parameter_substitution.rs` substitutes cloned type parameters,
//! `const_substitution.rs` substitutes fixed-array and const index
//! parameters, `machine_arguments.rs` substitutes machine parameter types
//! and forwarded arguments, `evidence_rewrites.rs` rewrites evidence
//! arguments and requirements and `cloned_calls.rs` rewrites cloned calls
//! and static machine transition targets.

mod cloned_calls;
mod const_substitution;
mod evidence_rewrites;
mod machine_arguments;
mod type_parameter_substitution;

pub(crate) use cloned_calls::{rewrite_cloned_calls, statement_span_handles};
pub(crate) use const_substitution::collect_statement_expression_trees;
pub(crate) use machine_arguments::{
    forwarded_static_argument_rewrites, remap_machine_argument_symbols,
    static_const_literal_from_type_reference, substitute_forwarded_machine_arguments,
};
pub(crate) use type_parameter_substitution::{
    cloned_expression_roots, rebind_state_scoped_range_endpoints,
    reject_runtime_bound_static_occurrences, substitute_cloned_type_parameters,
};

use super::{
    CallSite, ExpressionHandle, ExpressionNode, HandleSpan, StatementNode, StaticMachineArgument,
    SymbolHandle, TypedTrees,
};
use crate::monomorphization::const_arguments;
use crate::monomorphization::selection::state_by_symbol;

pub(super) fn rewrite_selected_call(
    program: &mut TypedTrees,
    site: CallSite,
    target: SymbolHandle,
    subjects: &[(typed_trees::name::Identifier, SymbolHandle)],
) {
    let target_name = state_by_symbol(program, target)
        .map(|state| state.name.clone())
        .expect("cloned specialization state");
    rewrite_selected_call_with_name(program, site, target, target_name, subjects);
}

/// Runtime subjects bound to `Value` binder slots at one call site, in
/// telescope order. A rewritten call passes each as an appended ordinary
/// argument matching the specialization's realized trailing parameters.
pub(super) fn runtime_value_subjects(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    machine_arguments: &[StaticMachineArgument],
) -> Vec<(typed_trees::name::Identifier, SymbolHandle)> {
    machine_arguments
        .iter()
        .filter_map(|argument| {
            let symbol =
                const_arguments::resolve_runtime_subject(program, state, usize::MAX, argument)?;
            Some((argument.path.first()?.clone(), symbol))
        })
        .collect()
}

/// Resolve runtime subjects while their cloned statement owner is in hand.
/// Only sibling calls in the new expression region need these subjects; owned
/// initializers and contract-only expressions deliberately acquire no body owner.
pub(super) fn cloned_runtime_call_subjects(
    program: &TypedTrees,
    states: HandleSpan<typed_trees::state::State>,
    state_symbols: &[(SymbolHandle, SymbolHandle)],
    expression_start: usize,
) -> Vec<(
    ExpressionHandle,
    Vec<(typed_trees::name::Identifier, SymbolHandle)>,
)> {
    let mut calls = Vec::new();
    let mut expressions = Vec::new();
    for state in program.machine_states.span_or_empty(states) {
        for statement in program.statement_table.statements(state.statement_nodes) {
            expressions.clear();
            collect_statement_expression_trees(program, statement, &mut expressions);
            for expression in expressions.iter().copied() {
                if (expression.arena_index() as usize) < expression_start {
                    continue;
                }
                let ExpressionNode::Call(call) = program.expression_table.expression(expression)
                else {
                    continue;
                };
                if state_symbols
                    .iter()
                    .any(|(_, target)| *target == call.target_symbol)
                {
                    calls.push((
                        expression,
                        runtime_value_subjects(program, state, &call.machine_arguments),
                    ));
                }
            }
        }
    }
    // Shared expression references retain the first lexical owner, as the
    // original owner lookup did. Generation remains part of handle identity.
    calls.sort_by_key(|(expression, _)| (expression.arena_index(), expression.generation()));
    calls.dedup_by_key(|(expression, _)| *expression);
    calls
}

pub(super) fn insert_subject_name(
    table: &mut typed_trees::expression::ExpressionTable,
    member: typed_trees::name::Identifier,
    symbol: SymbolHandle,
) -> ExpressionHandle {
    let mut members = HandleSpan::empty();
    table.push_name_path_member(&mut members, member);
    let mut member_symbols = HandleSpan::empty();
    table.push_name_path_member_symbol(&mut member_symbols, symbol);
    table.insert(ExpressionNode::Name(
        typed_trees::expression::TableNamePath {
            members,
            member_symbols,
            head_symbol: symbol,
            symbol,
        },
    ))
}

pub(super) fn rewrite_selected_call_with_name(
    program: &mut TypedTrees,
    site: CallSite,
    target: SymbolHandle,
    target_name: typed_trees::name::Identifier,
    subjects: &[(typed_trees::name::Identifier, SymbolHandle)],
) {
    match site {
        CallSite::Statement(handle) => {
            let StatementNode::Call(snapshot) = program.statement_table.statement(handle).clone()
            else {
                return;
            };
            let mut arguments = program
                .statement_table
                .expression_handles(snapshot.arguments)
                .to_vec();
            for (member, symbol) in subjects.iter().cloned() {
                arguments.push(insert_subject_name(
                    &mut program.expression_table,
                    member,
                    symbol,
                ));
            }
            let new_arguments = program.statement_table.insert_expression_handles(arguments);
            let StatementNode::Call(call) = program.statement_table.statement_mut(handle) else {
                unreachable!();
            };
            call.target_symbol = target;
            call.target = target_name;
            call.arguments = new_arguments;
            call.machine_arguments = Box::default();
        }
        CallSite::Expression(handle) => {
            let ExpressionNode::Call(snapshot) =
                program.expression_table.expression(handle).clone()
            else {
                return;
            };
            let mut arguments = program
                .expression_table
                .expression_handles(snapshot.arguments)
                .to_vec();
            for (member, symbol) in subjects.iter().cloned() {
                arguments.push(insert_subject_name(
                    &mut program.expression_table,
                    member,
                    symbol,
                ));
            }
            let new_arguments = program
                .expression_table
                .insert_expression_handles(arguments);
            let ExpressionNode::Call(call) = program.expression_table.expression_mut(handle) else {
                unreachable!();
            };
            call.target_symbol = target;
            call.target = target_name;
            call.arguments = new_arguments;
            call.machine_arguments = Box::default();
        }
    }
}
