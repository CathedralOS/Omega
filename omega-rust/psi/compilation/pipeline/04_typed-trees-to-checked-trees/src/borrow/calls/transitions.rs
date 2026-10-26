use crate::borrow::calls::BorrowCallCollection;
use crate::borrow::calls::expression;
use checked_trees::statement::TransitionTargetNode;

pub(super) fn collect_transition_target_borrow_calls(
    collection: &mut BorrowCallCollection<'_>,
    target: typed_trees::statement::TransitionTargetHandle,
) {
    match collection.program.statement_table.transition_target(target) {
        TransitionTargetNode::Named {
            arguments, path, ..
        } => {
            // Borrow consumers join executable callee states, not declaration
            // heads. Keep the authored path in the call occurrence so machine
            // invocation remains distinct from an internal state transfer.
            // Signature-only targets retain their exact declaration identity.
            let target_symbol = validation::call_target_state(collection.program, path.symbol)
                .map_or(path.symbol, |state| state.symbol);
            let accesses = collection.collect_transition_target_argument_accesses(
                collection
                    .program
                    .statement_table
                    .expression_handles(*arguments),
                target_symbol,
            );
            collection.append_borrow_call(
                Default::default(),
                path.head_symbol,
                target_symbol,
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
        TransitionTargetNode::Terminal => {}
    }
}
