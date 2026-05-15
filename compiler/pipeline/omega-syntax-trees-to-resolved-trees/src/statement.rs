use crate::expression::lower_expression_handle;
use crate::name::lower_name_members;
use crate::program::Lowerer;
use crate::type_reference::lower_type_reference_handle;
use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolHandle;
use omega_resolved_trees::statement::{
    Assignment, Call, CallStorage, LocalData, LocalDataStorage, NamedTransitionTarget,
    NamedTransitionTargetStorage, Statement, Transition, TransitionGuard, TransitionTarget,
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
            target: crate::name::lower_name(&call.target),
            storage: CallStorage {
                receiver: if call.receiver.is_empty() {
                    None
                } else {
                    Some(lower_name_members(
                        syntax_trees
                            .statements
                            .identifier_path_members(call.receiver)
                            .iter(),
                    ))
                },
                arguments: syntax_trees
                    .statements
                    .expression_handles(call.arguments)
                    .iter()
                    .map(|argument| lower_expression_handle(syntax_trees, *argument))
                    .collect::<Result<Vec<_>, _>>()?,
            },
        })),
        syntax::statement::StatementNode::Expression(expression) => Ok(Statement::Expression(
            lower_expression_handle(syntax_trees, *expression)?,
        )),
        syntax::statement::StatementNode::LocalData(local_data) => {
            Ok(Statement::LocalData(LocalData {
                symbol: SymbolHandle::invalid(),
                name: crate::name::lower_name(&local_data.name),
                storage: LocalDataStorage {
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
            Ok(TransitionTarget::Named(NamedTransitionTarget {
                storage: NamedTransitionTargetStorage {
                    path: lower_name_members(
                        syntax_trees
                            .statements
                            .identifier_path_members(*path)
                            .iter(),
                    ),
                    arguments: syntax_trees
                        .statements
                        .expression_handles(*arguments)
                        .iter()
                        .map(|argument| lower_expression_handle(syntax_trees, *argument))
                        .collect::<Result<Vec<_>, _>>()?,
                },
            }))
        }
        syntax::statement::TransitionTargetNode::Value(expression) => Ok(TransitionTarget::Value(
            lower_expression_handle(syntax_trees, *expression)?,
        )),
        syntax::statement::TransitionTargetNode::SelfTarget => Ok(TransitionTarget::SelfTarget),
        syntax::statement::TransitionTargetNode::Terminal => Ok(TransitionTarget::Terminal),
    }
}
