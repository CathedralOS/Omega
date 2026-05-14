use crate::expression::lower_expression;
use crate::name::lower_name_path;
use crate::program::Lowerer;
use crate::type_reference::lower_type_reference_handle;
use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolHandle;
use omega_resolved_trees::statement::{
    Assignment, Call, LocalData, Statement, Transition, TransitionGuard, TransitionTarget,
};
use omega_syntax_trees::{self as syntax, SyntaxTrees};

pub(crate) fn lower_statement_handle(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    statement: syntax::statement::StatementHandle,
) -> Result<Statement, Diagnostic> {
    lower_statement_node(
        lowerer,
        syntax_trees,
        syntax_trees.statements.statement(statement),
    )
}

fn lower_statement_node(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    statement: &syntax::statement::StatementNode,
) -> Result<Statement, Diagnostic> {
    match statement {
        syntax::statement::StatementNode::Assignment(assignment) => {
            Ok(Statement::Assignment(Assignment {
                target: lower_expression_handle(syntax_trees, assignment.target)?,
                value: lower_expression_handle(syntax_trees, assignment.value)?,
            }))
        }
        syntax::statement::StatementNode::Call(call) => Ok(Statement::Call(Call {
            receiver_symbol: SymbolHandle::invalid(),
            target_symbol: SymbolHandle::invalid(),
            receiver: if call.receiver.is_empty() {
                None
            } else {
                Some(lower_name_path(&syntax::identifier::IdentifierPath::from(
                    syntax_trees
                        .statements
                        .identifier_path_members(call.receiver)
                        .to_vec(),
                )))
            },
            target: crate::name::lower_name(&call.target),
            arguments: syntax_trees
                .statements
                .expression_handles(call.arguments)
                .iter()
                .map(|argument| lower_expression_handle(syntax_trees, *argument))
                .collect::<Result<Vec<_>, _>>()?,
        })),
        syntax::statement::StatementNode::Expression(expression) => Ok(Statement::Expression(
            lower_expression_handle(syntax_trees, *expression)?,
        )),
        syntax::statement::StatementNode::LocalData(local_data) => {
            Ok(Statement::LocalData(LocalData {
                symbol: SymbolHandle::invalid(),
                name: crate::name::lower_name(&local_data.name),
                type_reference: lower_type_reference_handle(
                    lowerer,
                    syntax_trees,
                    local_data.type_reference,
                )?,
                initial_value: if local_data.initial_value.is_valid() {
                    Some(lower_expression_handle(
                        syntax_trees,
                        local_data.initial_value,
                    )?)
                } else {
                    None
                },
            }))
        }
        syntax::statement::StatementNode::Transition(transition) => {
            Ok(Statement::Transition(Transition {
                target: lower_transition_target_node(syntax_trees, transition.target)?,
                continuation: if transition.continuation.is_valid() {
                    Some(lower_transition_target_node(
                        syntax_trees,
                        transition.continuation,
                    )?)
                } else {
                    None
                },
                guard: lower_transition_guard_node(syntax_trees, transition.guard)?,
            }))
        }
    }
}

pub(crate) fn lower_expression_handle(
    syntax_trees: &SyntaxTrees,
    expression: syntax::expression::ExpressionHandle,
) -> Result<omega_resolved_trees::expression::Expression, Diagnostic> {
    lower_expression(&rebuild_expression(syntax_trees, expression))
}

fn rebuild_expression(
    syntax_trees: &SyntaxTrees,
    expression: syntax::expression::ExpressionHandle,
) -> syntax::expression::Expression {
    match syntax_trees.expressions.expression(expression) {
        syntax::expression::ExpressionNode::ArrayLiteral(values) => {
            syntax::expression::Expression::ArrayLiteral(
                syntax_trees
                    .expressions
                    .expression_handles(*values)
                    .iter()
                    .map(|value| rebuild_expression(syntax_trees, *value))
                    .collect(),
            )
        }
        syntax::expression::ExpressionNode::Binary(binary) => {
            syntax::expression::Expression::Binary(Box::new(syntax::expression::BinaryExpression {
                left: rebuild_expression(syntax_trees, binary.left),
                operator: binary.operator,
                right: rebuild_expression(syntax_trees, binary.right),
            }))
        }
        syntax::expression::ExpressionNode::Boolean(value) => {
            syntax::expression::Expression::Boolean(*value)
        }
        syntax::expression::ExpressionNode::Cast(cast) => {
            syntax::expression::Expression::Cast(Box::new(syntax::expression::CastExpression {
                value: rebuild_expression(syntax_trees, cast.value),
                target_type: syntax::identifier::IdentifierPath::from(
                    syntax_trees
                        .expressions
                        .identifier_path_members(cast.target_type)
                        .to_vec(),
                ),
            }))
        }
        syntax::expression::ExpressionNode::Call(call) => {
            syntax::expression::Expression::Call(Box::new(syntax::expression::CallExpression {
                receiver: if call.receiver.is_valid() {
                    Some(Box::new(rebuild_expression(syntax_trees, call.receiver)))
                } else {
                    None
                },
                target: call.target.clone(),
                arguments: syntax_trees
                    .expressions
                    .expression_handles(call.arguments)
                    .iter()
                    .map(|argument| rebuild_expression(syntax_trees, *argument))
                    .collect(),
            }))
        }
        syntax::expression::ExpressionNode::Float(value) => {
            syntax::expression::Expression::Float(value.clone())
        }
        syntax::expression::ExpressionNode::Indexed(indexed) => {
            syntax::expression::Expression::Indexed(Box::new(
                syntax::expression::IndexedExpression {
                    collection: rebuild_expression(syntax_trees, indexed.collection),
                    index: rebuild_expression(syntax_trees, indexed.index),
                },
            ))
        }
        syntax::expression::ExpressionNode::Integer(value) => {
            syntax::expression::Expression::Integer(*value)
        }
        syntax::expression::ExpressionNode::Member(member) => {
            syntax::expression::Expression::Member(Box::new(syntax::expression::MemberExpression {
                receiver: rebuild_expression(syntax_trees, member.receiver),
                member: member.member.clone(),
            }))
        }
        syntax::expression::ExpressionNode::Mutable(expression) => {
            syntax::expression::Expression::Mutable(Box::new(rebuild_expression(
                syntax_trees,
                *expression,
            )))
        }
        syntax::expression::ExpressionNode::Name(path) => {
            syntax::expression::Expression::Name(syntax::identifier::IdentifierPath::from(
                syntax_trees
                    .expressions
                    .identifier_path_members(*path)
                    .to_vec(),
            ))
        }
        syntax::expression::ExpressionNode::StructLiteral(struct_literal) => {
            syntax::expression::Expression::StructLiteral(syntax::expression::StructLiteral {
                type_name: struct_literal.type_name.clone(),
                fields: syntax_trees
                    .expressions
                    .struct_fields(struct_literal.fields)
                    .iter()
                    .map(|field| syntax::expression::StructLiteralField {
                        name: field.name.clone(),
                        value: rebuild_expression(syntax_trees, field.value),
                    })
                    .collect(),
            })
        }
        syntax::expression::ExpressionNode::String(value) => {
            syntax::expression::Expression::String(value.clone())
        }
    }
}

fn lower_transition_guard_node(
    syntax_trees: &SyntaxTrees,
    guard: syntax::statement::TransitionGuardNode,
) -> Result<TransitionGuard, Diagnostic> {
    match guard {
        syntax::statement::TransitionGuardNode::Always => Ok(TransitionGuard::Always),
        syntax::statement::TransitionGuardNode::When(expression) => Ok(TransitionGuard::When(
            lower_expression_handle(syntax_trees, expression)?,
        )),
    }
}

fn lower_transition_target_node(
    syntax_trees: &SyntaxTrees,
    target: syntax::statement::TransitionTargetHandle,
) -> Result<TransitionTarget, Diagnostic> {
    match syntax_trees.statements.transition_target(target) {
        syntax::statement::TransitionTargetNode::Named { path, arguments } => {
            Ok(TransitionTarget::Named {
                path: lower_name_path(&syntax::identifier::IdentifierPath::from(
                    syntax_trees
                        .statements
                        .identifier_path_members(*path)
                        .to_vec(),
                )),
                arguments: syntax_trees
                    .statements
                    .expression_handles(*arguments)
                    .iter()
                    .map(|argument| lower_expression_handle(syntax_trees, *argument))
                    .collect::<Result<Vec<_>, _>>()?,
            })
        }
        syntax::statement::TransitionTargetNode::Value(expression) => Ok(TransitionTarget::Value(
            lower_expression_handle(syntax_trees, *expression)?,
        )),
        syntax::statement::TransitionTargetNode::SelfTarget => Ok(TransitionTarget::SelfTarget),
        syntax::statement::TransitionTargetNode::Terminal => Ok(TransitionTarget::Terminal),
    }
}
