use super::*;

#[test]
fn structural_entry_identity_requires_plain_contents_through_generic_substitution() {
    for (carrier, stable) in [
        ("Holder<Flag>", true),
        ("Holder<Borrowed>", false),
        ("Holder<Holder<Borrowed>>", false),
        ("&Holder<Flag>", true),
        ("&mut Holder<Flag>", false),
    ] {
        let source = format!(
            "data Flag {{ enabled: bool; }}
             data Borrowed {{ flag: &mut Flag; }}
             data Holder<T> {{ value: T; }}
             machine inspect(holder: {carrier}) {{}}"
        );
        let tokens = source_files_to_tokens::Lexer::new(&source)
            .tokenize()
            .unwrap();
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
        let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
        let program =
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "inspect")
            .unwrap();
        let parameter = &program.state_parameters(&program.machine_states(machine)[0])[0];
        assert_eq!(
            has_stable_observable_contents(&program, parameter.type_reference),
            stable,
            "{carrier}"
        );
    }
}
