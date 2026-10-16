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
pub(crate) use machine_arguments::forwarded_static_argument_rewrites;
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
use typed_trees::expression::TableMemberExpression;

pub(super) fn rewrite_selected_call(
    program: &mut TypedTrees,
    site: CallSite,
    target: SymbolHandle,
    subjects: &[ExpressionHandle],
) {
    let target_name = state_by_symbol(program, target)
        .map(|state| state.name.clone())
        .expect("cloned specialization state");
    rewrite_selected_call_with_name(program, site, target, target_name, subjects);
}

/// One runtime `Value` operand, in telescope order. A name, a field of a
/// local or parameter, and a call result are the same trailing argument:
/// each is evaluated once and passed to the shared body. The parser's
/// evidence spelling is only the field form; it is not a second operation.
pub(super) fn runtime_value_operands(
    program: &mut TypedTrees,
    state_symbol: SymbolHandle,
    machine_arguments: &[StaticMachineArgument],
) -> Vec<ExpressionHandle> {
    let plans = {
        let Some(state) = state_by_symbol(program, state_symbol) else {
            return Vec::new();
        };
        machine_arguments
            .iter()
            .filter_map(|argument| plan_runtime_operand(program, state, argument))
            .collect::<Vec<_>>()
    };
    plans
        .into_iter()
        .map(|plan| realize_operand(program, plan))
        .collect()
}

enum OperandPlan {
    Existing(ExpressionHandle),
    Name(typed_trees::name::Identifier, SymbolHandle),
    Field {
        receiver_name: typed_trees::name::Identifier,
        receiver: SymbolHandle,
        field_name: typed_trees::name::Identifier,
        field: SymbolHandle,
    },
}

fn plan_runtime_operand(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    argument: &StaticMachineArgument,
) -> Option<OperandPlan> {
    if let Some(expression) = const_arguments::runtime_subject_expression(program, argument) {
        return Some(OperandPlan::Existing(expression));
    }
    if let Some(projection) = &argument.evidence_projection {
        let subject = validation::runtime_field_subject(
            program,
            state,
            &projection.term,
            &projection.member,
        )?;
        return Some(OperandPlan::Field {
            receiver_name: projection.term.clone(),
            receiver: subject.receiver,
            field_name: projection.member.clone(),
            field: subject.field,
        });
    }
    let symbol = const_arguments::resolve_runtime_subject(program, state, usize::MAX, argument)?;
    Some(OperandPlan::Name(argument.path.first()?.clone(), symbol))
}

fn realize_operand(program: &mut TypedTrees, plan: OperandPlan) -> ExpressionHandle {
    match plan {
        OperandPlan::Existing(expression) => expression,
        OperandPlan::Name(name, symbol) => {
            insert_subject_name(&mut program.expression_table, name, symbol)
        }
        OperandPlan::Field {
            receiver_name,
            receiver,
            field_name,
            field,
        } => {
            let receiver =
                insert_subject_name(&mut program.expression_table, receiver_name, receiver);
            program
                .expression_table
                .insert(ExpressionNode::Member(TableMemberExpression {
                    receiver,
                    member_symbol: field,
                    member: field_name,
                    case_variant: None,
                }))
        }
    }
}

/// Resolve runtime subjects while their cloned statement owner is in hand.
/// Only sibling calls in the new expression region need these subjects; owned
/// initializers and contract-only expressions deliberately acquire no body owner.
pub(super) fn cloned_runtime_call_subjects(
    program: &mut TypedTrees,
    states: HandleSpan<typed_trees::state::State>,
    state_symbols: &[(SymbolHandle, SymbolHandle)],
    expression_start: usize,
) -> Vec<(ExpressionHandle, Vec<ExpressionHandle>)> {
    let mut pending = Vec::new();
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
                    let plans = call
                        .machine_arguments
                        .iter()
                        .filter_map(|argument| plan_runtime_operand(program, state, argument))
                        .collect::<Vec<_>>();
                    pending.push((expression, plans));
                }
            }
        }
    }
    let mut calls = Vec::new();
    for (expression, plans) in pending {
        let operands = plans
            .into_iter()
            .map(|plan| realize_operand(program, plan))
            .collect();
        calls.push((expression, operands));
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

pub(super) fn insert_subject_operands(
    arguments: &mut Vec<ExpressionHandle>,
    operands: &[ExpressionHandle],
) {
    arguments.extend(operands.iter().copied());
}

/// The statement call's retained argument vector plus one `Name` expression
/// per subject, reinserted as a handle span.
pub(super) fn insert_arguments_with_subjects(
    program: &mut TypedTrees,
    arguments_span: HandleSpan<ExpressionHandle>,
    subjects: &[ExpressionHandle],
) -> HandleSpan<ExpressionHandle> {
    let mut arguments = program
        .statement_table
        .expression_handles(arguments_span)
        .to_vec();
    insert_subject_operands(&mut arguments, subjects);
    program.statement_table.insert_expression_handles(arguments)
}

pub(super) fn rewrite_selected_call_with_name(
    program: &mut TypedTrees,
    site: CallSite,
    target: SymbolHandle,
    target_name: typed_trees::name::Identifier,
    subjects: &[ExpressionHandle],
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
            stamp_field_subjects(program, authored_machine_application, subjects);
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
            insert_subject_operands(&mut arguments, subjects);
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
            stamp_field_subjects(program, authored_machine_application, subjects);
        }
    }
}

/// A field subject has no symbol. Record the member expression on the binding
/// so index substitution sees the same read the call argument evaluates.
fn stamp_field_subjects(
    program: &mut TypedTrees,
    application: typed_trees::static_applications::StaticMachineApplicationHandle,
    operands: &[ExpressionHandle],
) {
    if !application.is_valid() {
        return;
    }
    let mut bindings = program
        .expression_table
        .static_machine_application_bindings(application)
        .to_vec();
    let mut operands = operands.iter().copied();
    for binding in &mut bindings {
        let typed_trees::static_applications::StaticMachineBindingValue::RuntimeValue {
            subject,
            expression,
            ..
        } = &mut binding.value
        else {
            continue;
        };
        let Some(operand) = operands.next() else {
            break;
        };
        if !subject.is_valid() && !expression.is_valid() {
            *expression = operand;
        }
    }
    program
        .expression_table
        .set_static_machine_application_bindings(application, &bindings);
}
