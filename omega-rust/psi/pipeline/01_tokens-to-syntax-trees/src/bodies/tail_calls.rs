use arena::HandleSpan;
use syntax_trees::SyntaxTrees;
use syntax_trees::identifier::Identifier;
use syntax_trees::item::StateHandle;
use syntax_trees::statement::StatementHandle;

/// MR2's rewrite walk (see the call site above): for each state, when the
/// LAST statement is a bare terminal expression that IS a self-entry call,
/// replace it with an always-transition to the entry carrying the same
/// argument expressions.
pub(crate) fn rewrite_terminal_tail_self_calls(
    syntax_trees: &mut SyntaxTrees,
    states: HandleSpan<StateHandle>,
    entry_callable: &Identifier,
    receiver_must_be_self: bool,
) {
    use syntax_trees::expression::ExpressionNode;
    use syntax_trees::statement::{
        StatementNode, TableTransition, TransitionGuardNode, TransitionTargetNode,
    };

    // Phase 1 (reads): collect the rewrite sites.
    let mut sites: Vec<(
        StatementHandle,
        Vec<syntax_trees::expression::ExpressionHandle>,
        source::SourceSpan,
    )> = Vec::new();
    for state_handle in syntax_trees.items.state_handles(states).to_vec() {
        let state = syntax_trees.items.state(state_handle);
        let Some(&last) = syntax_trees.items.statements(state.statements).last() else {
            continue;
        };
        let StatementNode::Expression(expression) = syntax_trees.statements.statement(last) else {
            continue;
        };
        let ExpressionNode::Call(call) = syntax_trees.expressions.expression(*expression) else {
            continue;
        };
        if call.target.as_str() != entry_callable.as_str() {
            continue;
        }
        let receiver_is_self = call.receiver.is_valid()
            && matches!(
                syntax_trees.expressions.expression(call.receiver),
                ExpressionNode::SelfValue
            );
        let shape_matches = if receiver_must_be_self {
            receiver_is_self
        } else {
            !call.receiver.is_valid()
        };
        if !shape_matches {
            continue;
        }
        let arguments = syntax_trees
            .expressions
            .expression_handles(call.arguments)
            .to_vec();
        sites.push((last, arguments, call.target.source_span()));
    }

    // Phase 2 (writes): mint the bare Named loop-back per site.
    for (statement_handle, arguments, source_span) in sites {
        let path_start = syntax_trees
            .statements
            .append_identifier_path_member(entry_callable.clone());
        let path = HandleSpan::from_parts(path_start, 1);
        let mut argument_span = HandleSpan::empty();
        for (index, argument) in arguments.iter().enumerate() {
            let handle = syntax_trees.statements.append_expression_handle(*argument);
            if index == 0 {
                argument_span = HandleSpan::from_parts(handle, arguments.len() as u32);
            }
        }
        let target =
            syntax_trees
                .statements
                .insert_transition_target(TransitionTargetNode::Named {
                    path,
                    path_starts_at_self: false,
                    arguments: argument_span,
                    evidence_arguments: Box::default(),
                    source_span,
                });
        syntax_trees.statements.replace_statement(
            statement_handle,
            StatementNode::Transition(TableTransition {
                target,
                continuation: syntax_trees::statement::TransitionTargetHandle::invalid(),
                guard: TransitionGuardNode::Always,
                proof_selectors: HandleSpan::empty(),
                exit: Default::default(),
                source_span: Default::default(),
            }),
        );
    }
}
