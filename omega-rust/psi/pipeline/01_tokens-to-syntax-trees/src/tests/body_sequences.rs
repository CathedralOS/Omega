use crate::parse_syntax_trees;
use source_files_to_tokens::Lexer;
use syntax_trees::item::Item;
use syntax_trees::statement::StatementNode;

#[test]
fn entry_and_state_bodies_preserve_expanded_statement_order_and_boundaries() {
    let body =
        "let first: u64 = 1; asm where clobbers none { lfence; sfence } transition { _ -> done() }";
    for source in [
        format!("machine process() {{ {body} state done() {{}} }}"),
        format!("machine process() {{ state begin() {{ {body} }} state done() {{}} }}"),
    ] {
        let tokens = Lexer::new(&source).tokenize().unwrap();
        let trees = parse_syntax_trees(&tokens).unwrap();
        let Item::Machine(machine) = trees.root_items().next().unwrap() else {
            panic!("machine root")
        };
        let states = trees.items.state_handles(machine.states);
        assert_eq!(states.len(), 2);
        let statements = trees
            .items
            .statements(trees.items.state(states[0]).statements);
        assert_eq!(statements.len(), 4);
        assert!(matches!(
            trees.statements.statement(statements[0]),
            StatementNode::LocalData(_)
        ));
        for (handle, expected) in [(statements[1], "asm#lfence"), (statements[2], "asm#sfence")] {
            assert!(
                matches!(trees.statements.statement(handle), StatementNode::Call(call) if call.target.as_str() == expected)
            );
        }
        assert!(matches!(
            trees.statements.statement(statements[3]),
            StatementNode::Transition(_)
        ));
        assert!(trees.items.state(states[1]).statements.is_empty());
    }
}

#[test]
fn both_body_forms_reject_bare_arrows_with_their_own_diagnostic() {
    for (source, expected) in [
        ("machine process() { -> done() }", "machine entry bodies"),
        (
            "machine process() { state begin() { -> done() } }",
            "explicit state bodies",
        ),
    ] {
        let tokens = Lexer::new(source).tokenize().unwrap();
        let error = parse_syntax_trees(&tokens).unwrap_err();
        assert!(error.message.starts_with(expected), "{}", error.message);
        assert_eq!(error.source_span.span.start, source.find("->").unwrap());
    }
}
