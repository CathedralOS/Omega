use symbol_resolved_trees::expression::ExpressionNode;
use symbols::SymbolHandle;

pub(in crate::symbols) fn stamp_receiver_path_symbols_in_table(
    expression_table: &mut symbol_resolved_trees::expression::ExpressionTable,
    expression: symbol_resolved_trees::expression::ExpressionHandle,
    head_symbol: SymbolHandle,
    symbol: SymbolHandle,
) {
    match expression_table.expression(expression).clone() {
        ExpressionNode::Name(_) => {
            if let ExpressionNode::Name(path) = expression_table.expression_mut(expression) {
                path.head_symbol = head_symbol;
                path.symbol = symbol;
            }
        }
        ExpressionNode::Indexed(indexed) => {
            stamp_receiver_path_symbols_in_table(
                expression_table,
                indexed.collection,
                head_symbol,
                head_symbol,
            );
        }
        ExpressionNode::Member(member) => {
            stamp_receiver_path_symbols_in_table(
                expression_table,
                member.receiver,
                head_symbol,
                head_symbol,
            );
        }
        ExpressionNode::Borrow(inner) => {
            stamp_receiver_path_symbols_in_table(
                expression_table,
                inner.target,
                head_symbol,
                symbol,
            );
        }
        _ => {}
    }
}
