use crate::expression::lower_expression_into_table;
use crate::program::Lowerer;
use crate::type_reference::lower_type_reference_handle;
use omega_core::arena::HandleSpan;
use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolHandle;
use omega_symbol_resolved_trees::name::DiagnosticName;
use omega_symbol_resolved_trees::statement::{
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
                target: lower_statement_expression(lowerer, syntax_trees, assignment.target)?,
                value: lower_statement_expression(lowerer, syntax_trees, assignment.value)?,
            }))
        }
        syntax::statement::StatementNode::Call(call) => Ok(Statement::Call(Call {
            receiver_symbol: SymbolHandle::invalid(),
            target_symbol: SymbolHandle::invalid(),
            target: crate::name::lower_name(&call.target),
            storage: CallStorage {
                receiver: lower_statement_path_members(lowerer, syntax_trees, call.receiver),
                receiver_starts_at_self: call.receiver_starts_at_self,
                arguments: lower_statement_expressions(lowerer, syntax_trees, call.arguments)?,
            },
        })),
        syntax::statement::StatementNode::Expression(expression) => Ok(Statement::Expression(
            lower_statement_expression(lowerer, syntax_trees, *expression)?,
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
                        lower_statement_expression(lowerer, syntax_trees, local_data.initial_value)?
                    } else {
                        omega_symbol_resolved_trees::expression::ExpressionHandle::invalid()
                    },
                },
            }))
        }
        syntax::statement::StatementNode::Transition(transition) => {
            Ok(Statement::Transition(Transition {
                target: lower_transition_target_node(lowerer, syntax_trees, transition.target)?,
                continuation: if transition.continuation.is_valid() {
                    Some(lower_transition_target_node(
                        lowerer,
                        syntax_trees,
                        transition.continuation,
                    )?)
                } else {
                    None
                },
                guard: lower_transition_guard_node(lowerer, syntax_trees, transition.guard)?,
            }))
        }
    }
}

fn lower_statement_expression(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    expression: syntax::expression::ExpressionHandle,
) -> Result<omega_symbol_resolved_trees::expression::ExpressionHandle, Diagnostic> {
    lower_expression_into_table(
        syntax_trees,
        &mut lowerer.symbol_resolved_trees.tables.bodies.expressions,
        expression,
    )
}

fn lower_statement_expressions(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    expressions: HandleSpan<syntax::expression::ExpressionHandle>,
) -> Result<HandleSpan<omega_symbol_resolved_trees::expression::ExpressionHandle>, Diagnostic> {
    let span = lowerer
        .symbol_resolved_trees
        .tables
        .bodies
        .expressions
        .reserve_expression_handles(expressions.count());

    for (offset, expression) in syntax_trees
        .statements
        .expression_handles(expressions)
        .iter()
        .enumerate()
    {
        let expression = lower_expression_into_table(
            syntax_trees,
            &mut lowerer.symbol_resolved_trees.tables.bodies.expressions,
            *expression,
        )?;
        lowerer
            .symbol_resolved_trees
            .tables
            .bodies
            .expressions
            .set_expression_handle_at_offset(
                span,
                offset
                    .try_into()
                    .expect("expression handle span count overflow"),
                expression,
            );
    }

    Ok(span)
}

fn lower_transition_guard_node(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    guard: syntax::statement::TransitionGuardNode,
) -> Result<TransitionGuard, Diagnostic> {
    match guard {
        syntax::statement::TransitionGuardNode::Always => Ok(TransitionGuard::Always),
        syntax::statement::TransitionGuardNode::When(expression) => Ok(TransitionGuard::When(
            lower_statement_expression(lowerer, syntax_trees, expression)?,
        )),
    }
}

fn lower_transition_target_node(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    target: syntax::statement::TransitionTargetHandle,
) -> Result<TransitionTarget, Diagnostic> {
    match syntax_trees.statements.transition_target(target) {
        syntax::statement::TransitionTargetNode::Named {
            path,
            path_starts_at_self,
            arguments,
        } => Ok(TransitionTarget::Named(NamedTransitionTarget {
            head_symbol: SymbolHandle::invalid(),
            symbol: SymbolHandle::invalid(),
            storage: NamedTransitionTargetStorage {
                path: lower_statement_path_members(lowerer, syntax_trees, *path),
                path_starts_at_self: *path_starts_at_self,
                arguments: lower_statement_expressions(lowerer, syntax_trees, *arguments)?,
            },
        })),
        syntax::statement::TransitionTargetNode::Value(expression) => Ok(TransitionTarget::Value(
            lower_statement_expression(lowerer, syntax_trees, *expression)?,
        )),
        syntax::statement::TransitionTargetNode::SelfTarget => Ok(TransitionTarget::SelfTarget),
        syntax::statement::TransitionTargetNode::Terminal => Ok(TransitionTarget::Terminal),
    }
}

fn lower_statement_path_members(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    members: HandleSpan<syntax::identifier::Identifier>,
) -> HandleSpan<DiagnosticName> {
    let mut span = HandleSpan::empty();

    for member in syntax_trees.statements.identifier_path_members(members) {
        lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .statement_path_members
            .append_to_span(&mut span, crate::name::lower_name(member));
    }

    span
}
