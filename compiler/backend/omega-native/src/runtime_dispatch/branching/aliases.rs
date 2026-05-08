use crate::control_flow::StateKey;
use crate::plan::NativePlan;
use crate::state_calls::{StateCall, StateCallArgumentKind};
use omega_typed_program::expression::Expression;
use omega_typed_program::name::ProgramName;
use omega_typed_program::statement::TransitionGuard;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(super) struct RuntimeBranchAlias {
    pub(super) source_key: StateKey,
    pub(super) machine: ProgramName,
    pub(super) state: ProgramName,
    pub(super) parameter_name: ProgramName,
    pub(super) expression: Expression,
}

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

pub(super) fn resolve_branch_expression(
    expression: &Expression,
    branch_bindings: &[(ProgramName, Expression)],
) -> Expression {
    match expression {
        Expression::Mutable(target) => {
            let resolved_target = resolve_branch_expression(target, branch_bindings);
            if matches!(resolved_target, Expression::Mutable(_)) {
                resolved_target
            } else {
                Expression::Mutable(Box::new(resolved_target))
            }
        }
        Expression::Name(path) if !path.is_empty() => branch_bindings
            .iter()
            .find(|(parameter_name, _)| parameter_name == &path[0])
            .map(|(_, bound_expression)| append_place_suffix(bound_expression, &path[1..]))
            .unwrap_or_else(|| expression.clone()),
        Expression::Binary(binary) => Expression::Binary(Box::new(
            omega_typed_program::expression::BinaryExpression {
                left: resolve_branch_expression(&binary.left, branch_bindings),
                operator: binary.operator,
                right: resolve_branch_expression(&binary.right, branch_bindings),
            },
        )),
        _ => expression.clone(),
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

fn resolve_runtime_branch_alias_expression(
    expression: &Expression,
    source_key: StateKey,
    aliases: &[RuntimeBranchAlias],
) -> Expression {
    match expression {
        Expression::Mutable(target) => {
            let resolved_target =
                resolve_runtime_branch_alias_expression(target, source_key, aliases);
            if matches!(resolved_target, Expression::Mutable(_)) {
                resolved_target
            } else {
                Expression::Mutable(Box::new(resolved_target))
            }
        }
        Expression::Indexed(indexed) => Expression::Indexed(Box::new(
            omega_typed_program::expression::IndexedExpression {
                collection: resolve_runtime_branch_alias_expression(
                    &indexed.collection,
                    source_key,
                    aliases,
                ),
                index: resolve_runtime_branch_alias_expression(&indexed.index, source_key, aliases),
            },
        )),
        Expression::Name(path) if !path.is_empty() => aliases
            .iter()
            .rev()
            .find(|alias| alias.source_key == source_key && alias.parameter_name == path[0])
            .map(|alias| append_place_suffix(&alias.expression, &path[1..]))
            .unwrap_or_else(|| expression.clone()),
        _ => expression.clone(),
    }
}

fn append_place_suffix(expression: &Expression, suffix: &[ProgramName]) -> Expression {
    if suffix.is_empty() {
        return expression.clone();
    }

    match expression {
        Expression::Name(path) => {
            let mut resolved_path = path.clone();
            resolved_path.extend_from_slice(suffix);
            Expression::Name(resolved_path)
        }
        Expression::Indexed(indexed) => {
            if let Some(mut indexed_path) = indexed_expression_path(indexed) {
                indexed_path.extend_from_slice(suffix);
                Expression::Name(indexed_path)
            } else {
                expression.clone()
            }
        }
        Expression::Mutable(target) => {
            Expression::Mutable(Box::new(append_place_suffix(target, suffix)))
        }
        _ => expression.clone(),
    }
}

fn indexed_expression_path(
    indexed: &omega_typed_program::expression::IndexedExpression,
) -> Option<Vec<ProgramName>> {
    let Expression::Integer(index) = &indexed.index else {
        return None;
    };
    let mut path = match &indexed.collection {
        Expression::Name(path) => path.clone(),
        Expression::Indexed(inner_indexed) => indexed_expression_path(inner_indexed)?,
        _ => return None,
    };
    let last_segment = path.last_mut()?;
    *last_segment = ProgramName::generated(format!("{last_segment}[{index}]"));
    Some(path)
}
