use crate::expression::lower_expression;
use crate::name::lower_name_path;
use crate::program::Lowerer;
use crate::type_reference::lower_type_reference;
use omega_core::diagnostics::Diagnostic;
use omega_resolved_trees as resolved;
use omega_typed_trees as typed;

pub(crate) fn lower_statement(
    lowerer: &mut Lowerer,
    statement: &resolved::statement::Statement,
) -> Result<typed::statement::Statement, Diagnostic> {
    match statement {
        resolved::statement::Statement::Assignment(assignment) => Ok(
            typed::statement::Statement::Assignment(typed::statement::Assignment {
                target: lower_expression(&assignment.target)?,
                value: lower_expression(&assignment.value)?,
            }),
        ),
        resolved::statement::Statement::Call(call) => {
            let mut arguments = omega_core::arena::HandleSpan::empty();
            for argument in lowerer
                .source_trees
                .state_statement_expressions(call.arguments)
            {
                let argument = lower_expression(argument)?;
                lowerer
                    .typed_trees
                    .push_statement_expression(&mut arguments, argument);
            }

            Ok(typed::statement::Statement::Call(typed::statement::Call {
                receiver_symbol: call.receiver_symbol,
                target_symbol: call.target_symbol,
                receiver: call.receiver.as_ref().map(lower_name_path),
                target: crate::name::lower_name(&call.target),
                arguments,
            }))
        }
        resolved::statement::Statement::Expression(expression) => Ok(
            typed::statement::Statement::Expression(lower_expression(expression)?),
        ),
        resolved::statement::Statement::LocalData(local_data) => Ok(
            typed::statement::Statement::LocalData(typed::statement::LocalData {
                symbol: local_data.symbol,
                name: crate::name::lower_name(&local_data.name),
                type_reference: lower_type_reference(lowerer, &local_data.type_reference)?,
                initial_value: local_data
                    .initial_value
                    .as_ref()
                    .map(lower_expression)
                    .transpose()?,
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
                guard: lower_transition_guard(&transition.guard)?,
            }),
        ),
    }
}

fn lower_transition_guard(
    guard: &resolved::statement::TransitionGuard,
) -> Result<typed::statement::TransitionGuard, Diagnostic> {
    match guard {
        resolved::statement::TransitionGuard::Always => {
            Ok(typed::statement::TransitionGuard::Always)
        }
        resolved::statement::TransitionGuard::When(expression) => Ok(
            typed::statement::TransitionGuard::When(lower_expression(expression)?),
        ),
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
                .state_statement_expressions(named.arguments)
            {
                let argument = lower_expression(argument)?;
                lowerer
                    .typed_trees
                    .push_statement_expression(&mut arguments, argument);
            }

            Ok(typed::statement::TransitionTarget::Named {
                path: lower_name_path(&named.path),
                arguments,
            })
        }
        resolved::statement::TransitionTarget::Value(expression) => Ok(
            typed::statement::TransitionTarget::Value(lower_expression(expression)?),
        ),
        resolved::statement::TransitionTarget::SelfTarget => {
            Ok(typed::statement::TransitionTarget::SelfTarget)
        }
        resolved::statement::TransitionTarget::Terminal => {
            Ok(typed::statement::TransitionTarget::Terminal)
        }
    }
}
