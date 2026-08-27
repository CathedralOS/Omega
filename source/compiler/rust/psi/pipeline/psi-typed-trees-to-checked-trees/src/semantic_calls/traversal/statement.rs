use super::*;
use crate::lookup::statement_call_can_dispatch_to_machine;
use expression::find_call_site_in_expression;

pub(crate) fn find_call_site_in_statement<'program>(
    traversal: &mut CallSiteTraversal<'program, '_>,
    statement: &'program StatementNode,
) -> Option<CallSite<'program>> {
    match statement {
        StatementNode::AssemblyFact(_) => None,
        StatementNode::Assignment(assignment) => {
            find_call_site_in_expression(traversal, assignment.value)
        }
        StatementNode::Call(call) => {
            let is_machine_call = statement_call_can_dispatch_to_machine(
                traversal.program,
                traversal.machine,
                traversal.state,
                call,
            );
            if is_machine_call {
                if traversal.is_target_call_site() {
                    return Some(CallSite::Statement(call));
                }
                traversal.advance_call_ordinal();
            }

            for argument in traversal
                .program
                .statement_table
                .expression_handles(call.arguments)
            {
                if let Some(call_site) = find_call_site_in_expression(traversal, *argument) {
                    return Some(call_site);
                }
            }

            None
        }
        StatementNode::Expression(expression) => {
            find_call_site_in_expression(traversal, *expression)
        }
        StatementNode::LocalData(local_data) => {
            if !local_data.initial_value.is_valid() {
                return None;
            }
            find_call_site_in_expression(traversal, local_data.initial_value)
        }
        StatementNode::Transition(transition) => {
            if let TransitionGuardNode::When(expression) = transition.guard
                && let Some(call_site) = find_call_site_in_expression(traversal, expression)
            {
                return Some(call_site);
            }

            if let Some(call_site) =
                find_call_site_in_transition_target(traversal, transition.target)
            {
                return Some(call_site);
            }

            if transition.continuation.is_valid() {
                return find_call_site_in_transition_target(traversal, transition.continuation);
            }

            None
        }
    }
}

fn find_call_site_in_transition_target<'program>(
    traversal: &mut CallSiteTraversal<'program, '_>,
    target: psi_typed_trees::statement::TransitionTargetHandle,
) -> Option<CallSite<'program>> {
    match traversal.program.statement_table.transition_target(target) {
        TransitionTargetNode::Named {
            arguments,
            evidence_arguments,
            path,
            source_span,
            authored_call_selection,
            ..
        } => {
            if traversal.is_target_call_site() {
                return Some(CallSite::TransitionNamed {
                    path,
                    arguments: *arguments,
                    evidence_arguments,
                    source_span: *source_span,
                    authored_call_selection: *authored_call_selection,
                });
            }
            traversal.advance_call_ordinal();

            for argument in traversal
                .program
                .statement_table
                .expression_handles(*arguments)
            {
                if let Some(call_site) = find_call_site_in_expression(traversal, *argument) {
                    return Some(call_site);
                }
            }
            None
        }
        TransitionTargetNode::Value(expression) => {
            find_call_site_in_expression(traversal, *expression)
        }
        TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => None,
    }
}
