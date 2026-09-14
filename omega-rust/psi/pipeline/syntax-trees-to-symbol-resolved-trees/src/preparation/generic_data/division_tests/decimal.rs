use super::*;

#[test]
fn decimal_const_intermediates_remain_exact_beyond_machine_precision() {
    for (expression, expected) in [
        ("9007199254740993.0 - 9007199254740992", 1),
        (
            "340282366920938463463374607431768211456.0 / 3 * 3 - 340282366920938463463374607431768211455",
            1,
        ),
        ("-7.0 / 2 * 2", -7),
        ("7 / -2.0 * 2", -7),
        ("-0.0", 0),
        ("0.1 + 0.9", 1),
    ] {
        let source = format!(
            "data Integer<const N: i64> {{ value: i64; }}
             data Main {{ value: Integer<{expression}>; }}"
        );
        let (syntax, _) = normalize(&source).expect("exact decimal cancellation");
        instance(&syntax, "Integer", expected);
    }
}

#[test]
fn decimal_const_landing_checks_integrality_range_and_operand_type() {
    for (integer_type, expression, expected_error) in [
        ("u64", "7.5", "exact anonymous value `15/2`"),
        ("u64", "7 / 2.0", "exact anonymous value `7/2`"),
        ("u64", "2u64 * (7 / 2.0)", "exact anonymous value `7/2`"),
        ("u64", "1.0 / 0.0", "division by zero"),
        ("u64", "(1 / 0.0) * 0", "division by zero"),
        ("u8", "256.0", "const value `256` does not fit `u8`"),
        ("u8", "-0.1 * 10", "const value `-1` does not fit `u8`"),
        ("i64", "9223372036854775808.0", "does not fit `i64`"),
        ("u64", "18446744073709551616.0", "64-bit envelope"),
        ("u64", "7.0f64", "not a symbolic integer const expression"),
        (
            "u64",
            "7.0f32 / 2 * 2",
            "not a symbolic integer const expression",
        ),
        ("u64", "7.0 % 2", "requires an integer-typed operand"),
    ] {
        assert_rejected(
            &format!(
                "data Integer<const N: {integer_type}> {{ value: i64; }}
                 data Main {{ value: Integer<{expression}>; }}"
            ),
            expected_error,
        );
    }
    for (integer_type, expression, expected) in [
        ("u8", "255.0", 255),
        ("i64", "-9223372036854775808.0", i128::from(i64::MIN)),
        ("u64", "18446744073709551615.0", i128::from(u64::MAX)),
        ("u64", "7u64 / 2.0 * 2", 6),
    ] {
        let source = format!(
            "data Integer<const N: {integer_type}> {{ value: i64; }}
             data Main {{ value: Integer<{expression}>; }}"
        );
        let (syntax, warnings) = normalize(&source).expect("representable integer boundary");
        instance(&syntax, "Integer", expected);
        assert!(warnings.is_empty(), "{source}: {warnings:?}");
    }
}

#[test]
fn decimal_fraction_warnings_keep_the_authored_leaf_through_generic_clones() {
    let source = "data Buffer<const N: u64> { values: [u8; N]; }
        data Wrapper<const N: u64> { value: Buffer<N * (0.1 * 70)>; }
        data Main { first: Wrapper<2>; second: Wrapper<3>; }";
    let (syntax, warnings) = normalize(source).expect("cloned decimal arithmetic");
    assert_buffer(&syntax, 14);
    assert_buffer(&syntax, 21);
    let [warning] = warnings.as_slice() else {
        panic!("one warning for the authored fractional leaf: {warnings:?}");
    };
    assert!(warning.message.contains("fractional intermediate `1/10`"));
    assert!(warning.message.contains("integer `7`"));
    let offset = source.find("0.1").expect("authored decimal");
    assert_eq!(
        warning.source_span,
        Some(SourceSpan::new(SourceId(17), Span::new(offset, offset + 3)))
    );

    let source = "data Buffer<const N: u64> { values: [u8; N]; }
        data Main { good: Buffer<0.1 * 70>; bad: Buffer<0.1>; }";
    let errors = normalize(source).expect_err("fractional final value");
    assert!(errors.iter().all(Diagnostic::is_error));
    let offset = source.rfind("0.1").expect("failed decimal");
    assert!(
        errors.iter().any(|error| {
            error.message.contains("exact anonymous value `1/10`")
                && error.source_span
                    == Some(SourceSpan::new(SourceId(17), Span::new(offset, offset + 3)))
        }),
        "{errors:?}"
    );
}

#[test]
fn indexed_domains_share_decimal_integer_landing() {
    for expression in ["7.0", "7 / 2.0 * 2", "0.1 * 70"] {
        let source = format!(
            "domain<T, const N: u64> T::Indexed<N>;
             data Main {{ value: u64 in Indexed<{expression}>; }}"
        );
        let (syntax, _) = normalize(&source).expect("decimal domain argument");
        let domains = syntax.tables.type_references.domain_constraints();
        let [domain] = domains.as_slice() else {
            panic!("one indexed domain");
        };
        let [argument] = syntax
            .tables
            .type_references
            .type_reference_handles(domain.arguments)
        else {
            panic!("one const argument");
        };
        assert!(
            matches!(syntax.tables.type_references.type_reference(*argument),
            TypeReferenceNode::Named(name) if name.as_str() == "7")
        );
    }
    for (expression, expected_error) in [
        ("7.5", "exact anonymous value `15/2`"),
        ("256.0", "const value `256` does not fit `u8`"),
        ("1.0 / 0", "division by zero"),
    ] {
        assert_rejected(
            &format!(
                "domain<T, const N: u8> T::Indexed<N>;
             data Main {{ value: u64 in Indexed<{expression}>; }}"
            ),
            expected_error,
        );
    }
}
