use crate::branching::aliases::{
    BranchParameterBinding, BranchParameterBindings, RuntimeBranchAlias, RuntimeBranchAliasBuffer,
};
use omega_checked_trees::expression::{
    ExpressionHandle, ExpressionNode, ExpressionTable, TableBinaryExpression,
    TableIndexedExpression, TableMemberExpression, TableNamePath,
};
use omega_control_flow::StateKey;

pub(crate) fn resolve_branch_expression_handle(
    expression: ExpressionHandle,
    branch_bindings: &BranchParameterBindings,
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
        ExpressionNode::Indexed(indexed) => {
            let collection = resolve_branch_expression_handle(
                indexed.collection,
                branch_bindings,
                expression_table,
            );
            let index =
                resolve_branch_expression_handle(indexed.index, branch_bindings, expression_table);
            expression_table.insert(ExpressionNode::Indexed(TableIndexedExpression {
                collection,
                index,
            }))
        }
        ExpressionNode::Member(member) => {
            let receiver = resolve_branch_expression_handle(
                member.receiver,
                branch_bindings,
                expression_table,
            );
            expression_table.insert(ExpressionNode::Member(TableMemberExpression {
                receiver,
                member_symbol: member.member_symbol,
                member: member.member,
                case_variant: member.case_variant,
            }))
        }
        ExpressionNode::Name(path) => branch_bindings
            .iter()
            .find(|binding| branch_binding_matches_table_path(binding, expression_table, &path))
            .map(|binding| {
                if path.members.count() == 0 {
                    binding.expression
                } else {
                    expression_table.insert_copy_with_member_suffix(
                        binding.expression,
                        path.members,
                        path.member_symbols,
                        1,
                    )
                }
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
    aliases: &RuntimeBranchAliasBuffer,
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
                expression_table.insert_copy_with_member_suffix(
                    alias.expression,
                    path.members,
                    path.member_symbols,
                    1,
                )
            })
            .unwrap_or(expression),
        _ => expression,
    }
}

fn branch_binding_matches_table_path(
    binding: &BranchParameterBinding,
    table: &ExpressionTable,
    path: &TableNamePath,
) -> bool {
    let matches_parameter = (binding.parameter_symbol.is_valid()
        && path.head_symbol.is_valid()
        && binding.parameter_symbol == path.head_symbol)
        || table
            .name_path_members(path.members)
            .first()
            .is_some_and(|name| *name == binding.parameter_name);
    matches_parameter && binding_expression_rewrites_parameter(table, binding.expression, binding)
}

fn binding_expression_rewrites_parameter(
    table: &ExpressionTable,
    expression: ExpressionHandle,
    binding: &BranchParameterBinding,
) -> bool {
    match table.expression(expression) {
        ExpressionNode::Mutable(target) => {
            binding_expression_rewrites_parameter(table, *target, binding)
        }
        ExpressionNode::Name(path) => table
            .name_path_members(path.members)
            .first()
            .is_none_or(|name| *name != binding.parameter_name),
        _ => true,
    }
}

fn alias_matches_table_path(alias: &RuntimeBranchAlias, path: &TableNamePath) -> bool {
    alias.parameter_symbol.is_valid()
        && path.head_symbol.is_valid()
        && alias.parameter_symbol == path.head_symbol
}
