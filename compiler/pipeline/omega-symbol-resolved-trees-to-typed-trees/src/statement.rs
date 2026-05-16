use crate::expression::lower_expression_from_table;
use crate::program::Lowerer;
use crate::type_reference::lower_type_reference;
use omega_core::arena::HandleSpan;
use omega_core::diagnostics::Diagnostic;
use omega_symbol_resolved_trees as resolved;
use omega_typed_trees as typed;

pub(crate) fn lower_statement(
    lowerer: &mut Lowerer,
    statement: &resolved::statement::Statement,
) -> Result<typed::statement::Statement, Diagnostic> {
    match statement {
        resolved::statement::Statement::Assignment(assignment) => Ok(
            typed::statement::Statement::Assignment(typed::statement::Assignment {
                target: lower_statement_expression(lowerer, assignment.target)?,
                value: lower_statement_expression(lowerer, assignment.value)?,
            }),
        ),
        resolved::statement::Statement::Call(call) => {
            let mut arguments = omega_core::arena::HandleSpan::empty();
            for argument in lowerer
                .source_trees
                .tables
                .bodies
                .expressions
                .expression_handles(call.arguments)
            {
                let argument = lower_statement_expression(lowerer, *argument)?;
                lowerer
                    .typed_trees
                    .push_statement_expression(&mut arguments, argument);
            }

            Ok(typed::statement::Statement::Call(typed::statement::Call {
                receiver_symbol: call.receiver_symbol,
                target_symbol: call.target_symbol,
                receiver: lower_statement_path_members(lowerer, call.receiver),
                target: crate::name::lower_name(&call.target),
                arguments,
            }))
        }
        resolved::statement::Statement::Expression(expression) => {
            Ok(typed::statement::Statement::Expression(
                lower_statement_expression(lowerer, *expression)?,
            ))
        }
        resolved::statement::Statement::LocalData(local_data) => Ok(
            typed::statement::Statement::LocalData(typed::statement::LocalData {
                symbol: local_data.symbol,
                name: crate::name::lower_name(&local_data.name),
                type_reference: lower_type_reference(lowerer, &local_data.type_reference)?,
                initial_value: match local_data.initial_value {
                    Some(initial_value) => {
                        Some(lower_statement_expression(lowerer, initial_value)?)
                    }
                    None => None,
                },
            }),
        ),
        resolved::statement::Statement::Transition(transition) => Ok(
            typed::statement::Statement::Transition(typed::statement::Transition {
                target: lower_transition_target(lowerer, &transition.target)?,
                continuation: transition
                    .continuation
                    .as_ref()
                    .map(|target| lower_transition_target(lowerer, target))
                    .transpose()?,
                guard: lower_transition_guard(lowerer, &transition.guard)?,
            }),
        ),
    }
}

fn lower_statement_expression(
    lowerer: &Lowerer,
    expression: resolved::expression::ExpressionHandle,
) -> Result<typed::expression::Expression, Diagnostic> {
    lower_expression_from_table(&lowerer.source_trees.tables.bodies.expressions, expression)
}

fn lower_transition_guard(
    lowerer: &Lowerer,
    guard: &resolved::statement::TransitionGuard,
) -> Result<typed::statement::TransitionGuard, Diagnostic> {
    match guard {
        resolved::statement::TransitionGuard::Always => {
            Ok(typed::statement::TransitionGuard::Always)
        }
        resolved::statement::TransitionGuard::When(expression) => {
            Ok(typed::statement::TransitionGuard::When(
                lower_statement_expression(lowerer, *expression)?,
            ))
        }
    }
}

fn lower_transition_target(
    lowerer: &mut Lowerer,
    target: &resolved::statement::TransitionTarget,
) -> Result<typed::statement::TransitionTarget, Diagnostic> {
    match target {
        resolved::statement::TransitionTarget::Named(named) => {
            let mut arguments = omega_core::arena::HandleSpan::empty();
            for argument in lowerer
                .source_trees
                .tables
                .bodies
                .expressions
                .expression_handles(named.arguments)
            {
                let argument = lower_statement_expression(lowerer, *argument)?;
                lowerer
                    .typed_trees
                    .push_statement_expression(&mut arguments, argument);
            }

            Ok(typed::statement::TransitionTarget::Named {
                path: lower_statement_path_members(lowerer, named.path),
                head_symbol: named.head_symbol,
                symbol: named.symbol,
                arguments,
            })
        }
        resolved::statement::TransitionTarget::Value(expression) => {
            Ok(typed::statement::TransitionTarget::Value(
                lower_statement_expression(lowerer, *expression)?,
            ))
        }
        resolved::statement::TransitionTarget::SelfTarget => {
            Ok(typed::statement::TransitionTarget::SelfTarget)
        }
        resolved::statement::TransitionTarget::Terminal => {
            Ok(typed::statement::TransitionTarget::Terminal)
        }
    }
}

fn lower_statement_path_members(
    lowerer: &mut Lowerer,
    path: HandleSpan<resolved::name::DiagnosticName>,
) -> HandleSpan<typed::name::ProgramName> {
    let mut lowered_path = HandleSpan::empty();

    for member in lowerer
        .source_trees
        .tables
        .declarations
        .statement_path_members
        .span_or_empty(path)
    {
        lowerer
            .typed_trees
            .push_statement_path_member(&mut lowered_path, crate::name::lower_name(member));
    }

    lowered_path
}
