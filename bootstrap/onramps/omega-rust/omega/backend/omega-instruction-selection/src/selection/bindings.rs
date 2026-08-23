use omega_control_flow::StateKey;
use psi_arena::{Arena, Handle, HandleSpan};
use psi_checked_trees::expression::{
    Expression, ExpressionHandle, ExpressionNode, ExpressionTable, NamePath, TableBorrowExpression,
    TableIndexedExpression, TableNamePath,
};
use psi_checked_trees::name::Identifier;
use psi_symbols::SymbolHandle;

use super::storage_places::indexed_expression_path;
use omega_runtime_branching::{
    RuntimeBranchPreludeBinding, RuntimeLeafBranchBinding, RuntimeLeafBranchBindingKind,
    RuntimeStraightLineBranchBinding, RuntimeStraightLineBranchBindingKind,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct RuntimeAliasBinding {
    pub(super) source_key: StateKey,
    pub(super) parameter_symbol: SymbolHandle,
    pub(super) parameter_name: Identifier,
    pub(super) expression_source_key: StateKey,
    pub(super) expression: ExpressionHandle,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct RuntimeAliasBuffer {
    aliases: Arena<RuntimeAliasBinding>,
    scope: HandleSpan<RuntimeAliasBinding>,
}

impl RuntimeAliasBuffer {
    pub(super) fn with_capacity(capacity: usize) -> Self {
        Self {
            aliases: Arena::with_capacity(capacity),
            scope: HandleSpan::empty(),
        }
    }

    pub(super) fn clear(&mut self) {
        self.aliases.reset_retain_capacity();
        self.scope = HandleSpan::empty();
    }

    pub(super) fn from_iter(bindings: impl IntoIterator<Item = RuntimeAliasBinding>) -> Self {
        let bindings = bindings.into_iter();
        let (capacity, _) = bindings.size_hint();
        let mut buffer = Self::with_capacity(capacity);
        buffer.scope = buffer.aliases.insert_many(bindings);
        buffer
    }

    pub(super) fn copy_from_bindings(
        source: &ExpressionTable,
        bindings: &[RuntimeAliasBinding],
        target: &mut ExpressionTable,
    ) -> Self {
        let mut buffer = Self::with_capacity(bindings.len());
        buffer.scope =
            buffer
                .aliases
                .insert_many(bindings.iter().map(|binding| RuntimeAliasBinding {
                    source_key: binding.source_key,
                    parameter_symbol: binding.parameter_symbol,
                    parameter_name: binding.parameter_name.clone(),
                    expression_source_key: binding.expression_source_key,
                    expression: target.copy_from(source, binding.expression),
                }));
        buffer
    }

    pub(super) fn bindings(&self) -> &[RuntimeAliasBinding] {
        self.aliases.span_or_empty(self.scope)
    }

    pub(super) fn set_alias(&mut self, alias: RuntimeAliasBinding) {
        if let Some(handle) = self.alias_handle(alias.source_key, alias.parameter_symbol) {
            *self.aliases.get_mut(handle) = alias;
            return;
        }

        self.aliases.append_to_span(&mut self.scope, alias);
    }

    fn alias_handle(
        &self,
        source_key: StateKey,
        parameter_symbol: SymbolHandle,
    ) -> Option<Handle<RuntimeAliasBinding>> {
        let binding_index = self.bindings().iter().position(|binding| {
            binding.source_key == source_key && binding.parameter_symbol == parameter_symbol
        })?;
        let arena_index = self
            .scope
            .start()
            .arena_index()
            .checked_add(u32::try_from(binding_index).ok()?)?;

        Some(Handle::from_parts(
            arena_index,
            self.scope.start().generation(),
        ))
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
        Expression::Borrow(target) => target.target,
        _ => expression,
    }
}

pub(super) fn strip_mutable_expression_handle(
    table: &ExpressionTable,
    expression: ExpressionHandle,
) -> ExpressionHandle {
    match table.expression(expression) {
        ExpressionNode::Borrow(target) => target.target,
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
                    psi_checked_trees::expression::BinaryExpression {
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
                    psi_checked_trees::expression::CastExpression {
                        value: resolved.expression,
                        target_type: cast.target_type.clone(),
                        target_label: cast.target_label.clone(),
                        domain: cast.domain,
                        form: cast.form,
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
                    psi_checked_trees::expression::CallExpression {
                        receiver: receiver.map(|resolved| Box::new(resolved.expression)),
                        target_symbol: call.target_symbol,
                        target: call.target.clone(),
                        arguments,
                        evidence_arguments: call.evidence_arguments.clone(),
                        operational_acknowledgement: call.operational_acknowledgement,
                    },
                )),
            }
        }
        Expression::Borrow(borrow) => {
            let resolved = resolve_runtime_alias_binding(
                &borrow.target,
                source_key,
                aliases,
                alias_expressions,
            );
            RuntimeResolvedExpression {
                source_key: resolved.source_key,
                expression: Expression::Borrow(Box::new(
                    psi_checked_trees::expression::BorrowExpression {
                        target: resolved.expression,
                        access: borrow.access,
                    },
                )),
            }
        }
        Expression::Indexed(indexed) => {
            // An aggregate-literal alias must NOT substitute into the
            // COLLECTION position (`g[a][b]` -> `[[9, 8], [6, 5]][a][b]`):
            // a literal has no place to index -- the indexed resolvers need
            // the local's SLOT, which state-storage keeps for exactly these
            // aggregates. The SELECTION-layer twin of state-values'
            // `simplify_collection_expression` guard (the third fold layer).
            let collection = if alias_for_path(&indexed.collection, source_key, aliases)
                .is_some_and(|alias| {
                    matches!(
                        alias_expressions.expression(alias.expression),
                        ExpressionNode::ArrayLiteral(_) | ExpressionNode::StructLiteral(_)
                    )
                }) {
                RuntimeResolvedExpression {
                    source_key,
                    expression: indexed.collection.clone(),
                }
            } else {
                resolve_runtime_alias_binding(
                    &indexed.collection,
                    source_key,
                    aliases,
                    alias_expressions,
                )
            };
            let index = resolve_runtime_alias_binding(
                &indexed.index,
                source_key,
                aliases,
                alias_expressions,
            );
            RuntimeResolvedExpression {
                source_key: collection.source_key,
                expression: Expression::Indexed(Box::new(
                    psi_checked_trees::expression::IndexedExpression {
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
                    psi_checked_trees::expression::MemberExpression {
                        receiver: receiver.expression,
                        member_symbol: member.member_symbol,
                        member: member.member.clone(),
                        case_variant: member.case_variant.clone(),
                    },
                )),
            }
        }
        Expression::StructLiteral(struct_literal) => {
            let mut resolved_source_key = source_key;
            RuntimeResolvedExpression {
                source_key: resolved_source_key,
                expression: Expression::StructLiteral(
                    psi_checked_trees::expression::StructLiteral {
                        type_name: struct_literal.type_name.clone(),
                        case_name: struct_literal.case_name.clone(),
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
                                psi_checked_trees::expression::StructLiteralField {
                                    name: field.name.clone(),
                                    value: resolved.expression,
                                }
                            })
                            .collect::<std::sync::Arc<[_]>>(),
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

/// The alias binding a bare `Name` expression would substitute with, if any
/// (`None` for non-Name expressions or unmatched names).
fn alias_for_path<'aliases>(
    expression: &Expression,
    source_key: StateKey,
    aliases: &'aliases [RuntimeAliasBinding],
) -> Option<&'aliases RuntimeAliasBinding> {
    let Expression::Name(path) = expression else {
        return None;
    };
    if path.is_empty() {
        return None;
    }
    aliases
        .iter()
        .rev()
        .find(|alias| alias.source_key == source_key && alias_matches_path(alias, path))
}

pub(super) fn resolve_runtime_alias_binding_handle(
    expression: ExpressionHandle,
    source_key: StateKey,
    aliases: &[RuntimeAliasBinding],
    alias_expressions: &mut ExpressionTable,
) -> RuntimeResolvedExpressionHandle {
    if aliases.is_empty() {
        return RuntimeResolvedExpressionHandle {
            source_key,
            expression,
        };
    }
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
                expression: insert_rebuilt_expression(
                    alias_expressions,
                    expression,
                    ExpressionNode::ArrayLiteral(copied_values),
                ),
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
                expression: insert_rebuilt_expression(
                    alias_expressions,
                    expression,
                    ExpressionNode::Binary(psi_checked_trees::expression::TableBinaryExpression {
                        left: left.expression,
                        operator: binary.operator,
                        right: right.expression,
                    }),
                ),
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
                expression: insert_rebuilt_expression(
                    alias_expressions,
                    expression,
                    ExpressionNode::Cast(psi_checked_trees::expression::TableCastExpression {
                        value: resolved.expression,
                        target_type: cast.target_type,
                        target_label: cast.target_label,
                        domain: cast.domain,
                        semantic_domain: cast.semantic_domain,
                        semantic_domain_arguments: cast.semantic_domain_arguments,
                        semantic_domain_symbol: cast.semantic_domain_symbol,
                        semantic_domain_id: cast.semantic_domain_id,
                        form: cast.form,
                    }),
                ),
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
                expression: insert_rebuilt_expression(
                    alias_expressions,
                    expression,
                    ExpressionNode::Call(psi_checked_trees::expression::TableCallExpression {
                        receiver: receiver
                            .map(|resolved| resolved.expression)
                            .unwrap_or_else(ExpressionHandle::invalid),
                        target_symbol: call.target_symbol,
                        target: call.target.clone(),
                        machine_arguments: call.machine_arguments.clone(),
                        quotient_operation: call.quotient_operation.clone(),
                        arguments: copied_arguments,
                        evidence_arguments: call.evidence_arguments.clone(),
                        operational_acknowledgement: call.operational_acknowledgement,
                    }),
                ),
            }
        }
        ExpressionNode::Borrow(target) => {
            let resolved = resolve_runtime_alias_binding_handle(
                target.target,
                source_key,
                aliases,
                alias_expressions,
            );
            RuntimeResolvedExpressionHandle {
                source_key: resolved.source_key,
                expression: insert_rebuilt_expression(
                    alias_expressions,
                    expression,
                    ExpressionNode::Borrow(TableBorrowExpression {
                        target: resolved.expression,
                        access: target.access,
                    }),
                ),
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
                expression: insert_rebuilt_expression(
                    alias_expressions,
                    expression,
                    ExpressionNode::Indexed(TableIndexedExpression {
                        collection: collection.expression,
                        index: index.expression,
                    }),
                ),
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
                expression: insert_rebuilt_expression(
                    alias_expressions,
                    expression,
                    ExpressionNode::Member(psi_checked_trees::expression::TableMemberExpression {
                        receiver: receiver.expression,
                        member_symbol: member.member_symbol,
                        member: member.member.clone(),
                        case_variant: member.case_variant.clone(),
                    }),
                ),
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
                    psi_checked_trees::expression::TableStructLiteralField {
                        name: field.name,
                        value: resolved.expression,
                    },
                );
            }
            RuntimeResolvedExpressionHandle {
                source_key: resolved_source_key,
                expression: insert_rebuilt_expression(
                    alias_expressions,
                    expression,
                    ExpressionNode::StructLiteral(
                        psi_checked_trees::expression::TableStructLiteral {
                            type_name: struct_literal.type_name.clone(),
                            case_name: struct_literal.case_name.clone(),
                            fields: copied_fields,
                        },
                    ),
                ),
            }
        }
        ExpressionNode::Name(path) => aliases
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
                    expression: if path.members.count() > 1 {
                        let suffixed = alias_expressions.insert_copy_with_member_suffix(
                            resolved.expression,
                            path.members,
                            path.member_symbols,
                            1,
                        );
                        alias_expressions
                            .set_source_span(suffixed, alias_expressions.source_span(expression));
                        suffixed
                    } else {
                        // A bare name reuses the alias expression itself. Its
                        // authored span belongs to the replacement and must not
                        // be overwritten with the use-site name span.
                        resolved.expression
                    },
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

fn insert_rebuilt_expression(
    table: &mut ExpressionTable,
    original: ExpressionHandle,
    expression: ExpressionNode,
) -> ExpressionHandle {
    let source_span = table.source_span(original);
    let rebuilt = table.insert(expression);
    table.set_source_span(rebuilt, source_span);
    rebuilt
}

/// Cycle guard for the binding-substitution walks below. A SELF-REFERENTIAL
/// binding set -- a CYCLIC callee reached through a value-call TERMINAL
/// (`machine cos { transition { _ -> (sin(..)) } }` where sin's `reduce`
/// self-loops, binding `x` to an expression over the same `x`) -- has no
/// finite substitution, and the unbounded Name re-resolve overflowed the
/// compile thread's stack (2026-07-11; the shape now hoists at the frontend
/// and runs -- pass/calls/runtime_value_call_terminal_exit -- so this cap is
/// the backstop for binding cycles reached some other way). Legitimate
/// chains are bounded by the callee's binding count (single digits), so only
/// a true cycle reaches the cap; at the cap the name stays UNSUBSTITUTED,
/// which downstream either resolves as a real place or refuses at the loud
/// unlowered-terminal fences -- never a silent misdelivery, never a crash.
/// Only Name-substitution re-entries count; structural descent does not.
const MAX_BINDING_SUBSTITUTION_DEPTH: usize = 32;

pub(super) fn resolve_leaf_binding_expression_handle(
    source_table: &ExpressionTable,
    table: &mut ExpressionTable,
    expression: ExpressionHandle,
    bindings: &[RuntimeLeafBranchBinding],
) -> ExpressionHandle {
    // Preserve the copied tree's exact authored spans when there is no
    // substitution to perform. Rebuilding a no-op tree would synthesize fresh
    // nodes without those spans, severing checked operator/provider evidence
    // from later instruction selection.
    if bindings.is_empty() {
        return expression;
    }
    resolve_leaf_binding_expression_handle_at_depth(source_table, table, expression, bindings, 0)
}

fn resolve_leaf_binding_expression_handle_at_depth(
    source_table: &ExpressionTable,
    table: &mut ExpressionTable,
    expression: ExpressionHandle,
    bindings: &[RuntimeLeafBranchBinding],
    substitution_depth: usize,
) -> ExpressionHandle {
    match table.expression(expression).clone() {
        ExpressionNode::ArrayLiteral(values) => {
            let copied_values = table.reserve_expression_handles(values.count());
            for offset in 0..values.count() {
                let value = table.expression_handle_at_offset(values, offset);
                let resolved = resolve_leaf_binding_expression_handle_at_depth(
                    source_table,
                    table,
                    value,
                    bindings,
                    substitution_depth,
                );
                table.set_expression_handle_at_offset(copied_values, offset, resolved);
            }
            table.insert(ExpressionNode::ArrayLiteral(copied_values))
        }
        ExpressionNode::Binary(binary) => {
            let left = resolve_leaf_binding_expression_handle_at_depth(
                source_table,
                table,
                binary.left,
                bindings,
                substitution_depth,
            );
            let right = resolve_leaf_binding_expression_handle_at_depth(
                source_table,
                table,
                binary.right,
                bindings,
                substitution_depth,
            );
            table.insert(ExpressionNode::Binary(
                psi_checked_trees::expression::TableBinaryExpression {
                    left,
                    operator: binary.operator,
                    right,
                },
            ))
        }
        ExpressionNode::Cast(cast) => {
            let value = resolve_leaf_binding_expression_handle_at_depth(
                source_table,
                table,
                cast.value,
                bindings,
                substitution_depth,
            );
            table.insert(ExpressionNode::Cast(
                psi_checked_trees::expression::TableCastExpression {
                    value,
                    target_type: cast.target_type,
                    target_label: cast.target_label,
                    domain: cast.domain,
                    semantic_domain: cast.semantic_domain,
                    semantic_domain_arguments: cast.semantic_domain_arguments,
                    semantic_domain_symbol: cast.semantic_domain_symbol,
                    semantic_domain_id: cast.semantic_domain_id,
                    form: cast.form,
                },
            ))
        }
        ExpressionNode::Call(call) => {
            let receiver = call.receiver.is_valid().then(|| {
                resolve_leaf_binding_expression_handle_at_depth(
                    source_table,
                    table,
                    call.receiver,
                    bindings,
                    substitution_depth,
                )
            });
            let copied_arguments = table.reserve_expression_handles(call.arguments.count());
            for offset in 0..call.arguments.count() {
                let argument = table.expression_handle_at_offset(call.arguments, offset);
                let resolved = resolve_leaf_binding_expression_handle_at_depth(
                    source_table,
                    table,
                    argument,
                    bindings,
                    substitution_depth,
                );
                table.set_expression_handle_at_offset(copied_arguments, offset, resolved);
            }
            table.insert(ExpressionNode::Call(
                psi_checked_trees::expression::TableCallExpression {
                    receiver: receiver.unwrap_or_else(ExpressionHandle::invalid),
                    target_symbol: call.target_symbol,
                    target: call.target.clone(),
                    machine_arguments: call.machine_arguments.clone(),
                    quotient_operation: call.quotient_operation.clone(),
                    arguments: copied_arguments,
                    evidence_arguments: call.evidence_arguments.clone(),
                    operational_acknowledgement: call.operational_acknowledgement,
                },
            ))
        }
        ExpressionNode::Indexed(indexed) => {
            let collection = resolve_leaf_binding_expression_handle_at_depth(
                source_table,
                table,
                indexed.collection,
                bindings,
                substitution_depth,
            );
            let index = resolve_leaf_binding_expression_handle_at_depth(
                source_table,
                table,
                indexed.index,
                bindings,
                substitution_depth,
            );
            // An index is always a VALUE, never a pointer. An inlined by-value
            // argument binds as `mut <expr>` (e.g. `items[key]` with `key = mut 2`
            // becomes `items[mut 2]`), which the index-path resolvers reject because
            // they expect a bare integer/place. The `mut` wrapper is meaningless on
            // an index, so strip it here.
            let index = match table.expression(index) {
                ExpressionNode::Borrow(inner) => inner.target,
                _ => index,
            };
            table.insert(ExpressionNode::Indexed(TableIndexedExpression {
                collection,
                index,
            }))
        }
        ExpressionNode::Member(member) => {
            let receiver = resolve_leaf_binding_expression_handle_at_depth(
                source_table,
                table,
                member.receiver,
                bindings,
                substitution_depth,
            );
            table.insert(ExpressionNode::Member(
                psi_checked_trees::expression::TableMemberExpression {
                    receiver,
                    member_symbol: member.member_symbol,
                    member: member.member.clone(),
                    case_variant: member.case_variant.clone(),
                },
            ))
        }
        ExpressionNode::Borrow(target) => {
            let resolved_target = resolve_leaf_binding_expression_handle_at_depth(
                source_table,
                table,
                target.target,
                bindings,
                substitution_depth,
            );
            if matches!(table.expression(resolved_target), ExpressionNode::Borrow(_)) {
                resolved_target
            } else {
                table.insert(ExpressionNode::Borrow(TableBorrowExpression {
                    target: resolved_target,
                    access: target.access,
                }))
            }
        }
        ExpressionNode::Name(path) => bindings
            .iter()
            .find(|binding| {
                leaf_binding_matches_table_path(binding, source_table, table, &path)
                    && binding.kind == RuntimeLeafBranchBindingKind::LeafParameter
            })
            .or_else(|| {
                bindings.iter().find(|binding| {
                    leaf_binding_matches_table_path(binding, source_table, table, &path)
                })
            })
            .map(|binding| {
                let expression = table.copy_from(source_table, binding.expression);
                // A bare same-named symbol-less binding expression would
                // re-match this binding by the name fallback and recurse
                // forever; the substitution's only effect is stripping the
                // callee parameter's symbol, so skip the re-resolve. The
                // depth cap catches the INDIRECT cycles this name check
                // cannot (see MAX_BINDING_SUBSTITUTION_DEPTH).
                let resolved = if substitution_depth >= MAX_BINDING_SUBSTITUTION_DEPTH
                    || binding_substitution_is_self_similar_name(
                        source_table,
                        binding.expression,
                        &binding.parameter_name,
                    ) {
                    expression
                } else {
                    resolve_leaf_binding_expression_handle_at_depth(
                        source_table,
                        table,
                        expression,
                        bindings,
                        substitution_depth + 1,
                    )
                };
                if path.members.count() > 0 {
                    table.insert_copy_with_member_suffix(
                        resolved,
                        path.members,
                        path.member_symbols,
                        1,
                    )
                } else {
                    resolved
                }
            })
            .unwrap_or(expression),
        ExpressionNode::StructLiteral(struct_literal) => {
            let copied_fields = table.reserve_struct_fields(struct_literal.fields.count());
            for offset in 0..struct_literal.fields.count() {
                let field = table
                    .struct_field_at_offset(struct_literal.fields, offset)
                    .clone();
                let value = resolve_leaf_binding_expression_handle_at_depth(
                    source_table,
                    table,
                    field.value,
                    bindings,
                    substitution_depth,
                );
                table.set_struct_field_at_offset(
                    copied_fields,
                    offset,
                    psi_checked_trees::expression::TableStructLiteralField {
                        name: field.name,
                        value,
                    },
                );
            }
            table.insert(ExpressionNode::StructLiteral(
                psi_checked_trees::expression::TableStructLiteral {
                    type_name: struct_literal.type_name.clone(),
                    case_name: struct_literal.case_name.clone(),
                    fields: copied_fields,
                },
            ))
        }
        _ => expression,
    }
}

pub(super) fn resolve_straight_line_binding_expression_handle(
    source_table: &ExpressionTable,
    table: &mut ExpressionTable,
    expression: ExpressionHandle,
    bindings: &[RuntimeStraightLineBranchBinding],
) -> ExpressionHandle {
    if bindings.is_empty() {
        return expression;
    }
    resolve_straight_line_binding_expression_handle_at_depth(
        source_table,
        table,
        expression,
        bindings,
        0,
    )
}

fn resolve_straight_line_binding_expression_handle_at_depth(
    source_table: &ExpressionTable,
    table: &mut ExpressionTable,
    expression: ExpressionHandle,
    bindings: &[RuntimeStraightLineBranchBinding],
    substitution_depth: usize,
) -> ExpressionHandle {
    match table.expression(expression).clone() {
        ExpressionNode::ArrayLiteral(values) => {
            let copied_values = table.reserve_expression_handles(values.count());
            for offset in 0..values.count() {
                let value = table.expression_handle_at_offset(values, offset);
                let resolved = resolve_straight_line_binding_expression_handle_at_depth(
                    source_table,
                    table,
                    value,
                    bindings,
                    substitution_depth,
                );
                table.set_expression_handle_at_offset(copied_values, offset, resolved);
            }
            table.insert(ExpressionNode::ArrayLiteral(copied_values))
        }
        ExpressionNode::Binary(binary) => {
            let left = resolve_straight_line_binding_expression_handle_at_depth(
                source_table,
                table,
                binary.left,
                bindings,
                substitution_depth,
            );
            let right = resolve_straight_line_binding_expression_handle_at_depth(
                source_table,
                table,
                binary.right,
                bindings,
                substitution_depth,
            );
            table.insert(ExpressionNode::Binary(
                psi_checked_trees::expression::TableBinaryExpression {
                    left,
                    operator: binary.operator,
                    right,
                },
            ))
        }
        ExpressionNode::Cast(cast) => {
            let value = resolve_straight_line_binding_expression_handle_at_depth(
                source_table,
                table,
                cast.value,
                bindings,
                substitution_depth,
            );
            table.insert(ExpressionNode::Cast(
                psi_checked_trees::expression::TableCastExpression {
                    value,
                    target_type: cast.target_type,
                    target_label: cast.target_label,
                    domain: cast.domain,
                    semantic_domain: cast.semantic_domain,
                    semantic_domain_arguments: cast.semantic_domain_arguments,
                    semantic_domain_symbol: cast.semantic_domain_symbol,
                    semantic_domain_id: cast.semantic_domain_id,
                    form: cast.form,
                },
            ))
        }
        ExpressionNode::Call(call) => {
            let receiver = call.receiver.is_valid().then(|| {
                resolve_straight_line_binding_expression_handle_at_depth(
                    source_table,
                    table,
                    call.receiver,
                    bindings,
                    substitution_depth,
                )
            });
            let copied_arguments = table.reserve_expression_handles(call.arguments.count());
            for offset in 0..call.arguments.count() {
                let argument = table.expression_handle_at_offset(call.arguments, offset);
                let resolved = resolve_straight_line_binding_expression_handle_at_depth(
                    source_table,
                    table,
                    argument,
                    bindings,
                    substitution_depth,
                );
                table.set_expression_handle_at_offset(copied_arguments, offset, resolved);
            }
            table.insert(ExpressionNode::Call(
                psi_checked_trees::expression::TableCallExpression {
                    receiver: receiver.unwrap_or_else(ExpressionHandle::invalid),
                    target_symbol: call.target_symbol,
                    target: call.target.clone(),
                    machine_arguments: call.machine_arguments.clone(),
                    quotient_operation: call.quotient_operation.clone(),
                    arguments: copied_arguments,
                    evidence_arguments: call.evidence_arguments.clone(),
                    operational_acknowledgement: call.operational_acknowledgement,
                },
            ))
        }
        ExpressionNode::Indexed(indexed) => {
            let collection = resolve_straight_line_binding_expression_handle_at_depth(
                source_table,
                table,
                indexed.collection,
                bindings,
                substitution_depth,
            );
            let index = resolve_straight_line_binding_expression_handle_at_depth(
                source_table,
                table,
                indexed.index,
                bindings,
                substitution_depth,
            );
            table.insert(ExpressionNode::Indexed(TableIndexedExpression {
                collection,
                index,
            }))
        }
        ExpressionNode::Member(member) => {
            let receiver = resolve_straight_line_binding_expression_handle_at_depth(
                source_table,
                table,
                member.receiver,
                bindings,
                substitution_depth,
            );
            table.insert(ExpressionNode::Member(
                psi_checked_trees::expression::TableMemberExpression {
                    receiver,
                    member_symbol: member.member_symbol,
                    member: member.member.clone(),
                    case_variant: member.case_variant.clone(),
                },
            ))
        }
        ExpressionNode::Borrow(target) => {
            let resolved_target = resolve_straight_line_binding_expression_handle_at_depth(
                source_table,
                table,
                target.target,
                bindings,
                substitution_depth,
            );
            if matches!(table.expression(resolved_target), ExpressionNode::Borrow(_)) {
                resolved_target
            } else {
                table.insert(ExpressionNode::Borrow(TableBorrowExpression {
                    target: resolved_target,
                    access: target.access,
                }))
            }
        }
        ExpressionNode::Name(path) => bindings
            .iter()
            .find(|binding| {
                straight_line_binding_matches_table_path(binding, source_table, table, &path)
                    && binding.kind == RuntimeStraightLineBranchBindingKind::TargetParameter
            })
            .or_else(|| {
                bindings.iter().find(|binding| {
                    straight_line_binding_matches_table_path(binding, source_table, table, &path)
                })
            })
            .map(|binding| {
                let expression = table.copy_from(source_table, binding.expression);
                // Depth-capped: a cyclic binding set has no finite
                // substitution (see MAX_BINDING_SUBSTITUTION_DEPTH).
                let resolved = if substitution_depth >= MAX_BINDING_SUBSTITUTION_DEPTH
                    || binding_substitution_is_self_similar_name(
                        source_table,
                        binding.expression,
                        &binding.parameter_name,
                    ) {
                    expression
                } else {
                    resolve_straight_line_binding_expression_handle_at_depth(
                        source_table,
                        table,
                        expression,
                        bindings,
                        substitution_depth + 1,
                    )
                };
                if path.members.count() > 0 {
                    table.insert_copy_with_member_suffix(
                        resolved,
                        path.members,
                        path.member_symbols,
                        1,
                    )
                } else {
                    resolved
                }
            })
            .unwrap_or(expression),
        ExpressionNode::StructLiteral(struct_literal) => {
            let copied_fields = table.reserve_struct_fields(struct_literal.fields.count());
            for offset in 0..struct_literal.fields.count() {
                let field = table
                    .struct_field_at_offset(struct_literal.fields, offset)
                    .clone();
                let value = resolve_straight_line_binding_expression_handle_at_depth(
                    source_table,
                    table,
                    field.value,
                    bindings,
                    substitution_depth,
                );
                table.set_struct_field_at_offset(
                    copied_fields,
                    offset,
                    psi_checked_trees::expression::TableStructLiteralField {
                        name: field.name,
                        value,
                    },
                );
            }
            table.insert(ExpressionNode::StructLiteral(
                psi_checked_trees::expression::TableStructLiteral {
                    type_name: struct_literal.type_name.clone(),
                    case_name: struct_literal.case_name.clone(),
                    fields: copied_fields,
                },
            ))
        }
        _ => expression,
    }
}

pub(super) fn resolve_branch_prelude_binding_expression_handle(
    source_table: &ExpressionTable,
    table: &mut ExpressionTable,
    expression: ExpressionHandle,
    bindings: &[RuntimeBranchPreludeBinding],
) -> ExpressionHandle {
    if bindings.is_empty() {
        return expression;
    }
    resolve_branch_prelude_binding_expression_handle_at_depth(
        source_table,
        table,
        expression,
        bindings,
        0,
    )
}

fn resolve_branch_prelude_binding_expression_handle_at_depth(
    source_table: &ExpressionTable,
    table: &mut ExpressionTable,
    expression: ExpressionHandle,
    bindings: &[RuntimeBranchPreludeBinding],
    substitution_depth: usize,
) -> ExpressionHandle {
    match table.expression(expression).clone() {
        ExpressionNode::Borrow(target) => {
            let resolved_target = resolve_branch_prelude_binding_expression_handle_at_depth(
                source_table,
                table,
                target.target,
                bindings,
                substitution_depth,
            );
            if matches!(table.expression(resolved_target), ExpressionNode::Borrow(_)) {
                resolved_target
            } else {
                table.insert(ExpressionNode::Borrow(TableBorrowExpression {
                    target: resolved_target,
                    access: target.access,
                }))
            }
        }
        ExpressionNode::Name(path) => bindings
            .iter()
            .find(|binding| symbol_matches_table_path(binding.parameter_symbol, &path))
            .map(|binding| {
                if std::env::var_os("OMEGA_DEBUG_RECEIVER").is_some() {
                    eprintln!(
                        "PRELUDE SUBST: path head_sym {} sym {} matched binding `{}` (sym {})",
                        path.head_symbol.arena_index(),
                        path.symbol.arena_index(),
                        binding.parameter_name.as_str(),
                        binding.parameter_symbol.arena_index(),
                    );
                }
                let expression = table.copy_from(source_table, binding.expression);
                // Depth-capped: a cyclic binding set has no finite
                // substitution (see MAX_BINDING_SUBSTITUTION_DEPTH).
                let resolved = if substitution_depth >= MAX_BINDING_SUBSTITUTION_DEPTH {
                    expression
                } else {
                    resolve_branch_prelude_binding_expression_handle_at_depth(
                        source_table,
                        table,
                        expression,
                        bindings,
                        substitution_depth + 1,
                    )
                };
                if path.members.count() > 0 {
                    table.insert_copy_with_member_suffix(
                        resolved,
                        path.members,
                        path.member_symbols,
                        1,
                    )
                } else {
                    resolved
                }
            })
            .unwrap_or(expression),
        _ => expression,
    }
}

fn alias_matches_path(alias: &RuntimeAliasBinding, path: &NamePath) -> bool {
    symbol_matches_path(alias.parameter_symbol, path)
        || path
            .first()
            .is_some_and(|name| *name == alias.parameter_name)
}

fn alias_matches_table_path(
    alias: &RuntimeAliasBinding,
    table: &ExpressionTable,
    path: &TableNamePath,
) -> bool {
    symbol_matches_table_path(alias.parameter_symbol, path)
        || table
            .name_path_members(path.members)
            .first()
            .is_some_and(|name| *name == alias.parameter_name)
}

fn leaf_binding_matches_table_path(
    binding: &RuntimeLeafBranchBinding,
    source_table: &ExpressionTable,
    table: &ExpressionTable,
    path: &TableNamePath,
) -> bool {
    // Match the bound parameter by SYMBOL when the path carries one: a callee
    // parameter and an identically-named caller place (both `out`) are distinct
    // parameters with distinct symbols, and a name-only match would conflate them
    // -- making a binding's own rewrite target (the caller place) re-match the
    // binding and recurse forever. Fall back to the name only for symbol-less paths.
    let matches_parameter = if path.head_symbol.is_valid() || path.symbol.is_valid() {
        symbol_matches_table_path(binding.parameter_symbol, path)
    } else {
        table
            .name_path_members(path.members)
            .first()
            .is_some_and(|name| *name == binding.parameter_name)
    };
    matches_parameter
        && binding_expression_rewrites_leaf_parameter(source_table, binding.expression, binding)
}

fn straight_line_binding_matches_table_path(
    binding: &RuntimeStraightLineBranchBinding,
    source_table: &ExpressionTable,
    table: &ExpressionTable,
    path: &TableNamePath,
) -> bool {
    let matches_parameter = symbol_matches_table_path(binding.parameter_symbol, path)
        || table
            .name_path_members(path.members)
            .first()
            .is_some_and(|name| *name == binding.parameter_name);
    matches_parameter
        && binding_expression_rewrites_straight_line_parameter(
            source_table,
            binding.expression,
            binding,
        )
}

fn binding_expression_rewrites_leaf_parameter(
    table: &ExpressionTable,
    expression: ExpressionHandle,
    binding: &RuntimeLeafBranchBinding,
) -> bool {
    match table.expression(expression) {
        ExpressionNode::Borrow(target) => {
            binding_expression_rewrites_leaf_parameter(table, target.target, binding)
        }
        // A binding rewrites its leaf parameter unless it refers to that SAME
        // parameter (a no-op self-binding). Discriminate by SYMBOL, not name: a
        // callee `out` bound to a caller `out` is a genuine rewrite even though
        // both are named `out` -- they are distinct parameters with distinct
        // symbols. (Across a dispatched-call split the caller arg may not be
        // alias-resolved to a differently-named place, so a name-only check
        // wrongly rejects the binding and drops the arm's write.) A SYMBOL-LESS
        // expression name is CALLER material -- call-argument expressions reach
        // selection through control flow WITHOUT symbols, while the callee's
        // own names carry theirs -- so a same-named caller arg (`work(job)`
        // into `machine work(job: Job)`) is a genuine rewrite too: rejecting it
        // as a self-binding silently dropped the call's result-slot write (the
        // by-value struct/scalar arg-to-free-machine miscompile). Substituting
        // it strips the callee parameter's symbol, letting caller-local
        // initializer substitution match the caller's `let job` by name.
        // Termination: `binding_substitution_is_self_similar_name` stops the
        // post-substitution re-resolve for these bare same-named copies.
        ExpressionNode::Name(path) => {
            if path.head_symbol.is_valid() || path.symbol.is_valid() {
                !symbol_matches_table_path(binding.parameter_symbol, path)
            } else {
                true
            }
        }
        _ => true,
    }
}

/// True when the binding's expression is a bare symbol-less name IDENTICAL to
/// the binding's parameter name (`work(job)` binding callee `job` to caller
/// `job`). Substituting such a binding produces a copy that would re-match the
/// same binding by the name fallback and recurse forever; the caller skips the
/// post-substitution re-resolve for exactly this shape (the substitution's only
/// effect -- intentionally -- is stripping the callee parameter's symbol).
fn binding_substitution_is_self_similar_name(
    table: &ExpressionTable,
    expression: ExpressionHandle,
    parameter_name: &Identifier,
) -> bool {
    match table.expression(expression) {
        ExpressionNode::Borrow(target) => {
            binding_substitution_is_self_similar_name(table, target.target, parameter_name)
        }
        ExpressionNode::Name(path) => {
            !path.head_symbol.is_valid()
                && !path.symbol.is_valid()
                && table
                    .name_path_members(path.members)
                    .first()
                    .is_some_and(|name| name == parameter_name)
        }
        _ => false,
    }
}

fn binding_expression_rewrites_straight_line_parameter(
    table: &ExpressionTable,
    expression: ExpressionHandle,
    binding: &RuntimeStraightLineBranchBinding,
) -> bool {
    match table.expression(expression) {
        ExpressionNode::Borrow(target) => {
            binding_expression_rewrites_straight_line_parameter(table, target.target, binding)
        }
        // As for leaf bindings, a same-named call argument is still a real
        // rewrite when its symbol differs or was erased while copying the
        // caller expression. The latter is the normal shape for a state
        // parameter threaded from an entry local (`seconds_wrapped` ->
        // `seconds_wrapped`). Reject only an exact symbol-preserving
        // self-binding; the self-similar check below stops the symbol-less
        // rewrite from recursively matching by name.
        ExpressionNode::Name(path) => {
            if path.head_symbol.is_valid() || path.symbol.is_valid() {
                !symbol_matches_table_path(binding.parameter_symbol, path)
            } else {
                true
            }
        }
        _ => true,
    }
}

fn symbol_matches_path(symbol: SymbolHandle, path: &NamePath) -> bool {
    symbol.is_valid() && path.head_symbol().is_valid() && symbol == path.head_symbol()
}

fn symbol_matches_table_path(symbol: SymbolHandle, path: &TableNamePath) -> bool {
    symbol.is_valid()
        && ((path.head_symbol.is_valid() && symbol == path.head_symbol)
            || (path.symbol.is_valid() && symbol == path.symbol))
}

pub(super) fn append_place_suffix(expression: &Expression, suffix: &[Identifier]) -> Expression {
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
                append_member_suffix(expression, suffix)
            }
        }
        Expression::Borrow(borrow) => {
            Expression::Borrow(Box::new(psi_checked_trees::expression::BorrowExpression {
                target: append_place_suffix(&borrow.target, suffix),
                access: borrow.access,
            }))
        }
        // Member receivers (and anything else) get a MEMBER chain. The old
        // catch-all returned the expression UNCHANGED -- silently DROPPING the
        // suffix -- so a struct-literal field write built through here targeted
        // the WHOLE receiver: `self.p = Pair8::make(..)`'s decomposed `b` write
        // landed on `self.p` at root size and clobbered the pair with the last
        // field's value. A member over a non-place fails resolution cleanly
        // (None) instead of resolving to the root.
        _ => append_member_suffix(expression, suffix),
    }
}

fn append_member_suffix(expression: &Expression, suffix: &[Identifier]) -> Expression {
    let mut result = expression.clone();
    for member in suffix {
        result = Expression::Member(Box::new(psi_checked_trees::expression::MemberExpression {
            receiver: result,
            member_symbol: SymbolHandle::invalid(),
            member: member.clone(),
            case_variant: None,
        }));
    }
    result
}

#[cfg(test)]
mod source_span_tests {
    use super::*;
    use psi_checked_trees::expression::{BinaryOperator, Expression, NamePath};
    use psi_source::{SourceId, SourceSpan, Span};

    fn span(start: usize, end: usize) -> SourceSpan {
        SourceSpan::new(SourceId(1), Span::new(start, end))
    }

    #[test]
    fn runtime_alias_rebuild_preserves_wrapper_and_replacement_spans() {
        let source_key = StateKey {
            machine: SymbolHandle::from_arena_index(1),
            state: SymbolHandle::from_arena_index(2),
            segment_index: 0,
        };
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
        table.set_source_span(right, span(28, 29));
        let wrapper = table.insert(ExpressionNode::Binary(
            psi_checked_trees::expression::TableBinaryExpression {
                left: use_site,
                operator: BinaryOperator::And,
                right,
            },
        ));
        table.set_source_span(wrapper, span(20, 29));
        let aliases = [RuntimeAliasBinding {
            source_key,
            parameter_symbol,
            parameter_name: "value".into(),
            expression_source_key: source_key,
            expression: replacement,
        }];

        let resolved =
            resolve_runtime_alias_binding_handle(wrapper, source_key, &aliases, &mut table);
        let ExpressionNode::Binary(binary) = table.expression(resolved.expression) else {
            panic!("resolved expression must remain binary");
        };
        assert_eq!(table.source_span(resolved.expression), span(20, 29));
        assert_eq!(table.source_span(binary.left), span(10, 11));
        assert_ne!(table.source_span(binary.left), span(20, 25));
    }
}
