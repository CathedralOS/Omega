use crate::branching::aliases::{BranchParameterBinding, RuntimeBranchAlias};
use omega_control_flow::StateKey;
use omega_core::symbols::SymbolHandle;
use omega_typed_trees::expression::{
    BinaryExpression, Expression, ExpressionHandle, ExpressionNode, ExpressionTable, NamePath,
    TableBinaryExpression, TableIndexedExpression, TableNamePath,
};

pub(crate) fn resolve_branch_expression(
    expression: &Expression,
    branch_bindings: &[BranchParameterBinding],
    expression_table: &ExpressionTable,
) -> Expression {
    match expression {
        Expression::Mutable(target) => {
            let resolved_target =
                resolve_branch_expression(target, branch_bindings, expression_table);
            if matches!(resolved_target, Expression::Mutable(_)) {
                resolved_target
            } else {
                Expression::Mutable(Box::new(resolved_target))
            }
        }
        Expression::Name(path) if !path.is_empty() => branch_bindings
            .iter()
            .find(|binding| branch_binding_matches_path(binding, path))
            .map(|binding| {
                expression_table.to_tree_with_place_suffix(binding.expression, &path[1..])
            })
            .unwrap_or_else(|| expression.clone()),
        Expression::Binary(binary) => Expression::Binary(Box::new(BinaryExpression {
            left: resolve_branch_expression(&binary.left, branch_bindings, expression_table),
            operator: binary.operator,
            right: resolve_branch_expression(&binary.right, branch_bindings, expression_table),
        })),
        _ => expression.clone(),
    }
}

pub(crate) fn resolve_branch_expression_handle(
    expression: ExpressionHandle,
    branch_bindings: &[BranchParameterBinding],
    expression_table: &mut ExpressionTable,
) -> ExpressionHandle {
    match expression_table.expression(expression).clone() {
        ExpressionNode::Mutable(target) => {
            let resolved_target =
                resolve_branch_expression_handle(target, branch_bindings, expression_table);
            if matches!(
                expression_table.expression(resolved_target),
                ExpressionNode::Mutable(_)
            ) {
                resolved_target
            } else {
                expression_table.insert(ExpressionNode::Mutable(resolved_target))
            }
        }
        ExpressionNode::Name(path) if path.members.count() > 0 => branch_bindings
            .iter()
            .find(|binding| branch_binding_matches_table_path(binding, &path))
            .map(|binding| {
                expression_table.insert_copy_with_member_suffix(binding.expression, path.members, 1)
            })
            .unwrap_or(expression),
        ExpressionNode::Binary(binary) => {
            let left =
                resolve_branch_expression_handle(binary.left, branch_bindings, expression_table);
            let right =
                resolve_branch_expression_handle(binary.right, branch_bindings, expression_table);
            expression_table.insert(ExpressionNode::Binary(TableBinaryExpression {
                left,
                operator: binary.operator,
                right,
            }))
        }
        _ => expression,
    }
}

pub(super) fn resolve_runtime_branch_alias_expression_handle(
    expression: ExpressionHandle,
    source_key: StateKey,
    aliases: &[RuntimeBranchAlias],
    expression_table: &mut ExpressionTable,
) -> ExpressionHandle {
    match expression_table.expression(expression).clone() {
        ExpressionNode::Mutable(target) => {
            let resolved_target = resolve_runtime_branch_alias_expression_handle(
                target,
                source_key,
                aliases,
                expression_table,
            );
            if matches!(
                expression_table.expression(resolved_target),
                ExpressionNode::Mutable(_)
            ) {
                resolved_target
            } else {
                expression_table.insert(ExpressionNode::Mutable(resolved_target))
            }
        }
        ExpressionNode::Indexed(TableIndexedExpression { collection, index }) => {
            let collection = resolve_runtime_branch_alias_expression_handle(
                collection,
                source_key,
                aliases,
                expression_table,
            );
            let index = resolve_runtime_branch_alias_expression_handle(
                index,
                source_key,
                aliases,
                expression_table,
            );
            expression_table.insert(ExpressionNode::Indexed(TableIndexedExpression {
                collection,
                index,
            }))
        }
        ExpressionNode::Name(path) if path.members.count() > 0 => aliases
            .iter()
            .rev()
            .find(|alias| alias.source_key == source_key && alias_matches_table_path(alias, &path))
            .map(|alias| {
                expression_table.insert_copy_with_member_suffix(alias.expression, path.members, 1)
            })
            .unwrap_or(expression),
        _ => expression,
    }
}

fn branch_binding_matches_path(binding: &BranchParameterBinding, path: &NamePath) -> bool {
    symbol_matches_path(binding.parameter_symbol, path)
}

fn branch_binding_matches_table_path(
    binding: &BranchParameterBinding,
    path: &TableNamePath,
) -> bool {
    binding.parameter_symbol.is_valid()
        && path.head_symbol.is_valid()
        && binding.parameter_symbol == path.head_symbol
}

fn alias_matches_table_path(alias: &RuntimeBranchAlias, path: &TableNamePath) -> bool {
    alias.parameter_symbol.is_valid()
        && path.head_symbol.is_valid()
        && alias.parameter_symbol == path.head_symbol
}

fn symbol_matches_path(symbol: SymbolHandle, path: &NamePath) -> bool {
    symbol.is_valid() && path.head_symbol().is_valid() && symbol == path.head_symbol()
}
