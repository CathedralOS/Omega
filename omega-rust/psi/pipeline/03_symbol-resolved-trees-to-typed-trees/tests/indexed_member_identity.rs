use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use typed_trees::expression::ExpressionNode;
use typed_trees::statement::StatementNode;

fn resolved_member_fixture() -> symbol_resolved_trees::SymbolResolvedTrees {
    let source = "data Decoy { end: i64; }
        data Endpoint { end: i64; }
        machine read(cells: &[Endpoint; 2]) -> i64 { cells[0].end }";
    crate::front_end::resolved_program(source)
}

fn only_member_symbol(typed: &typed_trees::TypedTrees) -> symbols::SymbolHandle {
    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "read")
        .expect("read machine");
    let state = &typed.machine_states(machine)[0];
    let expression = typed
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .find_map(|statement| match statement {
            StatementNode::Expression(expression) => Some(*expression),
            _ => None,
        })
        .expect("read result");
    let ExpressionNode::Member(member) = typed.expression_table.expression(expression) else {
        panic!("member result");
    };
    member.member_symbol
}

#[test]
fn missing_or_stale_collection_identity_cannot_bind_by_spelling() {
    use symbol_resolved_trees::expression::ExpressionNode as ResolvedExpression;

    let original = resolved_member_fixture();
    let (expression, symbol) = original
        .tables
        .bodies
        .expressions
        .iter_expressions()
        .find_map(|(handle, node)| match node {
            ResolvedExpression::Name(path)
                if original.tables.bodies.expressions.display_name(handle) == "cells" =>
            {
                Some((handle, path.symbol))
            }
            _ => None,
        })
        .expect("collection");
    let stale = symbols::SymbolHandle::from_parts(symbol.arena_index(), symbol.generation() + 1);
    for replacement in [symbols::SymbolHandle::invalid(), stale] {
        let mut resolved = original.clone();
        let ResolvedExpression::Name(path) = resolved
            .tables
            .bodies
            .expressions
            .expression_mut(expression)
        else {
            panic!("collection");
        };
        path.head_symbol = replacement;
        path.symbol = replacement;
        path.member_symbols = arena::HandleSpan::empty();
        // Earlier identity validation may reject the malformed tree; successful
        // lowering must never repair its field from the unchanged spelling.
        if let Ok(typed) = lower_symbol_resolved_trees(&resolved) {
            assert!(!only_member_symbol(&typed).is_valid());
        }
    }
}
