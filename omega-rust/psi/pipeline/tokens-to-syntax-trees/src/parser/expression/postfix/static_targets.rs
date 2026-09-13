use source_files_to_tokens::Lexer;
use syntax_trees::{SyntaxTrees, expression::ExpressionNode, statement::StatementNode};

#[test]
fn call_roots_retain_distinct_target_occurrences_through_syntax_copy() {
    let text = "const FIRST: u64 = outer(inner()); const SECOND: u64 = library::read(); const THIRD: u64 = receiver.read();";
    let tokens = Lexer::new(text).tokenize().expect("tokens");
    let syntax = crate::parse_syntax_trees(&tokens).expect("syntax");
    let mut copied = SyntaxTrees::new(source::SourceId::default());
    copied.extend_from(&syntax);
    for program in [&syntax, &copied] {
        let mut pending = program
            .root_items()
            .filter_map(|item| match item {
                syntax_trees::item::Item::Const(definition) => Some(definition.value),
                _ => None,
            })
            .collect::<Vec<_>>();
        let mut spans = Vec::new();
        while let Some(expression) = pending.pop() {
            let ExpressionNode::Call(call) = program.expressions.expression(expression) else {
                panic!("call");
            };
            let span = program.expressions.source_span(expression);
            assert_eq!(span, call.target.source_span());
            assert_eq!(&text[span.span.start..span.span.end], call.target.as_str());
            assert!(
                !spans.contains(&span),
                "nested and sibling calls have distinct occurrences"
            );
            spans.push(span);
            pending.extend(
                program
                    .expressions
                    .expression_handles(call.arguments)
                    .iter()
                    .copied(),
            );
        }
        assert_eq!(spans.len(), 4);
    }
}

#[test]
fn static_and_value_call_targets_remain_distinct_after_syntax_copy() {
    let text = r#"
        machine example() {
            library::emit();
            receiver.emit();
            let first: u64 = library::read();
            let second: u64 = receiver.read();
        }
    "#;
    let tokens = Lexer::new(text).tokenize().expect("tokenize calls");
    let syntax = crate::parse_syntax_trees(&tokens).expect("parse calls");
    let mut copied = SyntaxTrees::new(source::SourceId::default());
    copied.extend_from(&syntax);
    for program in [&syntax, &copied] {
        let mut statements = Vec::new();
        let mut expressions = Vec::new();
        for item in program.root_items() {
            let syntax_trees::item::Item::Machine(machine) = item else {
                continue;
            };
            for state in program.items.state_handles(machine.states) {
                let state = program.items.state(*state);
                for statement in program.items.statements(state.statements) {
                    if let StatementNode::Call(call) = program.statements.statement(*statement) {
                        statements.push(call.target_is_static);
                    }
                    if let StatementNode::LocalData(local) =
                        program.statements.statement(*statement)
                    {
                        let ExpressionNode::Call(call) =
                            program.expressions.expression(local.initial_value)
                        else {
                            panic!("local initializer remains a call");
                        };
                        expressions.push(call.target_is_static);
                    }
                }
            }
        }
        assert_eq!(statements, [true, false]);
        assert_eq!(expressions, [true, false]);
    }
}
