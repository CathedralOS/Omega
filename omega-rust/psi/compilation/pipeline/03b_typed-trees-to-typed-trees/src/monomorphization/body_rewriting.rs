//! Substitute instance types and rewrite calls with exact lexical subjects.
//!
//! This file rewrites one selected call and its runtime subjects.
//! `type_parameter_substitution.rs` substitutes cloned type parameters,
//! `const_substitution.rs` substitutes fixed-array and const index
//! parameters, `machine_arguments.rs` substitutes machine parameter types
//! and forwarded arguments, `evidence_rewrites.rs` rewrites evidence
//! arguments and requirements and `cloned_calls.rs` rewrites cloned calls
//! and static machine transition targets.

use validation::collect_statement_expression_trees;

mod cloned_calls;
mod const_substitution;
mod evidence_rewrites;
mod machine_arguments;
mod type_parameter_substitution;

pub(crate) use cloned_calls::{rewrite_cloned_calls, statement_span_handles};

pub(crate) use evidence_rewrites::collect_evidence_requirement_closure;
pub(crate) use evidence_rewrites::{evidence_argument_rewrites, rewrite_index_algebra};
pub(crate) use machine_arguments::{
    forwarded_static_argument_rewrites, static_const_literal_from_type_reference,
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
    program: &dyn ::validation::CheckingInput,
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
    program: &dyn ::validation::CheckingInput,
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
                let Some(call) = program.expression_table.call_node(expression) else {
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

pub(crate) fn insert_subject_name(
    table: &mut typed_trees::expression::ExpressionTable,
    member: typed_trees::name::Identifier,
    symbol: SymbolHandle,
) -> ExpressionHandle {
    let mut members = HandleSpan::empty();
    table.push_name_path_member(&mut members, member);
    let mut member_symbols = HandleSpan::empty();
    table.push_name_path_member_symbol(&mut member_symbols, symbol);
    table.insert(ExpressionNode::Name(
        typed_trees::expression::TableNamePath::new(members, member_symbols, symbol, symbol),
    ))
}

/// One `Name` argument per subject appended to `arguments` — the
/// rewritten-call sites all grow the retained argument vector the same way
/// before reinserting it as a handle span.
pub(super) fn insert_subject_names(
    table: &mut typed_trees::expression::ExpressionTable,
    arguments: &mut Vec<ExpressionHandle>,
    subjects: &[(typed_trees::name::Identifier, SymbolHandle)],
) {
    for (member, symbol) in subjects.iter().cloned() {
        arguments.push(insert_subject_name(table, member, symbol));
    }
}

/// The statement call's retained argument vector plus one `Name` expression
/// per subject, reinserted as a handle span.
pub(super) fn insert_arguments_with_subjects(
    program: &mut TypedTrees,
    arguments_span: HandleSpan<ExpressionHandle>,
    subjects: &[(typed_trees::name::Identifier, SymbolHandle)],
) -> HandleSpan<ExpressionHandle> {
    let mut arguments = program
        .statement_table
        .expression_handles(arguments_span)
        .to_vec();
    insert_subject_names(&mut program.expression_table, &mut arguments, subjects);
    program.statement_table.insert_expression_handles(arguments)
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
            let authored_machine_application = if snapshot.authored_machine_application.is_valid() {
                snapshot.authored_machine_application
            } else {
                program.expression_table.retain_static_machine_application(
                    snapshot.target_symbol,
                    snapshot.target.clone(),
                    &snapshot.machine_arguments,
                )
            };
            let new_arguments =
                insert_arguments_with_subjects(program, snapshot.arguments, subjects);
            let StatementNode::Call(call) = program.statement_table.statement_mut(handle) else {
                unreachable!();
            };
            call.target_symbol = target;
            call.target = target_name;
            call.arguments = new_arguments;
            call.authored_machine_application = authored_machine_application;
            call.machine_arguments = Box::default();
        }
        CallSite::Expression(handle) => {
            let ExpressionNode::Call(snapshot) =
                program.expression_table.expression(handle).clone()
            else {
                return;
            };
            let authored_machine_application = if snapshot.authored_machine_application.is_valid() {
                snapshot.authored_machine_application
            } else {
                program.expression_table.retain_static_machine_application(
                    snapshot.target_symbol,
                    snapshot.target.clone(),
                    &snapshot.machine_arguments,
                )
            };
            let mut arguments = program
                .expression_table
                .expression_handles(snapshot.arguments)
                .to_vec();
            insert_subject_names(&mut program.expression_table, &mut arguments, subjects);
            let new_arguments = program
                .expression_table
                .insert_expression_handles(arguments);
            let ExpressionNode::Call(call) = program.expression_table.expression_mut(handle) else {
                unreachable!();
            };
            call.target_symbol = target;
            call.target = target_name;
            call.arguments = new_arguments;
            call.authored_machine_application = authored_machine_application;
            call.machine_arguments = Box::default();
        }
    }
}
