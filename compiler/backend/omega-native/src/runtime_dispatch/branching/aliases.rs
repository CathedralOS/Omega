use crate::plan::NativePlan;
use crate::state_calls::{StateCall, StateCallArgumentKind};
use omega_typed_program::expression::Expression;
use omega_typed_program::name::ProgramName;
use omega_typed_program::statement::TransitionGuard;

mod expressions;
mod model;

pub(super) use expressions::resolve_branch_expression;
use expressions::resolve_runtime_branch_alias_expression;
pub(super) use model::RuntimeBranchAlias;

pub(super) fn resolve_branch_guard(
    guard: &TransitionGuard,
    branch_bindings: &[(ProgramName, Expression)],
) -> TransitionGuard {
    match guard {
        TransitionGuard::Always => TransitionGuard::Always,
        TransitionGuard::When(expression) => {
            TransitionGuard::When(resolve_branch_expression(expression, branch_bindings))
        }
    }
}

pub(super) fn branch_parameter_bindings(
    native_plan: &NativePlan,
    state_call: &StateCall,
    aliases: &[RuntimeBranchAlias],
) -> Vec<(ProgramName, Expression)> {
    native_plan
        .state_calls
        .arguments
        .span(state_call.arguments)
        .map(|arguments| {
            arguments
                .iter()
                .map(|argument| {
                    let expression = if argument.kind == StateCallArgumentKind::MutableAlias
                        && !matches!(argument.expression, Expression::Mutable(_))
                    {
                        Expression::Mutable(Box::new(argument.expression.clone()))
                    } else {
                        argument.expression.clone()
                    };
                    (
                        argument.parameter_name.clone(),
                        resolve_runtime_branch_alias_expression(
                            &expression,
                            state_call.source_key,
                            aliases,
                        ),
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn bind_runtime_branch_aliases(
    native_plan: &NativePlan,
    aliases: &mut Vec<RuntimeBranchAlias>,
    state_call: &StateCall,
) {
    let Some(arguments) = native_plan.state_calls.arguments.span(state_call.arguments) else {
        return;
    };

    for argument in arguments {
        let expression = if argument.kind == StateCallArgumentKind::MutableAlias
            && !matches!(argument.expression, Expression::Mutable(_))
        {
            Expression::Mutable(Box::new(argument.expression.clone()))
        } else {
            argument.expression.clone()
        };
        set_runtime_branch_alias(
            aliases,
            RuntimeBranchAlias {
                source_key: state_call.target_key,
                machine: state_call.target_machine.clone(),
                state: state_call.target_state.clone(),
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
            && existing_alias.parameter_name == alias.parameter_name
    }) {
        *existing_alias = alias;
    } else {
        aliases.push(alias);
    }
}
