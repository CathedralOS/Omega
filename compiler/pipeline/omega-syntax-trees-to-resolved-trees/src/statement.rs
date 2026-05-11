use crate::expression::lower_expression;
use crate::name::lower_name_path;
use crate::program::Lowerer;
use crate::type_reference::lower_type_reference;
use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolHandle;
use omega_syntax_trees as syntax;
use omega_resolved_trees::statement::{
    Assignment, Call, LocalData, Statement, Transition, TransitionGuard, TransitionTarget,
};

pub(crate) fn lower_statement(
    lowerer: &mut Lowerer,
    statement: &syntax::statement::Statement,
) -> Result<Statement, Diagnostic> {
    match statement {
        syntax::statement::Statement::Assignment(assignment) => Ok(Statement::Assignment(
            Assignment {
                target: lower_expression(&assignment.target)?,
                value: lower_expression(&assignment.value)?,
            },
        )),
        syntax::statement::Statement::Call(call) => Ok(Statement::Call(Call {
            receiver_symbol: SymbolHandle::invalid(),
            target_symbol: SymbolHandle::invalid(),
            receiver: call.receiver.as_ref().map(lower_name_path),
            target: crate::name::lower_name(&call.target),
            arguments: call
                .arguments
                .iter()
                .map(lower_expression)
                .collect::<Result<Vec<_>, _>>()?,
        })),
        syntax::statement::Statement::Expression(expression) => {
            Ok(Statement::Expression(lower_expression(expression)?))
        }
        syntax::statement::Statement::LocalData(local_data) => Ok(Statement::LocalData(
            LocalData {
                symbol: SymbolHandle::invalid(),
                name: crate::name::lower_name(&local_data.name),
                type_reference: lower_type_reference(lowerer, &local_data.type_reference)?,
                initial_value: local_data
                    .initial_value
                    .as_ref()
                    .map(lower_expression)
                    .transpose()?,
            },
        )),
        syntax::statement::Statement::Transition(transition) => Ok(Statement::Transition(
            Transition {
                target: lower_transition_target(&transition.target)?,
                continuation: transition
                    .continuation
                    .as_ref()
                    .map(lower_transition_target)
                    .transpose()?,
                guard: lower_transition_guard(&transition.guard)?,
            },
        )),
    }
}

fn lower_transition_guard(
    guard: &syntax::statement::TransitionGuard,
) -> Result<TransitionGuard, Diagnostic> {
    match guard {
        syntax::statement::TransitionGuard::Always => Ok(TransitionGuard::Always),
        syntax::statement::TransitionGuard::When(expression) => {
            Ok(TransitionGuard::When(lower_expression(expression)?))
        }
    }
}

fn lower_transition_target(
    target: &syntax::statement::TransitionTarget,
) -> Result<TransitionTarget, Diagnostic> {
    match target {
        syntax::statement::TransitionTarget::Named { path, arguments } => {
            Ok(TransitionTarget::Named {
                path: lower_name_path(path),
                arguments: arguments
                    .iter()
                    .map(lower_expression)
                    .collect::<Result<Vec<_>, _>>()?,
            })
        }
        syntax::statement::TransitionTarget::Value(expression) => {
            Ok(TransitionTarget::Value(lower_expression(expression)?))
        }
        syntax::statement::TransitionTarget::SelfTarget => Ok(TransitionTarget::SelfTarget),
        syntax::statement::TransitionTarget::Terminal => Ok(TransitionTarget::Terminal),
    }
}
