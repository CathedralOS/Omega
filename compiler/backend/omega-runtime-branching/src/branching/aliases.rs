use crate::RuntimeBranchingContext;
use omega_checked_trees::expression::{ExpressionNode, ExpressionTable};
use omega_state_calls::{StateCall, StateCallArgumentKind};

mod expressions;
mod model;

pub(super) use expressions::resolve_branch_expression_handle;
use expressions::resolve_runtime_branch_alias_expression_handle;
pub(super) use model::{BranchParameterBinding, RuntimeBranchAlias};

pub(super) fn resolve_branch_guard_handle(
    guard: omega_checked_trees::expression::ExpressionHandle,
    branch_bindings: &[BranchParameterBinding],
    expression_table: &mut ExpressionTable,
) -> omega_checked_trees::expression::ExpressionHandle {
    if guard.is_valid() {
        resolve_branch_expression_handle(guard, branch_bindings, expression_table)
    } else {
        omega_checked_trees::expression::ExpressionHandle::invalid()
    }
}

pub(super) fn branch_parameter_bindings(
    context: &RuntimeBranchingContext,
    state_call: &StateCall,
    aliases: &[RuntimeBranchAlias],
    expression_table: &mut ExpressionTable,
) -> Vec<BranchParameterBinding> {
    context
        .state_calls
        .arguments
        .span(state_call.arguments)
        .map(|arguments| {
            arguments
                .iter()
                .map(|argument| {
                    let argument_expression = expression_table
                        .copy_from(&context.state_calls.expressions, argument.expression);
                    let expression = if argument.kind == StateCallArgumentKind::MutableAlias
                        && !matches!(
                            expression_table.expression(argument_expression),
                            ExpressionNode::Mutable(_)
                        ) {
                        expression_table.insert(ExpressionNode::Mutable(argument_expression))
                    } else {
                        argument_expression
                    };
                    let expression = resolve_runtime_branch_alias_expression_handle(
                        expression,
                        state_call.source_key,
                        aliases,
                        expression_table,
                    );
                    BranchParameterBinding {
                        parameter_symbol: argument.parameter_symbol,
                        parameter_name: argument.parameter_name.clone(),
                        expression,
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn bind_runtime_branch_aliases(
    context: &RuntimeBranchingContext,
    expression_table: &mut ExpressionTable,
    aliases: &mut Vec<RuntimeBranchAlias>,
    state_call: &StateCall,
) {
    let Some(arguments) = context.state_calls.arguments.span(state_call.arguments) else {
        return;
    };

    for argument in arguments {
        let argument_expression =
            expression_table.copy_from(&context.state_calls.expressions, argument.expression);
        let expression = if argument.kind == StateCallArgumentKind::MutableAlias
            && !matches!(
                expression_table.expression(argument_expression),
                ExpressionNode::Mutable(_)
            ) {
            expression_table.insert(ExpressionNode::Mutable(argument_expression))
        } else {
            argument_expression
        };
        let expression = resolve_runtime_branch_alias_expression_handle(
            expression,
            state_call.source_key,
            aliases,
            expression_table,
        );
        set_runtime_branch_alias(
            aliases,
            RuntimeBranchAlias {
                source_key: state_call.target_key,
                parameter_symbol: argument.parameter_symbol,
                parameter_name: argument.parameter_name.clone(),
                expression,
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
