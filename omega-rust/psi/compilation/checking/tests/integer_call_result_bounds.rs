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

#[test]
fn receiver_method_results_carry_their_declared_policy_into_arithmetic() {
    let messages = |result: &str| -> Vec<String> {
        let program = crate::front_end::typed_program(&format!(
            r#"
            data Counter {{ ticks: {result}; }}
            machine Counter::tick(&mut self) -> {result} {{
                self.ticks
            }}
            machine Counter::digits(&mut self) -> {result} {{
                let digits: {result} = self.tick() * 10 + self.tick();
                digits
            }}
        "#
        ));
        validation::validate_program(&program)
            .err()
            .unwrap_or_default()
            .into_iter()
            .map(|diagnostic| diagnostic.message)
            .collect()
    };
    // The body is a tail read that defeats return-range inference, so only
    // the declared `in Wrapping` policy can admit the arithmetic.
    let wrapping = messages("u32 in Wrapping");
    assert!(wrapping.is_empty(), "{wrapping:?}");
    let exact = messages("u32");
    assert!(
        exact
            .iter()
            .any(|message| message.contains("may overflow `u32`")),
        "an unqualified result still owes the Exact overflow proof: {exact:?}"
    );
}

#[test]
fn relational_result_bounds_substitute_the_selected_call_actual() {
    let source = |actual: &str| {
        format!(
            r#"
            machine bounded(raw: u32 in Wrapping, divisor: u32 in Wrapping) -> u32
            requires divisor > 0
            ensures result < divisor
            {{ (raw % divisor) as u32 }}

            machine value(
                raw: u32 in Wrapping,
                bounded_divisor: u32 in Wrapping,
                unbounded_divisor: u32 in Wrapping
            ) -> i32
            requires bounded_divisor > 0 && bounded_divisor <= 100
                && unbounded_divisor > 0
            {{ bounded(raw, {actual}) as i32 }}
            "#
        )
    };

    validation::validate_specialized_program(
        &crate::front_end::typed_program(&source("bounded_divisor")),
        validation::OpaquePropertyValidation::Required(&[]),
    )
    .expect("the result bound follows the exact selected divisor actual");

    let diagnostics = validation::validate_program(&crate::front_end::typed_program(&source(
        "unbounded_divisor",
    )))
    .expect_err("a different same-carrier actual must not borrow the bounded divisor interval");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("not provably representable")),
        "{diagnostics:#?}"
    );
}

#[test]
fn relational_result_bound_does_not_reuse_a_preceding_argument_snapshot() {
    let program = crate::front_end::typed_program(
        r#"
        machine replace(value: &mut u32) -> u32 in Wrapping {
            value = 4000000000;
            0 as u32 in Wrapping
        }

        machine bounded(raw: u32 in Wrapping, divisor: u32 in Wrapping) -> u32
        requires divisor > 0
        ensures result < divisor
        { (raw % divisor) as u32 }

        machine value() -> i32 {
            let mut divisor: u32 = 3;
            bounded(replace(&mut divisor), divisor as u32 in Wrapping) as i32
        }
        "#,
    );
    let diagnostics = validation::validate_program(&program)
        .expect_err("the later argument must observe the preceding argument's write");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("not provably representable")),
        "{diagnostics:#?}"
    );
}

#[test]
fn typed_integer_operands_reject_implicit_carrier_changes() {
    for source in [
        "machine unsigned() -> u32 in Wrapping { 7 as u32 in Wrapping }
         machine combine(value: i32 in Wrapping) -> i32 in Wrapping { value + unsigned() }",
        "machine small() -> u16 in Wrapping { 7 as u16 in Wrapping }
         machine combine(value: u32 in Wrapping) -> u16 in Wrapping { small() + value }",
        "machine combine(value: u32 in Wrapping) -> u32 in Wrapping {
             value | (3 as u16 in Wrapping)
         }",
        "machine combine(value: u32 in Wrapping, small: u16 in Wrapping) -> u32 in Wrapping {
             value + (small * 2)
         }",
    ] {
        let program = crate::front_end::typed_program(source);
        let diagnostics = validation::validate_program(&program)
            .expect_err("a typed operand cannot silently change integer carrier");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| { diagnostic.message.contains("different integer types") }),
            "{source}: {diagnostics:#?}"
        );
    }
}

#[test]
fn typed_integer_operands_preserve_policy_and_range_obligations() {
    for (source, expected) in [
        (
            "machine exact() -> u32 { 7 }
             machine combine(value: u32 in Wrapping) -> u32 in Wrapping { value + exact() }",
            "mixed arithmetic domains",
        ),
        (
            "machine exact(value: u32) -> u32 { value }
             machine combine(value: u32) -> u32 { exact(value) + 1 }",
            "may overflow",
        ),
    ] {
        let program = crate::front_end::typed_program(source);
        let diagnostics = validation::validate_program(&program)
            .expect_err("operand type compatibility does not discharge policy or range proofs");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "{source}: {diagnostics:#?}"
        );
    }
}

#[test]
fn typed_integer_operands_keep_selected_operator_signatures() {
    let program = crate::front_end::typed_program(
        "operator + u8::sum(left: u8, right: u16) -> u64;
         machine combine(left: u8, right: u16) -> u64 { left + right }",
    );
    validation::validate_program(&program)
        .expect("a declared heterogeneous operator is not builtin integer arithmetic");
}

#[test]
fn typed_integer_operands_keep_comparison_and_shift_count_rules() {
    let program = crate::front_end::typed_program(
        "machine small() -> i8 { -1i8 }
         machine count() -> u16 { 7u16 }
         machine compare(value: u32) -> bool { small() < value }
         machine shift(value: u32 in Wrapping) -> u32 in Wrapping { value << count() }",
    );
    validation::validate_program(&program)
        .expect("integer comparisons and shift counts do not require matching carriers");
}
