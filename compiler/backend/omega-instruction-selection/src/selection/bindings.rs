use omega_checked_trees::expression::{
    Expression, ExpressionHandle, ExpressionNode, ExpressionTable, NamePath,
    TableIndexedExpression, TableNamePath,
};
use omega_checked_trees::name::ProgramName;
use omega_control_flow::StateKey;
use omega_core::arena::{Arena, HandleSpan};
use omega_core::symbols::SymbolHandle;

use super::storage_places::indexed_expression_path;
use omega_runtime_branching::{
    RuntimeBranchPreludeBinding, RuntimeLeafBranchBinding, RuntimeLeafBranchBindingKind,
    RuntimeStraightLineBranchBinding, RuntimeStraightLineBranchBindingKind,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct RuntimeAliasBinding {
    pub(super) source_key: StateKey,
    pub(super) parameter_symbol: SymbolHandle,
    pub(super) parameter_name: ProgramName,
    pub(super) expression_source_key: StateKey,
    pub(super) expression: ExpressionHandle,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct RuntimeAliasBuffer {
    aliases: Arena<RuntimeAliasBinding>,
    scope: HandleSpan<RuntimeAliasBinding>,
}

impl RuntimeAliasBuffer {
    pub(super) fn clear(&mut self) {
        self.aliases.reset_retain_capacity();
        self.scope = HandleSpan::empty();
    }

    pub(super) fn from_iter(bindings: impl IntoIterator<Item = RuntimeAliasBinding>) -> Self {
        let mut buffer = Self::default();
        buffer.scope = buffer.aliases.insert_many(bindings);
        buffer
    }

    pub(super) fn from_bindings(bindings: &[RuntimeAliasBinding]) -> Self {
        Self::from_iter(bindings.iter().cloned())
    }

    pub(super) fn bindings(&self) -> &[RuntimeAliasBinding] {
        self.aliases.span_or_empty(self.scope)
    }

    pub(super) fn set_alias(&mut self, alias: RuntimeAliasBinding) {
        let existing_handle = self
            .aliases
            .iter()
            .find(|(_, existing_alias)| {
                self.bindings().iter().any(|binding| {
                    binding.source_key == existing_alias.source_key
                        && binding.parameter_symbol == existing_alias.parameter_symbol
                }) && existing_alias.source_key == alias.source_key
                    && existing_alias.parameter_symbol == alias.parameter_symbol
            })
            .map(|(handle, _)| handle);

        if let Some(handle) = existing_handle {
            *self.aliases.get_mut(handle) = alias;
            return;
        }

        self.aliases.append_to_span(&mut self.scope, alias);
    }
}

#[derive(Clone, Copy)]
pub(super) struct RuntimeAliasResolutionContext<'alias, 'expr> {
    pub(super) aliases: &'alias [RuntimeAliasBinding],
    pub(super) alias_expressions: &'expr ExpressionTable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeResolvedExpression {
    pub(super) source_key: StateKey,
    pub(super) expression: Expression,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RuntimeResolvedExpressionHandle {
    pub(super) source_key: StateKey,
    pub(super) expression: ExpressionHandle,
}

pub(super) fn strip_mutable_expression(expression: Expression) -> Expression {
    match expression {
        Expression::Mutable(target) => *target,
        _ => expression,
    }
}

pub(super) fn strip_mutable_expression_handle(
    table: &ExpressionTable,
    expression: ExpressionHandle,
) -> ExpressionHandle {
    match table.expression(expression) {
        ExpressionNode::Mutable(target) => *target,
        _ => expression,
    }
}

pub(super) fn resolve_runtime_alias_expression(
    expression: &Expression,
    source_key: StateKey,
    aliases: &[RuntimeAliasBinding],
    alias_expressions: &ExpressionTable,
) -> Expression {
    resolve_runtime_alias_binding(expression, source_key, aliases, alias_expressions).expression
}

pub(super) fn resolve_runtime_alias_binding(
    expression: &Expression,
    source_key: StateKey,
    aliases: &[RuntimeAliasBinding],
    alias_expressions: &ExpressionTable,
) -> RuntimeResolvedExpression {
    match expression {
        Expression::ArrayLiteral(values) => {
            let mut resolved_source_key = source_key;
            let values = values
                .iter()
                .map(|value| {
                    let resolved = resolve_runtime_alias_binding(
                        value,
                        source_key,
                        aliases,
                        alias_expressions,
                    );
                    resolved_source_key = resolved.source_key;
                    resolved.expression
                })
                .collect();
            RuntimeResolvedExpression {
                source_key: resolved_source_key,
                expression: Expression::ArrayLiteral(values),
            }
        }
        Expression::Binary(binary) => {
            let left =
                resolve_runtime_alias_binding(&binary.left, source_key, aliases, alias_expressions);
            let right = resolve_runtime_alias_binding(
                &binary.right,
                source_key,
                aliases,
                alias_expressions,
            );
            RuntimeResolvedExpression {
                source_key: left.source_key,
                expression: Expression::Binary(Box::new(
                    omega_checked_trees::expression::BinaryExpression {
                        left: left.expression,
                        operator: binary.operator,
                        right: right.expression,
                    },
                )),
            }
        }
        Expression::Cast(cast) => {
            let resolved =
                resolve_runtime_alias_binding(&cast.value, source_key, aliases, alias_expressions);
            RuntimeResolvedExpression {
                source_key: resolved.source_key,
                expression: Expression::Cast(Box::new(
                    omega_checked_trees::expression::CastExpression {
                        value: resolved.expression,
                        target_type: cast.target_type.clone(),
                    },
                )),
            }
        }
        Expression::Call(call) => {
            let receiver = call.receiver.as_ref().map(|receiver| {
                resolve_runtime_alias_binding(receiver, source_key, aliases, alias_expressions)
            });
            let mut resolved_source_key = receiver
                .as_ref()
                .map(|resolved| resolved.source_key)
                .unwrap_or(source_key);
            let arguments = call
                .arguments
                .iter()
                .map(|argument| {
                    let resolved = resolve_runtime_alias_binding(
                        argument,
                        source_key,
                        aliases,
                        alias_expressions,
                    );
                    resolved_source_key = resolved.source_key;
                    resolved.expression
                })
                .collect();
            RuntimeResolvedExpression {
                source_key: resolved_source_key,
                expression: Expression::Call(Box::new(
                    omega_checked_trees::expression::CallExpression {
                        receiver: receiver.map(|resolved| Box::new(resolved.expression)),
                        target_symbol: call.target_symbol,
                        target: call.target.clone(),
                        arguments,
                    },
                )),
            }
        }
        Expression::Mutable(target) => {
            let resolved =
                resolve_runtime_alias_binding(target, source_key, aliases, alias_expressions);
            RuntimeResolvedExpression {
                source_key: resolved.source_key,
                expression: Expression::Mutable(Box::new(resolved.expression)),
            }
        }
        Expression::Indexed(indexed) => {
            let collection = resolve_runtime_alias_binding(
                &indexed.collection,
                source_key,
                aliases,
                alias_expressions,
            );
            let index = resolve_runtime_alias_binding(
                &indexed.index,
                source_key,
                aliases,
                alias_expressions,
            );
            RuntimeResolvedExpression {
                source_key: collection.source_key,
                expression: Expression::Indexed(Box::new(
                    omega_checked_trees::expression::IndexedExpression {
                        collection: collection.expression,
                        index: index.expression,
                    },
                )),
            }
        }
        Expression::Member(member) => {
            let receiver = resolve_runtime_alias_binding(
                &member.receiver,
                source_key,
                aliases,
                alias_expressions,
            );
            RuntimeResolvedExpression {
                source_key: receiver.source_key,
                expression: Expression::Member(Box::new(
                    omega_checked_trees::expression::MemberExpression {
                        receiver: receiver.expression,
                        member_symbol: member.member_symbol,
                        member: member.member.clone(),
                    },
                )),
            }
        }
        Expression::StructLiteral(struct_literal) => {
            let mut resolved_source_key = source_key;
            RuntimeResolvedExpression {
                source_key: resolved_source_key,
                expression: Expression::StructLiteral(
                    omega_checked_trees::expression::StructLiteral {
                        type_name: struct_literal.type_name.clone(),
                        fields: struct_literal
                            .fields
                            .iter()
                            .map(|field| {
                                let resolved = resolve_runtime_alias_binding(
                                    &field.value,
                                    source_key,
                                    aliases,
                                    alias_expressions,
                                );
                                resolved_source_key = resolved.source_key;
                                omega_checked_trees::expression::StructLiteralField {
                                    name: field.name.clone(),
                                    value: resolved.expression,
                                }
                            })
                            .collect(),
                    },
                ),
            }
        }
        Expression::Name(path) if !path.is_empty() => aliases
            .iter()
            .rev()
            .find(|alias| alias.source_key == source_key && alias_matches_path(alias, path))
            .map(|alias| {
                let expression =
                    alias_expressions.to_tree_with_place_suffix(alias.expression, &path[1..]);
                resolve_runtime_alias_binding(
                    &expression,
                    alias.expression_source_key,
                    aliases,
                    alias_expressions,
                )
            })
            .unwrap_or_else(|| RuntimeResolvedExpression {
                source_key,
                expression: expression.clone(),
            }),
        _ => RuntimeResolvedExpression {
            source_key,
            expression: expression.clone(),
        },
    }
}

pub(super) fn resolve_runtime_alias_binding_handle(
    expression: ExpressionHandle,
    source_key: StateKey,
    aliases: &[RuntimeAliasBinding],
    alias_expressions: &mut ExpressionTable,
) -> RuntimeResolvedExpressionHandle {
    match alias_expressions.expression(expression).clone() {
        ExpressionNode::ArrayLiteral(values) => {
            let mut resolved_source_key = source_key;
            let copied_values = alias_expressions.reserve_expression_handles(values.count());
            for offset in 0..values.count() {
                let value = alias_expressions.expression_handle_at_offset(values, offset);
                let resolved = resolve_runtime_alias_binding_handle(
                    value,
                    source_key,
                    aliases,
                    alias_expressions,
                );
                resolved_source_key = resolved.source_key;
                alias_expressions.set_expression_handle_at_offset(
                    copied_values,
                    offset,
                    resolved.expression,
                );
            }
            RuntimeResolvedExpressionHandle {
                source_key: resolved_source_key,
                expression: alias_expressions.insert(ExpressionNode::ArrayLiteral(copied_values)),
            }
        }
        ExpressionNode::Binary(binary) => {
            let left = resolve_runtime_alias_binding_handle(
                binary.left,
                source_key,
                aliases,
                alias_expressions,
            );
            let right = resolve_runtime_alias_binding_handle(
                binary.right,
                source_key,
                aliases,
                alias_expressions,
            );
            RuntimeResolvedExpressionHandle {
                source_key: left.source_key,
                expression: alias_expressions.insert(ExpressionNode::Binary(
                    omega_checked_trees::expression::TableBinaryExpression {
                        left: left.expression,
                        operator: binary.operator,
                        right: right.expression,
                    },
                )),
            }
        }
        ExpressionNode::Cast(cast) => {
            let resolved = resolve_runtime_alias_binding_handle(
                cast.value,
                source_key,
                aliases,
                alias_expressions,
            );
            RuntimeResolvedExpressionHandle {
                source_key: resolved.source_key,
                expression: alias_expressions.insert(ExpressionNode::Cast(
                    omega_checked_trees::expression::TableCastExpression {
                        value: resolved.expression,
                        target_type: cast.target_type,
                    },
                )),
            }
        }
        ExpressionNode::Call(call) => {
            let receiver = call.receiver.is_valid().then(|| {
                resolve_runtime_alias_binding_handle(
                    call.receiver,
                    source_key,
                    aliases,
                    alias_expressions,
                )
            });
            let mut resolved_source_key = receiver
                .as_ref()
                .map(|resolved| resolved.source_key)
                .unwrap_or(source_key);
            let copied_arguments =
                alias_expressions.reserve_expression_handles(call.arguments.count());
            for offset in 0..call.arguments.count() {
                let argument =
                    alias_expressions.expression_handle_at_offset(call.arguments, offset);
                let resolved = resolve_runtime_alias_binding_handle(
                    argument,
                    source_key,
                    aliases,
                    alias_expressions,
                );
                resolved_source_key = resolved.source_key;
                alias_expressions.set_expression_handle_at_offset(
                    copied_arguments,
                    offset,
                    resolved.expression,
                );
            }
            RuntimeResolvedExpressionHandle {
                source_key: resolved_source_key,
                expression: alias_expressions.insert(ExpressionNode::Call(
                    omega_checked_trees::expression::TableCallExpression {
                        receiver: receiver
                            .map(|resolved| resolved.expression)
                            .unwrap_or_else(ExpressionHandle::invalid),
                        target_symbol: call.target_symbol,
                        target: call.target.clone(),
                        arguments: copied_arguments,
                    },
                )),
            }
        }
        ExpressionNode::Mutable(target) => {
            let resolved = resolve_runtime_alias_binding_handle(
                target,
                source_key,
                aliases,
                alias_expressions,
            );
            RuntimeResolvedExpressionHandle {
                source_key: resolved.source_key,
                expression: alias_expressions.insert(ExpressionNode::Mutable(resolved.expression)),
            }
        }
        ExpressionNode::Indexed(TableIndexedExpression { collection, index }) => {
            let collection = resolve_runtime_alias_binding_handle(
                collection,
                source_key,
                aliases,
                alias_expressions,
            );
            let index =
                resolve_runtime_alias_binding_handle(index, source_key, aliases, alias_expressions);
            RuntimeResolvedExpressionHandle {
                source_key: collection.source_key,
                expression: alias_expressions.insert(ExpressionNode::Indexed(
                    TableIndexedExpression {
                        collection: collection.expression,
                        index: index.expression,
                    },
                )),
            }
        }
        ExpressionNode::Member(member) => {
            let receiver = resolve_runtime_alias_binding_handle(
                member.receiver,
                source_key,
                aliases,
                alias_expressions,
            );
            RuntimeResolvedExpressionHandle {
                source_key: receiver.source_key,
                expression: alias_expressions.insert(ExpressionNode::Member(
                    omega_checked_trees::expression::TableMemberExpression {
                        receiver: receiver.expression,
                        member_symbol: member.member_symbol,
                        member: member.member.clone(),
                    },
                )),
            }
        }
        ExpressionNode::StructLiteral(struct_literal) => {
            let mut resolved_source_key = source_key;
            let copied_fields =
                alias_expressions.reserve_struct_fields(struct_literal.fields.count());
            for offset in 0..struct_literal.fields.count() {
                let field = alias_expressions
                    .struct_field_at_offset(struct_literal.fields, offset)
                    .clone();
                let resolved = resolve_runtime_alias_binding_handle(
                    field.value,
                    source_key,
                    aliases,
                    alias_expressions,
                );
                resolved_source_key = resolved.source_key;
                alias_expressions.set_struct_field_at_offset(
                    copied_fields,
                    offset,
                    omega_checked_trees::expression::TableStructLiteralField {
                        name: field.name,
                        value: resolved.expression,
                    },
                );
            }
            RuntimeResolvedExpressionHandle {
                source_key: resolved_source_key,
                expression: alias_expressions.insert(ExpressionNode::StructLiteral(
                    omega_checked_trees::expression::TableStructLiteral {
                        type_name: struct_literal.type_name.clone(),
                        fields: copied_fields,
                    },
                )),
            }
        }
        ExpressionNode::Name(path) if path.members.count() > 0 => aliases
            .iter()
            .rev()
            .find(|alias| {
                alias.source_key == source_key
                    && alias_matches_table_path(alias, alias_expressions, &path)
            })
            .map(|alias| {
                let resolved = resolve_runtime_alias_binding_handle(
                    alias.expression,
                    alias.expression_source_key,
                    aliases,
                    alias_expressions,
                );
                RuntimeResolvedExpressionHandle {
                    source_key: resolved.source_key,
                    expression: alias_expressions.insert_copy_with_member_suffix(
                        resolved.expression,
                        path.members,
                        1,
                    ),
                }
            })
            .unwrap_or(RuntimeResolvedExpressionHandle {
                source_key,
                expression,
            }),
        _ => RuntimeResolvedExpressionHandle {
            source_key,
            expression,
        },
    }
}

pub(super) fn resolve_leaf_binding_expression(
    table: &ExpressionTable,
    expression: &Expression,
    bindings: &[RuntimeLeafBranchBinding],
) -> Expression {
    match expression {
        Expression::Mutable(target) => {
            let resolved_target = resolve_leaf_binding_expression(table, target, bindings);
            if matches!(resolved_target, Expression::Mutable(_)) {
                resolved_target
            } else {
                Expression::Mutable(Box::new(resolved_target))
            }
        }
        Expression::Name(path) if !path.is_empty() => bindings
            .iter()
            .find(|binding| {
                leaf_binding_matches_path(binding, path)
                    && binding.kind == RuntimeLeafBranchBindingKind::LeafParameter
            })
            .or_else(|| {
                bindings
                    .iter()
                    .find(|binding| leaf_binding_matches_path(binding, path))
            })
            .map(|binding| table.to_tree_with_place_suffix(binding.expression, &path[1..]))
            .unwrap_or_else(|| expression.clone()),
        _ => expression.clone(),
    }
}

pub(super) fn resolve_straight_line_binding_expression(
    table: &ExpressionTable,
    expression: &Expression,
    bindings: &[RuntimeStraightLineBranchBinding],
) -> Expression {
    match expression {
        Expression::Mutable(target) => {
            let resolved_target = resolve_straight_line_binding_expression(table, target, bindings);
            if matches!(resolved_target, Expression::Mutable(_)) {
                resolved_target
            } else {
                Expression::Mutable(Box::new(resolved_target))
            }
        }
        Expression::Name(path) if !path.is_empty() => bindings
            .iter()
            .find(|binding| {
                straight_line_binding_matches_path(binding, path)
                    && binding.kind == RuntimeStraightLineBranchBindingKind::TargetParameter
            })
            .or_else(|| {
                bindings
                    .iter()
                    .find(|binding| straight_line_binding_matches_path(binding, path))
            })
            .map(|binding| table.to_tree_with_place_suffix(binding.expression, &path[1..]))
            .unwrap_or_else(|| expression.clone()),
        _ => expression.clone(),
    }
}

pub(super) fn resolve_branch_prelude_binding_expression(
    table: &ExpressionTable,
    expression: &Expression,
    bindings: &[RuntimeBranchPreludeBinding],
) -> Expression {
    match expression {
        Expression::Mutable(target) => {
            let resolved_target =
                resolve_branch_prelude_binding_expression(table, target, bindings);
            if matches!(resolved_target, Expression::Mutable(_)) {
                resolved_target
            } else {
                Expression::Mutable(Box::new(resolved_target))
            }
        }
        Expression::Name(path) if !path.is_empty() => bindings
            .iter()
            .find(|binding| symbol_matches_path(binding.parameter_symbol, path))
            .map(|binding| table.to_tree_with_place_suffix(binding.expression, &path[1..]))
            .unwrap_or_else(|| expression.clone()),
        _ => expression.clone(),
    }
}

fn alias_matches_path(alias: &RuntimeAliasBinding, path: &NamePath) -> bool {
    if symbol_matches_path(alias.parameter_symbol, path) {
        return true;
    }

    path.first()
        .is_some_and(|root_name| root_name.as_str() == alias.parameter_name.as_str())
}

fn alias_matches_table_path(
    alias: &RuntimeAliasBinding,
    table: &ExpressionTable,
    path: &TableNamePath,
) -> bool {
    if alias.parameter_symbol.is_valid()
        && path.head_symbol.is_valid()
        && alias.parameter_symbol == path.head_symbol
    {
        return true;
    }

    table
        .name_path_members(path.members)
        .first()
        .is_some_and(|root_name| root_name.as_str() == alias.parameter_name.as_str())
}

fn leaf_binding_matches_path(binding: &RuntimeLeafBranchBinding, path: &NamePath) -> bool {
    symbol_matches_path(binding.parameter_symbol, path)
}

fn straight_line_binding_matches_path(
    binding: &RuntimeStraightLineBranchBinding,
    path: &NamePath,
) -> bool {
    symbol_matches_path(binding.parameter_symbol, path)
}

fn symbol_matches_path(symbol: SymbolHandle, path: &NamePath) -> bool {
    symbol.is_valid() && path.head_symbol().is_valid() && symbol == path.head_symbol()
}

pub(super) fn append_place_suffix(expression: &Expression, suffix: &[ProgramName]) -> Expression {
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
