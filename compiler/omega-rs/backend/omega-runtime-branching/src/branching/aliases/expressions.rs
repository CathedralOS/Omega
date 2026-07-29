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
        ExpressionNode::Cast(cast) => {
            let value =
                resolve_branch_expression_handle(cast.value, branch_bindings, expression_table);
            expression_table.insert(ExpressionNode::Cast(
                omega_checked_trees::expression::TableCastExpression {
                    value,
                    target_type: cast.target_type,
                    target_label: cast.target_label,
                    domain: cast.domain,
                    semantic_domain: cast.semantic_domain,
                    semantic_domain_symbol: cast.semantic_domain_symbol,
                    qualification_satisfier: cast.qualification_satisfier,
                    form: cast.form,
                },
            ))
        }
        ExpressionNode::Call(call) => {
            let receiver = if call.receiver.is_valid() {
                resolve_branch_expression_handle(call.receiver, branch_bindings, expression_table)
            } else {
                call.receiver
            };
            let arguments = expression_table.reserve_expression_handles(call.arguments.count());
            for offset in 0..call.arguments.count() {
                let argument = expression_table.expression_handle_at_offset(call.arguments, offset);
                let resolved =
                    resolve_branch_expression_handle(argument, branch_bindings, expression_table);
                expression_table.set_expression_handle_at_offset(arguments, offset, resolved);
            }
            expression_table.insert(ExpressionNode::Call(
                omega_checked_trees::expression::TableCallExpression {
                    receiver,
                    target_symbol: call.target_symbol,
                    target: call.target,
                    machine_arguments: call.machine_arguments,
                    arguments,
                    operational_acknowledgement: call.operational_acknowledgement,
                },
            ))
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
        ExpressionNode::Binary(binary) => {
            let left = resolve_runtime_branch_alias_expression_handle(
                binary.left,
                source_key,
                aliases,
                expression_table,
            );
            let right = resolve_runtime_branch_alias_expression_handle(
                binary.right,
                source_key,
                aliases,
                expression_table,
            );
            expression_table.insert(ExpressionNode::Binary(TableBinaryExpression {
                left,
                operator: binary.operator,
                right,
            }))
        }
        ExpressionNode::Cast(cast) => {
            let value = resolve_runtime_branch_alias_expression_handle(
                cast.value,
                source_key,
                aliases,
                expression_table,
            );
            expression_table.insert(ExpressionNode::Cast(
                omega_checked_trees::expression::TableCastExpression {
                    value,
                    target_type: cast.target_type,
                    target_label: cast.target_label,
                    domain: cast.domain,
                    semantic_domain: cast.semantic_domain,
                    semantic_domain_symbol: cast.semantic_domain_symbol,
                    qualification_satisfier: cast.qualification_satisfier,
                    form: cast.form,
                },
            ))
        }
        ExpressionNode::Call(call) => {
            let receiver = if call.receiver.is_valid() {
                resolve_runtime_branch_alias_expression_handle(
                    call.receiver,
                    source_key,
                    aliases,
                    expression_table,
                )
            } else {
                call.receiver
            };
            let arguments = expression_table.reserve_expression_handles(call.arguments.count());
            for offset in 0..call.arguments.count() {
                let argument = expression_table.expression_handle_at_offset(call.arguments, offset);
                let resolved = resolve_runtime_branch_alias_expression_handle(
                    argument,
                    source_key,
                    aliases,
                    expression_table,
                );
                expression_table.set_expression_handle_at_offset(arguments, offset, resolved);
            }
            expression_table.insert(ExpressionNode::Call(
                omega_checked_trees::expression::TableCallExpression {
                    receiver,
                    target_symbol: call.target_symbol,
                    target: call.target,
                    machine_arguments: call.machine_arguments,
                    arguments,
                    operational_acknowledgement: call.operational_acknowledgement,
                },
            ))
        }
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
        ExpressionNode::Member(member) => {
            let receiver = resolve_runtime_branch_alias_expression_handle(
                member.receiver,
                source_key,
                aliases,
                expression_table,
            );
            expression_table.insert(ExpressionNode::Member(TableMemberExpression {
                receiver,
                member_symbol: member.member_symbol,
                member: member.member,
                case_variant: member.case_variant,
            }))
        }
        ExpressionNode::Name(path) => aliases
            .iter()
            .rev()
            .find(|alias| alias.source_key == source_key && alias_matches_table_path(alias, &path))
            .map(|alias| {
                if path.members.count() > 0 {
                    expression_table.insert_copy_with_member_suffix(
                        alias.expression,
                        path.members,
                        path.member_symbols,
                        1,
                    )
                } else {
                    alias.expression
                }
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
