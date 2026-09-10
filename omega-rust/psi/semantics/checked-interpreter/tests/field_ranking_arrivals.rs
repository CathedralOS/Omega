use checked_interpreter::interpret_entry;
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees_to_checked_trees::lower_typed_trees;

#[test]
fn record_field_countdown_executes_through_renamed_state_arrivals() {
    let countdown = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../tests/omega/pass/termination/measure_field_named_arrival/main.omg"
    ));
    check_countdown(countdown, "Countdown { remaining: 5 }, 5");
}

#[test]
fn record_field_countdown_executes_a_computed_initial_arrival() {
    let countdown = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../tests/omega/pass/termination/measure_field_computed_arrival/main.omg"
    ));
    check_countdown(countdown, "Countdown { remaining: 5, limit: 5 }");
}

fn check_countdown(countdown: &str, arguments: &str) {
    let source = format!(
        "{countdown} machine main() -> i32 {{
            let result: u64 = walk({arguments});
            transition result == 0 {{ true -> 7 false -> 0 }}
        }}"
    );
    let tokens = Lexer::new(&source).tokenize().expect("countdown tokens");
    let syntax = parse_syntax_trees(&tokens).expect("countdown syntax");
    let resolved = lower_syntax_trees(&syntax).expect("countdown symbols");
    let typed = lower_symbol_resolved_trees(&resolved).expect("countdown types");
    let checked = lower_typed_trees(typed).expect("checked field arrivals");
    let outcome = interpret_entry(&checked, "main", &[]);
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 7);
}
