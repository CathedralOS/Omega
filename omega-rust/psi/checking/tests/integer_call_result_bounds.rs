#[test]
fn signed_literal_result_bounds_publish_exact_narrowing_facts() {
    for (guarantee, minimum, maximum) in [
        ("result == -7i16", -7, -7),
        ("-7i16 == result", -7, -7),
        ("result > -129i16 && result < 128i16", -128, 127),
        ("-128i16 <= result && 127i16 >= result", -128, 127),
    ] {
        let program = crate::front_end::typed_program(&format!(
            r#"
            machine bounded() -> i16 ensures {guarantee} {{ -7i16 }}
            machine value() -> i8 {{ bounded() as i8 }}
        "#
        ));
        let facts = validation::validate_specialized_program(
            &program,
            validation::OpaquePropertyValidation::Required(&[]),
        )
        .map(|validated| validated.facts)
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
        let program = crate::front_end::typed_program(&format!(
            r#"
            machine bounded({parameter}) -> u16
            requires input < 256u16
            ensures result == input
            {{ input }}
            machine value(input: u16 [0..=255]) -> u8 {{ bounded(input) as u8 }}
        "#
        ));
        let facts = validation::validate_specialized_program(
            &program,
            validation::OpaquePropertyValidation::Required(&[]),
        )
        .map(|validated| validated.facts)
        .unwrap();
        assert_eq!(facts.exact_integer_casts.len(), 1);
        assert_eq!(
            facts.exact_integer_casts[0].maximum,
            numerics::bignum::BigInt::from_i64(255)
        );
    }
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
        let program = crate::front_end::typed_program(&format!(
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
