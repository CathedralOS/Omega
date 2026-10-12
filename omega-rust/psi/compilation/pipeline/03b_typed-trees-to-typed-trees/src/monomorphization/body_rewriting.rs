//! Substitute instance types and rewrite calls with exact lexical subjects.
//!
//! This file rewrites one selected call and its runtime subjects.
//! `type_parameter_substitution.rs` substitutes cloned type parameters,
//! `const_substitution.rs` substitutes fixed-array and const index
//! parameters, and `cloned_calls.rs` consumes shared exact
//! application evidence while rewriting cloned calls
//! and static machine transition targets.

mod cloned_calls;
mod const_substitution;
mod type_parameter_substitution;

pub(crate) use cloned_calls::{rewrite_cloned_calls, statement_span_handles};

pub(crate) use super::application_arguments::{
    forwarded_static_argument_rewrites, substitute_forwarded_machine_arguments,
};
pub(crate) use type_parameter_substitution::{
    cloned_expression_roots, rebind_state_scoped_range_endpoints,
    reject_runtime_bound_static_occurrences, substitute_cloned_type_parameters,
};

use super::{
    CallSite, ExpressionHandle, ExpressionNode, HandleSpan, StatementNode, SymbolHandle, TypedTrees,
};
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
