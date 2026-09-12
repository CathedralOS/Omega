fn parse(source: &str) -> typed_trees::TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
    let resolved =
        syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("resolve");
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("type")
}

#[test]
fn boolean_call_promises_keep_actual_values_when_formal_spellings_overlap() {
    for (primitive, formal) in ["bool", "i32"]
        .into_iter()
        .flat_map(|primitive| ["value", "other", "input"].map(|formal| (primitive, formal)))
    {
        for (actual, admitted) in [("value", true), ("other", false)] {
            let program = parse(&format!(
                "machine identity({formal}: {primitive}) -> {primitive} ensures result == {formal} {{ {formal} }}
                 machine compute(value: {primitive}, other: {primitive}) -> {primitive} ensures result == value {{ identity({actual}) }}"
            ));
            let result = crate::lower_typed_trees(program);
            assert_eq!(result.is_ok(), admitted, "{primitive} {formal}: {actual}");
            if let Err(diagnostics) = result {
                assert!(diagnostics.iter().any(|diagnostic| {
                    diagnostic
                        .message
                        .contains("cannot prove ensures contract for exit from compute")
                }));
            }
        }
    }
}
