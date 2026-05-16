use crate::expression::lower_expression_handle_from_table;
use crate::program::Lowerer;
use crate::type_reference::lower_type_reference_handle_from_table;
use omega_core::arena::HandleSpan;
use omega_core::diagnostics::Diagnostic;
use omega_symbol_resolved_trees as resolved;
use omega_typed_trees as typed;

pub(crate) fn lower_statement_node(
    lowerer: &mut Lowerer,
    statement: &resolved::statement::StatementNode,
) -> Result<typed::statement::StatementNode, Diagnostic> {
    match statement {
        resolved::statement::StatementNode::Assignment(assignment) => Ok(
            typed::statement::StatementNode::Assignment(typed::statement::TableAssignment {
                target: lower_statement_expression(lowerer, assignment.target)?,
                value: lower_statement_expression(lowerer, assignment.value)?,
            }),
        ),
        resolved::statement::StatementNode::Call(call) => {
            let mut arguments = HandleSpan::empty();
            for argument in lowerer
                .source_trees
                .tables
                .bodies
                .expressions
                .expression_handles(call.arguments)
            {
                let argument = lower_statement_expression(lowerer, *argument)?;
                lowerer
                    .typed_trees
                    .statement_table
                    .push_expression_handle(&mut arguments, argument);
            }

            Ok(typed::statement::StatementNode::Call(
                typed::statement::TableCall {
                    receiver_symbol: call.receiver_symbol,
                    target_symbol: call.target_symbol,
                    receiver: lower_statement_path_members(lowerer, call.receiver),
                    target: crate::name::lower_name(&call.target),
                    arguments,
                },
            ))
        }
        resolved::statement::StatementNode::Expression(expression) => {
            Ok(typed::statement::StatementNode::Expression(
                lower_statement_expression(lowerer, *expression)?,
            ))
        }
        resolved::statement::StatementNode::LocalData(local_data) => Ok(
            typed::statement::StatementNode::LocalData(typed::statement::TableLocalData {
                symbol: local_data.symbol,
                name: crate::name::lower_name(&local_data.name),
                type_reference: lower_type_reference_handle_from_table(
                    lowerer,
                    local_data.type_reference,
                )?,
                initial_value: local_data
                    .initial_value
                    .is_valid()
                    .then(|| lower_statement_expression(lowerer, local_data.initial_value))
                    .transpose()?
                    .unwrap_or_else(typed::expression::ExpressionHandle::invalid),
            }),
        ),
        resolved::statement::StatementNode::Transition(transition) => Ok(
            typed::statement::StatementNode::Transition(typed::statement::TableTransition {
                target: lower_transition_target(lowerer, transition.target)?,
                continuation: transition
                    .continuation
                    .is_valid()
                    .then(|| lower_transition_target(lowerer, transition.continuation))
                    .transpose()?
                    .unwrap_or_else(typed::statement::TransitionTargetHandle::invalid),
                guard: lower_transition_guard(lowerer, &transition.guard)?,
            }),
        ),
    }
}

fn lower_statement_expression(
    lowerer: &mut Lowerer,
    expression: resolved::expression::ExpressionHandle,
) -> Result<typed::expression::ExpressionHandle, Diagnostic> {
    lower_expression_handle_from_table(
        &lowerer.source_trees.tables.bodies.expressions,
        &mut lowerer.typed_trees.expression_table,
        expression,
    )
}

fn lower_transition_guard(
    lowerer: &mut Lowerer,
    guard: &resolved::statement::TransitionGuardNode,
) -> Result<typed::statement::TransitionGuardNode, Diagnostic> {
    match guard {
        resolved::statement::TransitionGuardNode::Always => {
            Ok(typed::statement::TransitionGuardNode::Always)
        }
        resolved::statement::TransitionGuardNode::When(expression) => {
            Ok(typed::statement::TransitionGuardNode::When(
                lower_statement_expression(lowerer, *expression)?,
            ))
        }
    }
}

fn lower_transition_target(
    lowerer: &mut Lowerer,
    target: resolved::statement::TransitionTargetHandle,
) -> Result<typed::statement::TransitionTargetHandle, Diagnostic> {
    let target = match lowerer
        .source_trees
        .tables
        .bodies
        .statements
        .transition_target(target)
    {
        resolved::statement::TransitionTargetNode::Named { path, arguments } => {
            let mut lowered_arguments = HandleSpan::empty();
            for argument in lowerer
                .source_trees
                .tables
                .bodies
                .expressions
                .expression_handles(*arguments)
            {
                let argument = lower_statement_expression(lowerer, *argument)?;
                lowerer
                    .typed_trees
                    .statement_table
                    .push_expression_handle(&mut lowered_arguments, argument);
            }

            typed::statement::TransitionTargetNode::Named {
                path: typed::statement::TableNamePath {
                    members: lower_statement_path_members(lowerer, path.members),
                    head_symbol: path.head_symbol,
                    symbol: path.symbol,
                },
                arguments: lowered_arguments,
            }
        }
        resolved::statement::TransitionTargetNode::Value(expression) => {
            typed::statement::TransitionTargetNode::Value(lower_statement_expression(
                lowerer,
                *expression,
            )?)
        }
        resolved::statement::TransitionTargetNode::SelfTarget => {
            typed::statement::TransitionTargetNode::SelfTarget
        }
        resolved::statement::TransitionTargetNode::Terminal => {
            typed::statement::TransitionTargetNode::Terminal
        }
    };

    Ok(lowerer
        .typed_trees
        .statement_table
        .insert_transition_target(target))
}

fn lower_statement_path_members(
    lowerer: &mut Lowerer,
    path: HandleSpan<resolved::name::DiagnosticName>,
) -> HandleSpan<typed::name::ProgramName> {
    let mut lowered_path = HandleSpan::empty();

    for member in lowerer
        .source_trees
        .tables
        .declarations
        .statement_path_members
        .span_or_empty(path)
    {
        lowerer
            .typed_trees
            .statement_table
            .push_name_path_member(&mut lowered_path, crate::name::lower_name(member));
    }

    lowered_path
}
