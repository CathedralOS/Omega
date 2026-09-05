use crate::lowerer::Lowerer;
use diagnostics::Diagnostic;
use symbol_resolved_trees as resolved;
use typed_trees as typed;

use super::arguments::{
    lower_statement_argument_span, lower_statement_expression, lower_statement_path_members,
};

pub(super) fn lower_transition_statement(
    lowerer: &mut Lowerer,
    transition: &resolved::statement::TableTransition,
) -> Result<typed::statement::TableTransition, Diagnostic> {
    Ok(typed::statement::TableTransition {
        target: lower_transition_target(lowerer, transition.target)?,
        continuation: transition
            .continuation
            .is_valid()
            .then(|| lower_transition_target(lowerer, transition.continuation))
            .transpose()?
            .unwrap_or_else(typed::statement::TransitionTargetHandle::invalid),
        guard: lower_transition_guard(lowerer, &transition.guard)?,
        proof_selectors: lowerer
            .typed_trees
            .statement_table
            .insert_outcome_proof_selectors(
                lowerer
                    .source_trees
                    .tables
                    .bodies
                    .statements
                    .outcome_proof_selectors(transition.proof_selectors)
                    .iter()
                    .map(|selector| typed::statement::OutcomeProofSelector {
                        output_field: crate::name::lower_name(&selector.output_field),
                        binding: crate::name::lower_name(&selector.binding),
                    }),
            ),
        exit: match transition.exit {
            resolved::statement::TransitionExit::Ordinary => {
                typed::statement::TransitionExit::Ordinary
            }
            resolved::statement::TransitionExit::Crash(cause) => {
                typed::statement::TransitionExit::Crash(match cause {
                    resolved::signature::CrashCause::Trap => typed::signature::CrashCause::Trap,
                    resolved::signature::CrashCause::Abort => typed::signature::CrashCause::Abort,
                })
            }
        },
        source_span: transition.source_span,
    })
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
        resolved::statement::TransitionTargetNode::Named {
            path,
            arguments,
            evidence_arguments,
            source_span,
            authored_call_selection,
        } => {
            let lowered_arguments = lower_statement_argument_span(lowerer, *arguments)?;

            typed::statement::TransitionTargetNode::Named {
                path: typed::statement::TableNamePath {
                    members: lower_statement_path_members(lowerer, path.members),
                    head_symbol: path.head_symbol,
                    symbol: path.symbol,
                },
                arguments: lowered_arguments,
                evidence_arguments: evidence_arguments
                    .iter()
                    .map(crate::name::lower_name)
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
                source_span: *source_span,
                authored_call_selection: *authored_call_selection,
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
