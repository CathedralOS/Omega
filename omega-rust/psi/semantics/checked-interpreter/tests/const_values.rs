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
fn declared_range_endpoints_execute_independently_of_value_and_flow_narrowing() {
    assert_seven(
        "data Scenario { value: u64[0..=256]; }
        machine upper_bound<const N: u64>(value: u64[0..=N]) -> u64 { N }
        machine Scenario::selected(&self) -> i32 {
            transition self.value < 10 {
                true -> narrowed()
                false -> 0
            }
            state narrowed(&self) -> i32 {
                let inferred: u64 = upper_bound(self.value);
                let explicit: u64 = upper_bound<512>(self.value);
                transition inferred == 256 && explicit == 512 { true -> 7 false -> 0 }
            }
        }
        machine main() -> i32 {
            let scenario: Scenario = Scenario { value: 5 };
            scenario.selected()
        }",
    );
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

#[test]
fn explicit_const_arguments_execute_scalars_arrays_records_and_cases() {
    assert_seven("data Config { count: u8; enabled: bool; }
        data Choice { case First(value: u8); case Second(value: u8); }
        const Values::NEGATIVE: i32 = -2;
        const Values::CONFIG: Config = Config { enabled: true, count: 3 };
        const Values::ARRAY: [u8; 2] = [2, 3];
        const Values::CHOICE: Choice = Choice::Second { value: 4 };
        machine boolean<const N: bool>() -> bool { N }
        machine integer<const N: i32>() -> i32 { N }
        machine record<const N: Config>() -> Config { N }
        machine array<const N: [u8; 2]>() -> [u8; 2] { N }
        machine choice<const N: Choice>() -> Choice { N }
        machine check_choice(selected: Choice) -> bool {
            transition selected {
                Choice::First { value } -> false
                Choice::Second { value } -> (value == 4)
            }
        }
        machine main() -> i32 {
            let enabled: bool = boolean<true>();
            let disabled: bool = boolean<false>();
            let negative: i32 = integer<Values::NEGATIVE>();
            let config: Config = record<Values::CONFIG>();
            let items: [u8; 2] = array<Values::ARRAY>();
            let selected: Choice = choice<Values::CHOICE>();
            let correct_choice: bool = check_choice(selected);
            transition enabled && !disabled && negative == -2 && config.enabled && config.count == 3 && items[0] == 2 && items[1] == 3 && correct_choice {
                true -> 7 false -> 0
            }
        }");
}

#[test]
fn specialized_result_temporaries_execute_each_initializer_once() {
    assert_seven(
        "machine value<const N: u64>(calls: &mut i32 in Wrapping) -> u64[0..=N] {
            calls = calls + 1; N
        }
        machine forward(calls: &mut i32 in Wrapping) -> u64 {
            let first: u64 = value<2>(calls);
            value<3>(calls)
        }
        machine main() -> i32 {
            let mut calls: i32 in Wrapping = 0;
            let selected: u64 = forward(&mut calls);
            transition calls == 2 && selected == 3 { true -> 7 false -> 0 }
        }",
    );
}
