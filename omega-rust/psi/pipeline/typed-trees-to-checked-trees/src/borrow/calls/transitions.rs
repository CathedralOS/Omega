use super::*;

pub(super) fn collect_transition_target_borrow_calls(
    collection: &mut BorrowCallCollection<'_>,
    target: typed_trees::statement::TransitionTargetHandle,
) {
    match collection.program.statement_table.transition_target(target) {
        TransitionTargetNode::Named {
            arguments, path, ..
        } => {
            let accesses = collection.collect_call_argument_accesses(
                collection
                    .program
                    .statement_table
                    .expression_handles(*arguments),
            );
            collection.append_borrow_call(
                path.head_symbol,
                path.symbol,
                path.members.count() > 1,
                accesses,
            );

            for argument in collection
                .program
                .statement_table
                .expression_handles(*arguments)
            {
                expression::collect_expression_borrow_calls(collection, *argument);
            }
        }
        TransitionTargetNode::Value(expression) => {
            expression::collect_expression_borrow_calls(collection, *expression)
        }
        TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => {}
    }
}
