use checked_interpreter::interpret_entry;
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::{lower_syntax_trees, normalize_generic_data};
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees_to_checked_trees::lower_typed_trees;

fn assert_seven(source: &str) {
    let tokens = Lexer::new(source).tokenize().expect("const value tokens");
    let syntax = parse_syntax_trees(&tokens).expect("const value syntax");
    let syntax = normalize_generic_data(syntax).expect("canonical const arguments");
    let resolved = lower_syntax_trees(&syntax).expect("const value symbols");
    let typed = lower_symbol_resolved_trees(&resolved).expect("const value types");
    let checked = lower_typed_trees(typed).expect("checked const values");
    let outcome = interpret_entry(&checked, "main", &[]);
    assert_eq!(outcome.error, None, "{source}");
    assert_eq!(outcome.exit_code, 7, "{source}");
}

#[test]
fn type_qualified_const_calls_execute_the_selected_instance() {
    assert_seven(
        "data Selector {}
        machine Selector::value<const N: u64>(witness: &[u8; N]) -> u64 { N }
        machine main() -> i32 {
            let pair: [u8; 2] = [0, 0];
            let triple: [u8; 3] = [0, 0, 0];
            let first: u64 = Selector::value(&pair);
            let second: u64 = Selector::value(&triple);
            transition first == 2 && second == 3 { true -> 7 false -> 0 }
        }",
    );
}

#[test]
fn structured_const_instances_execute_distinct_integer_and_boolean_fields() {
    assert_seven("data Config { count: u8; enabled: bool; }
        const Values::FIRST: Config = Config { count: 2, enabled: true };
        const Values::SECOND: Config = Config { enabled: false, count: 3 };
        data Witness<const N: Config> { case Only; }
        data Scenario { first: Witness<Values::FIRST>; second: Witness<Values::SECOND>; }
        machine value<const N: Config>(witness: &Witness<N>) -> Config { N }
        machine Scenario::run(&self) -> i32 {
            let first_value: Config = value(&self.first);
            let second_value: Config = value(&self.second);
            transition first_value.count == 2 && first_value.enabled && second_value.count == 3 && !second_value.enabled {
                true -> 7 false -> 0
            }
        }
        machine main() -> i32 {
            let scenario: Scenario = Scenario { first: Witness::Only, second: Witness::Only };
            scenario.run()
        }");
}
