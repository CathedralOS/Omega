use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees::{TypedTrees, types::PrimitiveType};

fn typed(source: &str) -> TypedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokens");
    let syntax = parse_syntax_trees(&tokens).expect("syntax");
    let resolved = lower_syntax_trees(&syntax).expect("resolved source");
    lower_symbol_resolved_trees(&resolved).expect("typed source")
}

#[test]
fn free_call_remainder_retains_exact_narrowing_evidence() {
    let program = typed(
        "machine identity(input: u16) -> u16 { input }
         machine value(selected: bool, input: u16) -> bool {
             transition selected {
                 true -> finish(selected && (((identity(input) % 256u16) as u8) == 255u8))
                 false -> finish(false)
             }
             state finish(result: bool) -> bool { result }
         }",
    );
    validation::validate_program(&program).expect("valid source");
    let facts = validation::validate_program_after_generic_contract_entailment_with_facts(&program)
        .expect("retained validation facts");
    let [cast] = facts.exact_integer_casts.as_slice() else {
        panic!(
            "one exact narrowing occurrence: {:?}",
            facts.exact_integer_casts
        );
    };
    assert_eq!(cast.source_type, PrimitiveType::U16);
    assert_eq!(cast.target_type, PrimitiveType::U8);
    assert_eq!(cast.minimum, numerics::bignum::BigInt::from_i64(0));
    assert_eq!(cast.maximum, numerics::bignum::BigInt::from_i64(255));
}

#[test]
fn free_call_carrier_does_not_license_unproved_narrowing_or_overflow() {
    for (expression, expected) in [
        ("identity(input) as u8", "not provably representable"),
        ("(identity(input) + 1u16) as u8", "overflow"),
    ] {
        let program = typed(&format!(
            "machine identity(input: u16) -> u16 {{ input }}
             machine value(input: u16) -> u8 {{ {expression} }}"
        ));
        let diagnostics = validation::validate_program(&program).expect_err("unsafe full carrier");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "{expression}: {diagnostics:?}"
        );
    }
}

#[test]
fn free_call_arithmetic_uses_the_declared_result_policy() {
    for policy in ["Wrapping", "Saturating"] {
        let program = typed(&format!(
            "machine identity(input: u8 in {policy}) -> u8 in {policy} {{ input }}
             machine value(input: u8 in {policy}) -> u8 in {policy} {{ identity(input) + 1u8 }}"
        ));
        validation::validate_program(&program).expect("qualified overflow has declared semantics");
    }
    let program = typed(
        "machine wrapped(input: u8 in Wrapping) -> u8 in Wrapping { input }
         machine value(input: u8 in Wrapping, other: u8 in Saturating) -> u8 in Wrapping {
             wrapped(input) + other
         }",
    );
    let diagnostics = validation::validate_program(&program).expect_err("mixed explicit policies");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("mixed arithmetic domains")),
        "{diagnostics:?}"
    );
}

#[test]
fn free_call_result_carrier_does_not_follow_the_destination() {
    let program = typed(
        "machine small(input: u8) -> u8 { input }
         machine identity(input: u16) -> u16 { input }
         machine value(input: u16) -> u8 { identity(input) as u8 }",
    );
    let diagnostics = validation::validate_program(&program).expect_err("selected u16 carrier");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("not provably representable")),
        "{diagnostics:?}"
    );
}

#[test]
fn free_call_declared_range_remains_available_to_exact_arithmetic() {
    let program = typed(
        "machine bounded(input: u16 [0..=254]) -> u16 [0..=254] { input }
         machine value(input: u16 [0..=254]) -> u8 { (bounded(input) + 1u16) as u8 }",
    );
    validation::validate_program(&program).expect("enforced declared range constrains result");
}
