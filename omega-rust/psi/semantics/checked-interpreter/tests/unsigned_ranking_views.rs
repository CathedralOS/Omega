use checked_interpreter::interpret_entry;
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees_to_checked_trees::lower_typed_trees;

#[test]
fn unsigned_identity_views_check_and_execute_each_countdown() {
    let countdown = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../tests/omega/pass/termination/unsigned_identity_measure_rank_range/main.omg"
    ));
    for carrier in ["u8", "u16", "u32", "u64"] {
        let source = format!(
            "{} machine main() -> i32 {{
                let result: {carrier} = walk(5);
                transition result == 0 {{ true -> 7 false -> 0 }}
            }}",
            countdown.replace("u32", carrier),
        );
        let tokens = Lexer::new(&source).tokenize().expect("countdown tokens");
        let syntax = parse_syntax_trees(&tokens).expect("countdown syntax");
        let resolved = lower_syntax_trees(&syntax).expect("countdown symbols");
        let typed = lower_symbol_resolved_trees(&resolved).expect("countdown types");
        let checked = lower_typed_trees(typed).expect("checked unsigned identity view");
        let outcome = interpret_entry(&checked, "main", &[]);
        assert_eq!(outcome.error, None, "{source}");
        assert_eq!(outcome.exit_code, 7, "{source}");
    }
}
