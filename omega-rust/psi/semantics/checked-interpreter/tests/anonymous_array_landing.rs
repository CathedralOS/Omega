use checked_interpreter::interpret_entry;
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees_to_checked_trees::lower_typed_trees;

fn execute(source: &str) -> checked_interpreter::InterpretOutcome {
    let tokens = Lexer::new(source).tokenize().expect("array tokens");
    let syntax = parse_syntax_trees(&tokens).expect("array syntax");
    let resolved = lower_syntax_trees(&syntax).expect("array symbols");
    let typed = lower_symbol_resolved_trees(&resolved).expect("array types");
    let checked =
        lower_typed_trees(typed).unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
    interpret_entry(&checked, "main", &[])
}

fn assert_seven(source: &str) {
    let outcome = execute(source);
    assert_eq!(outcome.error, None, "{source}");
    assert_eq!(outcome.exit_code, 7, "{source}");
}

#[test]
fn array_landing_preserves_source_fuel_cost() {
    let anonymous =
        execute("machine main() -> i32 { let values: [i32; 1] = [7 / 2 * 2]; values[0] }");
    let typed =
        execute("machine main() -> i32 { let values: [i32; 1] = [7i32 / 2 * 2]; values[0] }");
    assert_eq!(anonymous.error, None);
    assert_eq!(typed.error, None);
    assert_eq!(anonymous.exit_code, 7);
    assert_eq!(typed.exit_code, 6);
    assert_eq!(anonymous.usage.fuel_units(), typed.usage.fuel_units());
}

#[test]
fn array_landing_keeps_effectful_elements_in_source_order() {
    assert_seven("machine mark(counter: &mut i32, value: i32) -> i32 { counter = value; value }
        machine main() -> i32 {
            let mut counter: i32 = 0;
            let values: [i32; 3] = [mark(&mut counter, 1), 0.1 * 70, mark(&mut counter, 2)];
            transition values[0] == 1 && values[1] == 7 && values[2] == 2 && counter == 2 { true -> 7 false -> 0 }
        }");
}

#[test]
fn local_array_elements_use_exact_integer_values() {
    for expression in [
        "7 / 2 * 2",
        "7 / 2.0 * 2",
        "0.1 * 70",
        "18446744073709551616 / 3 * 3 - 18446744073709551609",
    ] {
        assert_seven(&format!(
            "machine main() -> i32 {{ let values: [i32; 1] = [{expression}]; values[0] }}"
        ));
    }
}

#[test]
fn nested_array_elements_keep_their_declared_types() {
    assert_seven(
        "machine main() -> i32 {
        let values: [[i32; 2]; 1] = [[7 / 2.0 * 2, 7i32 / 2 * 2]];
        transition values[0][0] == 7 && values[0][1] == 6 { true -> 7 false -> 0 }
    }",
    );
}

#[test]
fn array_assignments_and_record_fields_keep_element_destinations() {
    for body in [
        "let mut values: [i32; 1] = [0]; values = [7 / 2.0 * 2]; values[0]",
        "let packet: Packet = Packet { values: [0.1 * 70] }; packet.values[0]",
    ] {
        assert_seven(&format!(
            "data Packet {{ values: [i32; 1]; }} machine main() -> i32 {{ {body} }}"
        ));
    }
}

#[test]
fn ordinary_array_arguments_keep_element_destinations() {
    assert_seven(
        "machine accept(values: [i32; 1]) -> i32 { values[0] }
        machine main() -> i32 { accept([7 / 2.0 * 2]) }",
    );
    assert_seven(
        "machine main() -> i32 {
        transition { _ -> finish([0.1 * 70]) }
        state finish(values: [i32; 1]) -> i32 { values[0] }
    }",
    );
}

#[test]
fn array_returns_keep_element_destinations() {
    for body in ["[7 / 2.0 * 2]", "transition { _ -> [0.1 * 70] }"] {
        assert_seven(&format!(
            "machine make() -> [i32; 1] {{ {body} }}
            machine main() -> i32 {{ let values: [i32; 1] = make(); values[0] }}"
        ));
    }
}
