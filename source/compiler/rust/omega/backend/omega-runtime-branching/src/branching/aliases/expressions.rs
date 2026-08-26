use crate::branching::aliases::{
    BranchParameterBinding, BranchParameterBindings, RuntimeBranchAlias, RuntimeBranchAliasBuffer,
};
use omega_control_flow::StateKey;
use psi_checked_trees::expression::{
    ExpressionHandle, ExpressionNode, ExpressionTable, TableBinaryExpression,
    TableIndexedExpression, TableMemberExpression, TableNamePath,
};

pub(crate) fn resolve_branch_expression_handle(
    expression: ExpressionHandle,
    branch_bindings: &BranchParameterBindings,
    expression_table: &mut ExpressionTable,
) -> ExpressionHandle {
    match expression_table.expression(expression).clone() {
        ExpressionNode::Borrow(target) => {
            let resolved_target =
                resolve_branch_expression_handle(target.target, branch_bindings, expression_table);
            if matches!(
                expression_table.expression(resolved_target),
                ExpressionNode::Borrow(_)
            ) {
                resolved_target
            } else {
                insert_rebuilt_expression(
                    expression_table,
                    expression,
                    ExpressionNode::Borrow(psi_checked_trees::expression::TableBorrowExpression {
                        target: resolved_target,
                        access: target.access,
                    }),
                )
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
            insert_rebuilt_expression(
                expression_table,
                expression,
                ExpressionNode::Indexed(TableIndexedExpression { collection, index }),
            )
        }
        ExpressionNode::Member(member) => {
            let receiver = resolve_branch_expression_handle(
                member.receiver,
                branch_bindings,
                expression_table,
            );
            insert_rebuilt_expression(
                expression_table,
                expression,
                ExpressionNode::Member(TableMemberExpression {
                    receiver,
                    member_symbol: member.member_symbol,
                    member: member.member,
                    case_variant: member.case_variant,
                }),
            )
        }
        ExpressionNode::Name(path) => branch_bindings
            .iter()
            .find(|binding| branch_binding_matches_table_path(binding, expression_table, &path))
            .map(|binding| {
                if path.members.count() <= 1 {
                    binding.expression
                } else {
                    let suffixed = expression_table.insert_copy_with_member_suffix(
                        binding.expression,
                        path.members,
                        path.member_symbols,
                        1,
                    );
                    expression_table
                        .set_source_span(suffixed, expression_table.source_span(expression));
                    suffixed
                }
            })
            .unwrap_or(expression),
        ExpressionNode::Binary(binary) => {
            let left =
                resolve_branch_expression_handle(binary.left, branch_bindings, expression_table);
            let right =
                resolve_branch_expression_handle(binary.right, branch_bindings, expression_table);
            insert_rebuilt_expression(
                expression_table,
                expression,
                ExpressionNode::Binary(TableBinaryExpression {
                    left,
                    operator: binary.operator,
                    right,
                }),
            )
        }
        ExpressionNode::Cast(cast) => {
            let value =
                resolve_branch_expression_handle(cast.value, branch_bindings, expression_table);
            insert_rebuilt_expression(
                expression_table,
                expression,
                ExpressionNode::Cast(psi_checked_trees::expression::TableCastExpression {
                    value,
                    target_type: cast.target_type,
                    target_label: cast.target_label,
                    domain: cast.domain,
                    semantic_domain: cast.semantic_domain,
                    semantic_domain_arguments: cast.semantic_domain_arguments,
                    semantic_domain_symbol: cast.semantic_domain_symbol,
                    semantic_domain_id: cast.semantic_domain_id,
                    form: cast.form,
                }),
            )
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
            insert_rebuilt_expression(
                expression_table,
                expression,
                ExpressionNode::Call(psi_checked_trees::expression::TableCallExpression {
                    receiver,
                    target_symbol: call.target_symbol,
                    target: call.target,
                    machine_arguments: call.machine_arguments,
                    quotient_operation: call.quotient_operation,
                    private_layout_operation: call.private_layout_operation,
                    arguments,
                    evidence_arguments: call.evidence_arguments,
                    operational_acknowledgement: call.operational_acknowledgement,
                }),
            )
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
            insert_rebuilt_expression(
                expression_table,
                expression,
                ExpressionNode::Binary(TableBinaryExpression {
                    left,
                    operator: binary.operator,
                    right,
                }),
            )
        }
        ExpressionNode::Cast(cast) => {
            let value = resolve_runtime_branch_alias_expression_handle(
                cast.value,
                source_key,
                aliases,
                expression_table,
            );
            insert_rebuilt_expression(
                expression_table,
                expression,
                ExpressionNode::Cast(psi_checked_trees::expression::TableCastExpression {
                    value,
                    target_type: cast.target_type,
                    target_label: cast.target_label,
                    domain: cast.domain,
                    semantic_domain: cast.semantic_domain,
                    semantic_domain_arguments: cast.semantic_domain_arguments,
                    semantic_domain_symbol: cast.semantic_domain_symbol,
                    semantic_domain_id: cast.semantic_domain_id,
                    form: cast.form,
                }),
            )
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
            insert_rebuilt_expression(
                expression_table,
                expression,
                ExpressionNode::Call(psi_checked_trees::expression::TableCallExpression {
                    receiver,
                    target_symbol: call.target_symbol,
                    target: call.target,
                    machine_arguments: call.machine_arguments,
                    quotient_operation: call.quotient_operation,
                    private_layout_operation: call.private_layout_operation,
                    arguments,
                    evidence_arguments: call.evidence_arguments,
                    operational_acknowledgement: call.operational_acknowledgement,
                }),
            )
        }
        ExpressionNode::Borrow(target) => {
            let resolved_target = resolve_runtime_branch_alias_expression_handle(
                target.target,
                source_key,
                aliases,
                expression_table,
            );
            if matches!(
                expression_table.expression(resolved_target),
                ExpressionNode::Borrow(_)
            ) {
                resolved_target
            } else {
                insert_rebuilt_expression(
                    expression_table,
                    expression,
                    ExpressionNode::Borrow(psi_checked_trees::expression::TableBorrowExpression {
                        target: resolved_target,
                        access: target.access,
                    }),
                )
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
            insert_rebuilt_expression(
                expression_table,
                expression,
                ExpressionNode::Indexed(TableIndexedExpression { collection, index }),
            )
        }
        ExpressionNode::Member(member) => {
            let receiver = resolve_runtime_branch_alias_expression_handle(
                member.receiver,
                source_key,
                aliases,
                expression_table,
            );
            insert_rebuilt_expression(
                expression_table,
                expression,
                ExpressionNode::Member(TableMemberExpression {
                    receiver,
                    member_symbol: member.member_symbol,
                    member: member.member,
                    case_variant: member.case_variant,
                }),
            )
        }
        ExpressionNode::Name(path) => aliases
            .iter()
            .rev()
            .find(|alias| alias.source_key == source_key && alias_matches_table_path(alias, &path))
            .map(|alias| {
                if path.members.count() > 1 {
                    let suffixed = expression_table.insert_copy_with_member_suffix(
                        alias.expression,
                        path.members,
                        path.member_symbols,
                        1,
                    );
                    expression_table
                        .set_source_span(suffixed, expression_table.source_span(expression));
                    suffixed
                } else {
                    // A bare name reuses the alias expression itself. Its
                    // authored span belongs to the replacement and must not
                    // be overwritten with the use-site name span.
                    alias.expression
                }
            })
            .unwrap_or(expression),
        _ => expression,
    }
}

pub(super) fn insert_rebuilt_expression(
    table: &mut ExpressionTable,
    original: ExpressionHandle,
    expression: ExpressionNode,
) -> ExpressionHandle {
    let source_span = table.source_span(original);
    let rebuilt = table.insert(expression);
    table.set_source_span(rebuilt, source_span);
    rebuilt
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
        ExpressionNode::Borrow(target) => {
            binding_expression_rewrites_parameter(table, target.target, binding)
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

#[cfg(test)]
mod tests {
    use super::*;
    use psi_checked_trees::expression::{BinaryOperator, Expression, NamePath};
    use psi_source::{SourceId, SourceSpan, Span};
    use psi_symbols::SymbolHandle;

    fn span(start: usize, end: usize) -> SourceSpan {
        SourceSpan::new(SourceId(1), Span::new(start, end))
    }

    #[test]
    fn branch_binding_rebuild_preserves_wrapper_and_replacement_spans() {
        let parameter_symbol = SymbolHandle::from_arena_index(3);
        let mut table = ExpressionTable::new();
        let replacement = table.insert_tree(&Expression::Boolean(true));
        table.set_source_span(replacement, span(10, 11));
        let use_site = table.insert_tree(&Expression::Name(NamePath::resolved(
            vec!["value".into()],
            parameter_symbol,
            parameter_symbol,
        )));
        table.set_source_span(use_site, span(20, 25));
        let right = table.insert_tree(&Expression::Boolean(false));
        let wrapper = table.insert(ExpressionNode::Binary(TableBinaryExpression {
            left: use_site,
            operator: BinaryOperator::And,
            right,
        }));
        table.set_source_span(wrapper, span(20, 29));
        let mut bindings = BranchParameterBindings::new();
        bindings.push(BranchParameterBinding {
            parameter_symbol,
            parameter_name: "value".into(),
            expression: replacement,
        });

        let resolved = resolve_branch_expression_handle(wrapper, &bindings, &mut table);
        let ExpressionNode::Binary(binary) = table.expression(resolved) else {
            panic!("resolved expression must remain binary");
        };
        assert_eq!(table.source_span(resolved), span(20, 29));
        assert_eq!(table.source_span(binary.left), span(10, 11));
        assert_ne!(table.source_span(binary.left), span(20, 25));
    }
}
