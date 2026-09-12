//! Generated tag observations guard active payload reads during ordinary execution.

#[test]
fn nested_sum_equality_executes_only_the_active_payload_comparison() {
    for (left, right, expected) in [
        ("Message::Empty {}", "Message::Empty {}", 7),
        ("Message::Empty {}", "Message::Data { value: 37 }", 11),
        ("Message::Data { value: 37 }", "Message::Empty {}", 11),
        (
            "Message::Data { value: 37 }",
            "Message::Data { value: 37 }",
            7,
        ),
        (
            "Message::Data { value: 37 }",
            "Message::Data { value: 38 }",
            11,
        ),
    ] {
        let source = format!(
            "trait Equatable {{ machine equals(&self, rhs: &Self) -> bool; }}
             data Message {{ case Empty; case Data(value: u64); }}
             MessageEquatable: Message satisfies Equatable;
             data Envelope {{ active: bool; message: Message; }}
             EnvelopeEquatable: Envelope satisfies Equatable;
             machine compare(left: Envelope, right: Envelope) -> bool {{ left == right }}
             machine main() -> i32 {{
                 let left: Envelope = Envelope {{ active: true, message: {left} }};
                 let right: Envelope = Envelope {{ active: true, message: {right} }};
                 transition compare(left, right) {{ true -> 7 false -> 11 }}
             }}"
        );
        let tokens = source_files_to_tokens::Lexer::new(&source)
            .tokenize()
            .expect("tokens");
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("syntax");
        let resolved =
            syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("symbols");
        let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
            .expect("types");
        let checked = typed_trees_to_checked_trees::lower_typed_trees(typed)
            .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:?}"));
        let outcome = checked_interpreter::interpret_entry(&checked, "main", &[]);
        assert_eq!(outcome.error, None, "{left} == {right}");
        assert_eq!(outcome.exit_code, expected, "{left} == {right}");
    }
}
