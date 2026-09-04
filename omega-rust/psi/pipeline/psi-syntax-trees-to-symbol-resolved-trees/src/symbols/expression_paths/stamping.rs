use psi_symbols::SymbolHandle;

pub(in crate::symbols) fn stamp_receiver_path_symbols_in_table(
    expression_table: &mut psi_symbol_resolved_trees::expression::ExpressionTable,
    expression: psi_symbol_resolved_trees::expression::ExpressionHandle,
    head_symbol: SymbolHandle,
    symbol: SymbolHandle,
) {
    match expression_table.expression(expression).clone() {
        psi_symbol_resolved_trees::expression::ExpressionNode::Name(_) => {
            if let psi_symbol_resolved_trees::expression::ExpressionNode::Name(path) =
                expression_table.expression_mut(expression)
            {
                path.head_symbol = head_symbol;
                path.symbol = symbol;
            }
        }
        psi_symbol_resolved_trees::expression::ExpressionNode::Indexed(indexed) => {
            stamp_receiver_path_head_symbol_in_table(
                expression_table,
                indexed.collection,
                head_symbol,
            );
        }
        psi_symbol_resolved_trees::expression::ExpressionNode::Member(member) => {
            stamp_receiver_path_head_symbol_in_table(
                expression_table,
                member.receiver,
                head_symbol,
            );
        }
        psi_symbol_resolved_trees::expression::ExpressionNode::Borrow(inner) => {
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

fn stamp_receiver_path_head_symbol_in_table(
    expression_table: &mut psi_symbol_resolved_trees::expression::ExpressionTable,
    expression: psi_symbol_resolved_trees::expression::ExpressionHandle,
    head_symbol: SymbolHandle,
) {
    match expression_table.expression(expression).clone() {
        psi_symbol_resolved_trees::expression::ExpressionNode::Name(_) => {
            if let psi_symbol_resolved_trees::expression::ExpressionNode::Name(path) =
                expression_table.expression_mut(expression)
            {
                path.head_symbol = head_symbol;
                path.symbol = head_symbol;
            }
        }
        psi_symbol_resolved_trees::expression::ExpressionNode::Indexed(indexed) => {
            stamp_receiver_path_head_symbol_in_table(
                expression_table,
                indexed.collection,
                head_symbol,
            );
        }
        psi_symbol_resolved_trees::expression::ExpressionNode::Member(member) => {
            stamp_receiver_path_head_symbol_in_table(
                expression_table,
                member.receiver,
                head_symbol,
            );
        }
        psi_symbol_resolved_trees::expression::ExpressionNode::Borrow(inner) => {
            stamp_receiver_path_head_symbol_in_table(expression_table, inner.target, head_symbol);
        }
        _ => {}
    }
}
