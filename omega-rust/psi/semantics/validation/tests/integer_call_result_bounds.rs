use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees::TypedTrees;

fn typed(source: &str) -> TypedTrees {
    let tokens = Lexer::new(source).tokenize().unwrap();
    let syntax = parse_syntax_trees(&tokens).unwrap();
    let resolved = lower_syntax_trees(&syntax).unwrap();
    lower_symbol_resolved_trees(&resolved).unwrap()
}

#[test]
fn result_comparisons_and_boolean_composition_publish_exact_cast_bounds() {
    for (guarantee, minimum, maximum) in [
        ("result == 7u16", 7, 7),
        ("7u16 == result", 7, 7),
        ("result <= 255u16", 0, 255),
        ("result < 256u16", 0, 255),
        ("255u16 >= result", 0, 255),
        ("result >= 7u16 && result < 256u16", 7, 255),
        ("result == 7u16 || result == 9u16", 7, 9),
    ] {
        let program = typed(&format!(
            r#"
            machine bounded() -> u16 ensures {guarantee} {{ 7u16 }}
            machine value() -> u8 {{ bounded() as u8 }}
        "#
        ));
        let facts =
            validation::validate_program_after_generic_contract_entailment_with_facts(&program)
                .unwrap_or_else(|diagnostics| panic!("{guarantee}: {diagnostics:#?}"));
        let [cast] = facts.exact_integer_casts.as_slice() else {
            panic!("one cast: {guarantee}");
        };
        assert_eq!(cast.minimum, numerics::bignum::BigInt::from_i64(minimum));
        assert_eq!(cast.maximum, numerics::bignum::BigInt::from_i64(maximum));
    }
}

#[test]
fn signed_literal_result_bounds_publish_exact_narrowing_facts() {
    for (guarantee, minimum, maximum) in [
        ("result == -7i16", -7, -7),
        ("-7i16 == result", -7, -7),
        ("result > -129i16 && result < 128i16", -128, 127),
        ("-128i16 <= result && 127i16 >= result", -128, 127),
    ] {
        let program = typed(&format!(
            r#"
            machine bounded() -> i16 ensures {guarantee} {{ -7i16 }}
            machine value() -> i8 {{ bounded() as i8 }}
        "#
        ));
        let facts =
            validation::validate_program_after_generic_contract_entailment_with_facts(&program)
                .unwrap_or_else(|diagnostics| panic!("{guarantee}: {diagnostics:#?}"));
        let [cast] = facts.exact_integer_casts.as_slice() else {
            panic!("one cast: {guarantee}");
        };
        assert_eq!(cast.minimum, numerics::bignum::BigInt::from_i64(minimum));
        assert_eq!(cast.maximum, numerics::bignum::BigInt::from_i64(maximum));
    }
}

#[test]
fn result_alias_uses_only_the_formals_declared_or_builtin_required_range() {
    for parameter in ["input: u16 [0..=255]", "input: u16"] {
        let program = typed(&format!(
            r#"
            machine bounded({parameter}) -> u16
            requires input < 256u16
            ensures result == input
            {{ input }}
            machine value(input: u16 [0..=255]) -> u8 {{ bounded(input) as u8 }}
        "#
        ));
        let facts =
            validation::validate_program_after_generic_contract_entailment_with_facts(&program)
                .unwrap();
        assert_eq!(facts.exact_integer_casts.len(), 1);
        assert_eq!(
            facts.exact_integer_casts[0].maximum,
            numerics::bignum::BigInt::from_i64(255)
        );
    }
}

#[test]
fn assignment_result_alias_retains_its_exact_cast_fact() {
    let program = typed(
        r#"
        machine bounded(input: u16) -> u16
        requires input < 256u16
        ensures result == input
        { input }
        machine value(input: u16) -> u8 {
            let mut current: u8 = 0u8;
            current = bounded(input % 256u16) as u8;
            current
        }
        "#,
    );
    let facts = validation::validate_program_after_generic_contract_entailment_with_facts(&program)
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:#?}"));
    let [cast] = facts.exact_integer_casts.as_slice() else {
        panic!("one assignment cast");
    };
    assert_eq!(cast.minimum, numerics::bignum::BigInt::from_i64(0));
    assert_eq!(cast.maximum, numerics::bignum::BigInt::from_i64(255));
}

#[test]
fn assignment_cast_facts_use_the_value_before_each_write() {
    let program = typed(
        r#"
        machine value() -> u16 {
            let mut current: u16 = 7u16;
            current = ((current as u8) as u16) + 1u16;
            current = ((current as u8) as u16) + 1u16;
            current
        }
        "#,
    );
    let facts = validation::validate_program_after_generic_contract_entailment_with_facts(&program)
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:#?}"));
    let casts = facts
        .exact_integer_casts
        .iter()
        .filter(|cast| cast.target_type == typed_trees::types::PrimitiveType::U8)
        .collect::<Vec<_>>();
    assert_eq!(casts.len(), 2);
    for (cast, old_value) in casts.iter().zip([7, 8]) {
        assert_eq!(cast.minimum, numerics::bignum::BigInt::from_i64(old_value));
        assert_eq!(cast.maximum, numerics::bignum::BigInt::from_i64(old_value));
    }
}

#[test]
fn assignment_cast_cannot_reuse_a_value_retired_by_an_earlier_write() {
    let program = typed(
        r#"
        machine value() -> u16 {
            let mut current: u16 = 7u16;
            current = 511u16;
            current = (current as u8) as u16;
            current
        }
        "#,
    );
    let diagnostics = validation::validate_program(&program).unwrap_err();
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("not provably representable")),
        "{diagnostics:#?}"
    );
}

#[test]
fn parameter_named_result_does_not_constrain_the_return_value() {
    let program = typed(
        r#"
        machine bounded(result: u16) -> u16
        requires result == 7u16
        ensures result == 7u16
        { 999u16 }
        machine value() -> u8 { bounded(7u16) as u8 }
    "#,
    );
    let diagnostics = validation::validate_program(&program).unwrap_err();
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("not provably representable")),
        "{diagnostics:#?}"
    );
}

#[test]
fn an_unbounded_disjunct_does_not_inherit_another_arms_result_bound() {
    let program = typed(
        r#"
        machine bounded() -> u16
        ensures result == 7u16 || true == true
        { 999u16 }
        machine value() -> u8 { bounded() as u8 }
    "#,
    );
    let diagnostics = validation::validate_program(&program).unwrap_err();
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("not provably representable")),
        "{diagnostics:#?}"
    );
}

#[test]
fn declared_comparison_meanings_cannot_manufacture_numeric_result_bounds() {
    for (declaration, signature) in [
        (
            "boundary operator == Number::equal(left: u16, right: u16) -> bool;",
            "ensures result == 7u16",
        ),
        (
            "boundary operator < Number::before(left: u16, right: u16) -> bool;",
            "requires input < 256u16\nensures result == input",
        ),
    ] {
        let program = typed(&format!(
            r#"
            {declaration}
            machine bounded(input: u16) -> u16 {signature} {{ input }}
            machine value(input: u16) -> u8 {{ bounded(input) as u8 }}
        "#
        ));
        let diagnostics = validation::validate_program(&program).unwrap_err();
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("not provably representable")),
            "{declaration}: {diagnostics:#?}"
        );
    }
}
