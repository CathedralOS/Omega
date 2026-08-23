use crate::RuntimeBranchingContext;
use omega_state_calls::{StateCall, StateCallArgumentKind};
use psi_checked_trees::expression::{ExpressionNode, ExpressionTable};
use psi_checked_trees::statement::StatementNode;
use psi_language_core::ReferenceAccess;

mod expressions;
mod model;

pub(super) use expressions::resolve_branch_expression_handle;
use expressions::{insert_rebuilt_expression, resolve_runtime_branch_alias_expression_handle};
pub(super) use model::{
    BranchParameterBinding, BranchParameterBindings, RuntimeBranchAlias, RuntimeBranchAliasBuffer,
};

pub(super) fn resolve_branch_guard_handle(
    guard: psi_checked_trees::expression::ExpressionHandle,
    branch_bindings: &BranchParameterBindings,
    expression_table: &mut ExpressionTable,
) -> psi_checked_trees::expression::ExpressionHandle {
    if guard.is_valid() {
        resolve_branch_expression_handle(guard, branch_bindings, expression_table)
    } else {
        psi_checked_trees::expression::ExpressionHandle::invalid()
    }
}

pub(super) fn branch_parameter_bindings(
    context: &RuntimeBranchingContext,
    state_call: &StateCall,
    aliases: &RuntimeBranchAliasBuffer,
    expression_table: &mut ExpressionTable,
) -> BranchParameterBindings {
    let mut bindings = BranchParameterBindings::with_capacity(state_call.arguments.len());

    if let Some(arguments) = context.state_calls.arguments.span(state_call.arguments) {
        for argument in arguments {
            let argument_expression =
                expression_table.copy_from(&context.state_calls.expressions, argument.expression);
            let expression = if argument.kind == StateCallArgumentKind::MutableAlias
                && !matches!(
                    expression_table.expression(argument_expression),
                    ExpressionNode::Borrow(_)
                ) {
                insert_rebuilt_expression(
                    expression_table,
                    argument_expression,
                    ExpressionNode::Borrow(psi_checked_trees::expression::TableBorrowExpression {
                        target: argument_expression,
                        access: ReferenceAccess::Mutable,
                    }),
                )
            } else {
                argument_expression
            };
            let expression = resolve_elided_source_local_expression_handle(
                context,
                state_call.source_key,
                state_call.statement_index,
                expression,
                expression_table,
            );
            let expression = resolve_runtime_branch_alias_expression_handle(
                expression,
                state_call.source_key,
                aliases,
                expression_table,
            );
            bindings.push(BranchParameterBinding {
                parameter_symbol: argument.parameter_symbol,
                parameter_name: argument.parameter_name.clone(),
                expression,
            });
        }
    }

    bindings
}

fn resolve_elided_source_local_expression_handle(
    context: &RuntimeBranchingContext<'_>,
    source_key: omega_control_flow::StateKey,
    statement_bound: usize,
    expression: psi_checked_trees::expression::ExpressionHandle,
    expressions: &mut ExpressionTable,
) -> psi_checked_trees::expression::ExpressionHandle {
    match expressions.expression(expression).clone() {
        ExpressionNode::Binary(binary) => {
            let left = resolve_elided_source_local_expression_handle(
                context,
                source_key,
                statement_bound,
                binary.left,
                expressions,
            );
            let right = resolve_elided_source_local_expression_handle(
                context,
                source_key,
                statement_bound,
                binary.right,
                expressions,
            );
            insert_rebuilt_expression(
                expressions,
                expression,
                ExpressionNode::Binary(psi_checked_trees::expression::TableBinaryExpression {
                    left,
                    operator: binary.operator,
                    right,
                }),
            )
        }
        ExpressionNode::Cast(cast) => {
            let value = resolve_elided_source_local_expression_handle(
                context,
                source_key,
                statement_bound,
                cast.value,
                expressions,
            );
            insert_rebuilt_expression(
                expressions,
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
        ExpressionNode::Borrow(target) => {
            let target = resolve_elided_source_local_expression_handle(
                context,
                source_key,
                statement_bound,
                target.target,
                expressions,
            );
            if matches!(expressions.expression(target), ExpressionNode::Borrow(_)) {
                target
            } else {
                insert_rebuilt_expression(
                    expressions,
                    expression,
                    ExpressionNode::Borrow(psi_checked_trees::expression::TableBorrowExpression {
                        target,
                        access: ReferenceAccess::Mutable,
                    }),
                )
            }
        }
        ExpressionNode::Indexed(indexed) => {
            let collection = resolve_elided_source_local_expression_handle(
                context,
                source_key,
                statement_bound,
                indexed.collection,
                expressions,
            );
            let index = resolve_elided_source_local_expression_handle(
                context,
                source_key,
                statement_bound,
                indexed.index,
                expressions,
            );
            insert_rebuilt_expression(
                expressions,
                expression,
                ExpressionNode::Indexed(psi_checked_trees::expression::TableIndexedExpression {
                    collection,
                    index,
                }),
            )
        }
        ExpressionNode::Member(member) => {
            let receiver = resolve_elided_source_local_expression_handle(
                context,
                source_key,
                statement_bound,
                member.receiver,
                expressions,
            );
            insert_rebuilt_expression(
                expressions,
                expression,
                ExpressionNode::Member(psi_checked_trees::expression::TableMemberExpression {
                    receiver,
                    member_symbol: member.member_symbol,
                    member: member.member,
                    case_variant: member.case_variant,
                }),
            )
        }
        ExpressionNode::Name(path) if path.members.count() == 1 => {
            let Some(machine) = context
                .program
                .machines()
                .iter()
                .find(|machine| machine.symbol == source_key.machine)
            else {
                return expression;
            };
            let Some(state) = context
                .program
                .machine_states(machine)
                .iter()
                .find(|state| state.symbol == source_key.state)
            else {
                return expression;
            };
            let mut matched = None;
            for (index, statement) in context
                .program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .enumerate()
                .take(statement_bound)
            {
                let StatementNode::LocalData(local) = statement else {
                    continue;
                };
                let name_matches = expressions
                    .name_path_members(path.members)
                    .first()
                    .is_some_and(|name| *name == local.name);
                let symbol_matches = (path.head_symbol.is_valid()
                    && path.head_symbol == local.symbol)
                    || (path.symbol.is_valid() && path.symbol == local.symbol);
                if (symbol_matches
                    || (!path.head_symbol.is_valid() && !path.symbol.is_valid() && name_matches))
                    && local.initial_value.is_valid()
                {
                    matched = Some((index, local));
                }
            }
            let Some((local_index, local)) = matched else {
                return expression;
            };
            // Aggregate locals have their own field/result materialization
            // routes. Substituting a struct or sum initializer here bypasses
            // those routes and loses by-value argument/result fields.
            if context
                .program
                .primitive_type_reference(local.type_reference)
                .is_none()
            {
                return expression;
            }
            // A bare value-call local may be absent from StateStorage while
            // still owning a RuntimeStorage call-result slot. Re-expanding its
            // initializer would execute or read the call in the wrong branch
            // context. Compiler builtins such as the min/max tree behind
            // `clamp` do not enter StateCallPlan, so genuinely elided scalar
            // builtin locals remain substitutable.
            if context
                .state_calls
                .assignment_value_call(source_key, local_index)
                .is_some()
            {
                return expression;
            }
            // Locals represented by the state-storage plan have a runtime
            // identity. Only substitute locals elided from that plan: these are
            // fold-only aliases whose name otherwise cannot resolve in a
            // flattened nested call.
            if context.state_storage.locals.iter().any(|(_, stored)| {
                stored.source_key == source_key
                    && stored.statement_index == local_index
                    && stored.symbol == local.symbol
            }) {
                return expression;
            }
            let initializer =
                expressions.copy_from(&context.program.expression_table, local.initial_value);
            resolve_elided_source_local_expression_handle(
                context,
                source_key,
                local_index,
                initializer,
                expressions,
            )
        }
        _ => expression,
    }
}

pub(super) fn bind_runtime_branch_aliases(
    context: &RuntimeBranchingContext,
    expression_table: &mut ExpressionTable,
    aliases: &mut RuntimeBranchAliasBuffer,
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
                ExpressionNode::Borrow(_)
            ) {
            insert_rebuilt_expression(
                expression_table,
                argument_expression,
                ExpressionNode::Borrow(psi_checked_trees::expression::TableBorrowExpression {
                    target: argument_expression,
                    access: ReferenceAccess::Mutable,
                }),
            )
        } else {
            argument_expression
        };
        let expression = resolve_elided_source_local_expression_handle(
            context,
            state_call.source_key,
            state_call.statement_index,
            expression,
            expression_table,
        );
        let expression = resolve_runtime_branch_alias_expression_handle(
            expression,
            state_call.source_key,
            aliases,
            expression_table,
        );
        aliases.set(RuntimeBranchAlias {
            source_key: state_call.target_key,
            parameter_symbol: argument.parameter_symbol,
            parameter_name: argument.parameter_name.clone(),
            expression,
        });
    }
}
