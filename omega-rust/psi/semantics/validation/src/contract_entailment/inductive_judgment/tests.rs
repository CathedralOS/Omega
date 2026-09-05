use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use tokens_to_syntax_trees::parse_syntax_trees;

fn validate(body: &str, contract: &str) -> Result<(), Vec<diagnostics::Diagnostic>> {
    let source = format!("machine value(n: u8) -> u8 ensures {contract} {{ {body} }}");
    let tokens = Lexer::new(&source).tokenize().unwrap();
    let syntax = parse_syntax_trees(&tokens).unwrap();
    let resolved = lower_syntax_trees(&syntax).unwrap();
    let typed = lower_symbol_resolved_trees(&resolved).unwrap();
    crate::validate_program(&typed)
}

#[test]
fn fallthrough_contracts_retain_both_boolean_arm_polarities() {
    for body in [
        "transition n > 0 { true -> 1u8 false -> 0u8 }",
        "transition n > 0 { false -> 0u8 true -> 1u8 }",
    ] {
        validate(body, "result <= n").expect("each arm retains its selected bound");
        let errors = validate(body, "result < n").expect_err("zero arm refutes strict bound");
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("disproved")),
            "{errors:?}"
        );
    }
}

#[test]
fn later_inductive_arms_include_all_preceding_guard_failures() {
    validate(
        "transition { n < 2 -> 0u8 n < 4 -> 1u8 _ -> 4u8 }",
        "result <= n",
    )
    .expect("second arm retains n >= 2 and final arm retains n >= 4");
}

#[test]
fn unreachable_fallback_cannot_refute_an_inductive_contract() {
    validate("transition { n >= 0 -> 0u8 _ -> 255u8 }", "result == 0u8")
        .expect("unsigned entry range makes the fallback unreachable");
}

#[test]
fn mutating_guards_cannot_make_a_reachable_bad_return_vacuous() {
    let source = "
        machine corrupt(n: &mut u8) -> bool { n = 255u8; false }
        machine value(mut n: u8) -> u8 ensures result == 0u8 {
            transition {
                n > 0 -> 0u8
                corrupt(&mut n) -> 0u8
                n > 0 -> 255u8
                _ -> 0u8
            }
        }";
    let tokens = Lexer::new(source).tokenize().unwrap();
    let syntax = parse_syntax_trees(&tokens).unwrap();
    let resolved = lower_syntax_trees(&syntax).unwrap();
    let typed = lower_symbol_resolved_trees(&resolved).unwrap();
    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "value")
        .unwrap();
    assert!(
        crate::proven_machine_contract_expressions(&typed, machine.symbol).is_empty(),
        "input zero reaches the 255 return after corrupt changes n"
    );
}
