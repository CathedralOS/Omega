//! A preserved readable loan can supply an invocation-fixed ranking endpoint.

use checked_interpreter::{InterpretOptions, interpret_entry};

const SOURCE: &str = r#"
    data Wrap { remaining: u64 [4..=500]; }
    data Indirect { target: &mut Wrap; }
    machine walk(n: u64 [0..=4], indirect: Indirect)
    terminates by n -> Nat::Descending in 0..=indirect.target.remaining;
    -> u64 {
        transition n > 0 {
            true -> walk(n - 1, indirect)
            false -> n
        }
    }
    machine main() -> u64 {
        let mut owner: Wrap = Wrap { remaining: 6 };
        let indirect: Indirect = Indirect { target: &mut owner };
        walk(4, indirect)
    }
"#;

#[test]
fn ranked_countdown_with_stored_exclusive_endpoint_checks_and_interprets() {
    let tokens = source_files_to_tokens::Lexer::new(SOURCE)
        .tokenize()
        .expect("tokens");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("syntax");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .expect("resolve");
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed).expect("rank checks");
    let outcome = interpret_entry(&checked, "main", &[], InterpretOptions::default());
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 0);
}
