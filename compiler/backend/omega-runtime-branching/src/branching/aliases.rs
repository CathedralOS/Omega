use crate::RuntimeBranchingContext;
use omega_state_calls::{StateCall, StateCallArgumentKind};
use omega_typed_program::expression::Expression;
use omega_typed_program::statement::TransitionGuard;

mod expressions;
mod model;

pub(super) use expressions::resolve_branch_expression;
use expressions::resolve_runtime_branch_alias_expression;
pub(super) use model::{BranchParameterBinding, RuntimeBranchAlias};

pub(super) fn resolve_branch_guard(
    guard: &TransitionGuard,
    branch_bindings: &[BranchParameterBinding],
) -> TransitionGuard {
    match guard {
        TransitionGuard::Always => TransitionGuard::Always,
        TransitionGuard::When(expression) => {
            TransitionGuard::When(resolve_branch_expression(expression, branch_bindings))
        }
    }
}

pub(super) fn branch_parameter_bindings(
    context: &RuntimeBranchingContext,
    state_call: &StateCall,
    aliases: &[RuntimeBranchAlias],
) -> Vec<BranchParameterBinding> {
    context
        .state_calls
        .arguments
        .span(state_call.arguments)
        .map(|arguments| {
            arguments
                .iter()
                .map(|argument| {
                    let argument_expression =
                        context.state_calls.expressions.to_tree(argument.expression);
                    let expression = if argument.kind == StateCallArgumentKind::MutableAlias
                        && !matches!(argument_expression, Expression::Mutable(_))
                    {
                        Expression::Mutable(Box::new(argument_expression))
                    } else {
                        argument_expression
                    };
                    BranchParameterBinding {
                        parameter_symbol: argument.parameter_symbol,
                        parameter_name: argument.parameter_name.clone(),
                        expression: resolve_runtime_branch_alias_expression(
                            &expression,
                            state_call.source_key,
                            aliases,
                        ),
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn bind_runtime_branch_aliases(
    context: &RuntimeBranchingContext,
    aliases: &mut Vec<RuntimeBranchAlias>,
    state_call: &StateCall,
) {
    let Some(arguments) = context.state_calls.arguments.span(state_call.arguments) else {
        return;
    };

    for argument in arguments {
        let argument_expression = context.state_calls.expressions.to_tree(argument.expression);
        let expression = if argument.kind == StateCallArgumentKind::MutableAlias
            && !matches!(argument_expression, Expression::Mutable(_))
        {
            Expression::Mutable(Box::new(argument_expression))
        } else {
            argument_expression
        };
        set_runtime_branch_alias(
            aliases,
            RuntimeBranchAlias {
                source_key: state_call.target_key,
                parameter_symbol: argument.parameter_symbol,
                parameter_name: argument.parameter_name.clone(),
                expression: resolve_runtime_branch_alias_expression(
                    &expression,
                    state_call.source_key,
                    aliases,
                ),
            },
        );
    }
}

fn set_runtime_branch_alias(aliases: &mut Vec<RuntimeBranchAlias>, alias: RuntimeBranchAlias) {
    if let Some(existing_alias) = aliases.iter_mut().find(|existing_alias| {
        existing_alias.source_key == alias.source_key
            && existing_alias.parameter_symbol == alias.parameter_symbol
    }) {
        *existing_alias = alias;
    } else {
        aliases.push(alias);
    }
}
