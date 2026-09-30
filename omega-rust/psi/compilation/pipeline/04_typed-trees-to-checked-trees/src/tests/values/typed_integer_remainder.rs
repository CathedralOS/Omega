use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::front_end::typed_program_with_generic_data_result;

fn check(source: &str) -> Result<checked_trees::CheckedTrees, String> {
    let typed =
        typed_program_with_generic_data_result(source).map_err(|error| format!("{error:?}"))?;
    lower_typed_trees(typed, &CheckingRequest::settled()).map_err(|error| format!("{error:?}"))
}

fn accepts(source: &str) {
    check(source).unwrap_or_else(|diagnostics| panic!("{source}\n{diagnostics}"));
}

fn rejects_anonymous_remainder(source: &str) {
    let diagnostics = match check(source) {
        Ok(_) => panic!("anonymous remainder acquired integer meaning: {source}"),
        Err(diagnostics) => diagnostics,
    };
    assert!(
        diagnostics.contains('%')
            && diagnostics.contains("integer")
            && diagnostics.contains("operand"),
        "expected the missing integer-typed operand diagnostic: {source}\n{diagnostics:#?}"
    );
}

#[test]
fn an_explicit_integer_operand_preserves_remainder_in_each_context() {
    for (expression, result) in [("7i32 % 2", "1"), ("8i32 % 2", "0"), ("-3i32 % 2", "-1")] {
        accepts(&format!("machine run() -> i32 {{ {expression} }}"));
        accepts(&format!(
            "machine run() -> i32 {{ let value: i32 = {expression}; value }}"
        ));
        accepts(&format!(
            "machine take(value: i32) -> i32 {{ value }}
            machine run() -> i32 {{ take({expression}) }}"
        ));
        accepts(&format!(
            "machine run() -> i32 [0..=(({expression}) + 10)] {{ 0 }}"
        ));
        accepts(&format!(
            "machine run() -> i32 requires ({expression}) == {result}; {{ 0 }}"
        ));
        // Typed remainder evaluation in the proof solver is a separate open
        // slice. This control preserves fact transport across both contexts
        // without asking that solver to compute the remainder from scratch.
        accepts(&format!(
            "machine run() -> i32
                requires ({expression}) == {result};
                ensures ({expression}) == {result};
            {{ 0 }}"
        ));
    }
}

#[test]
fn a_const_generic_argument_cannot_erase_anonymous_remainder_before_checking() {
    for expression in ["7 % 2", "8 % 2"] {
        rejects_anonymous_remainder(&format!(
            "data Buffer<const N: u64> {{ items: [u8; N]; }}
            machine run(value: &Buffer<{expression}>) {{}}"
        ));
    }
    accepts(
        "data Buffer<const N: u64> { items: [u8; N]; }
        machine run(value: &Buffer<7u64 % 2>) {}",
    );
}

#[test]
fn exact_const_quotients_determine_checked_array_bounds() {
    for (argument, last_element) in [
        ("7 / 2 * 2", 6),
        ("7 / 2.0 * 2", 6),
        ("0.1 * 70", 6),
        ("7.0", 6),
        ("7u64 / 2 * 2", 5),
        ("7u64 / 2.0 * 2", 5),
    ] {
        let source = format!(
            "data Buffer<const N: u64> {{ values: [u8; N]; }}
            machine run(value: &Buffer<{argument}>) -> u8 {{ value.values[{last_element}] }}"
        );
        accepts(&source);
        let outside = source.replace(
            &format!("values[{last_element}]"),
            &format!("values[{}]", last_element + 1),
        );
        let errors = check(&outside).expect_err("the first out-of-range element must reject");
        assert!(errors.contains("index"), "{outside}: {errors}");
    }
}

#[test]
fn const_requirements_reach_checking_with_exact_anonymous_meaning() {
    for fact in [
        "7 / 2 * 2 == 7",
        "7 / 2 > 3",
        "7 / 2.0 * 2 == 7",
        "7 / 2.0 > 3",
        "0.1 * 70 == 7",
        "7u64 / 2 == 3",
        "7u64 / 2.0 == 3",
        "N == 7 / 2 * 2 - 5",
        "N == 7 / 2.0 * 2 - 5",
        "N == 0.1 * 70 - 5",
        "N == 2.0",
        "2.0 == N",
    ] {
        accepts(&format!(
            "data Buffer<const N: u64> where {fact}, {{ values: [u8; N]; }}
            machine run(value: &Buffer<2>) -> u8 {{ value.values[0] }}"
        ));
    }
    for fact in ["7 / 2 == 3", "7 / 2.0 == 3", "0.1 * 70 == 6"] {
        let source = format!(
            "data Buffer<const N: u64> where {fact}, {{ values: [u8; N]; }}
            machine run(value: &Buffer<2>) -> u8 {{ value.values[0] }}"
        );
        let errors = check(&source).expect_err("a false const requirement cannot be erased");
        assert!(errors.contains("is false"), "{source}: {errors}");
    }
}

#[test]
fn integer_const_fact_peers_require_integral_anonymous_decimals() {
    for fact in ["N < 7 / 2.0", "7 / 2.0 > N", "N < 2.5", "2.5 > N"] {
        let source = format!(
            "data Buffer<const N: u64> where {fact}, {{ values: [u8; N]; }}
            machine run(value: &Buffer<2>) -> u8 {{ value.values[0] }}"
        );
        // Each comparison would be true over rationals, but N requires its
        // anonymous peer to land as an integer before the comparison.
        let errors = check(&source).expect_err("a named integer peer cannot admit a fraction");
        assert!(
            errors.contains("exact anonymous value") && errors.contains("integer"),
            "{source}: {errors}"
        );
    }
}

#[test]
fn anonymous_decimal_zero_divisors_reject_in_const_arguments_and_facts() {
    for expression in ["1 / 0.0", "7.0 / (2 - 2.0)", "(1.0 / 0) * 0"] {
        for source in [
            format!(
                "data Buffer<const N: u64> {{ values: [u8; N]; }}
                machine run(value: &Buffer<{expression}>) {{}}"
            ),
            format!(
                "data Buffer<const N: u64> where ({expression}) == 0, {{ values: [u8; N]; }}
                machine run(value: &Buffer<2>) {{}}"
            ),
        ] {
            let errors = check(&source).expect_err("anonymous division by zero has no const value");
            assert!(errors.contains("division by zero"), "{source}: {errors}");
        }
    }
}

#[test]
fn typed_float_const_arguments_cannot_implicitly_land_as_integers() {
    for argument in ["7.0f32", "7.0f64", "7.0f64 / 2 * 2", "7 / 2.0f64 * 2"] {
        let source = format!(
            "data Buffer<const N: u64> {{ values: [u8; N]; }}
            machine run(value: &Buffer<{argument}>) -> u8 {{ value.values[0] }}"
        );
        assert!(
            check(&source).is_err(),
            "a typed float requires an explicit integer conversion: {source}"
        );
    }
}

/// Resolve with a source map, as a build does. Source-free resolution mints
/// field symbols without declaration spans, so it cannot identify a closed
/// instance's field copy (`Buffer<2>::count`) with its template's
/// (`Buffer::count`) and refuses any retained field fact of a generic instance.
fn typed_with_sources(source: &str) -> typed_trees::TypedTrees {
    let mut sources = source::SourceMap::default();
    let source_id = sources
        .add(
            std::path::PathBuf::from("tests/main.omg"),
            source.to_owned(),
        )
        .source_id;
    crate::tests::front_end::typed_program_from_source_map_with_generic_data(
        sources,
        &[(source_id, source)],
    )
}

/// A default-domain fact outside the zero folder's fragment is construction
/// debt, not an early refusal (bbef94c08bb). Deferring a template's fact still
/// cannot admit it on the concrete record: the closed instance and the plain
/// record both retain the fact and stay zero-gated, a borrowed value is read
/// under its caller's establishment, and a literal must prove the fact.
#[test]
fn template_deferral_does_not_admit_unsupported_concrete_field_facts() {
    for (declaration, concrete) in [
        (
            "data Buffer<const N: u64> where count / 2 <= N, { count: u64; }",
            "Buffer<2>",
        ),
        (
            "data Buffer where count / 2 <= 2, { count: u64; }",
            "Buffer",
        ),
    ] {
        let source =
            format!("{declaration}\nmachine run(value: &{concrete}) -> u64 {{ value.count }}");
        let typed = typed_with_sources(&source);
        let definition = typed
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == concrete)
            .unwrap_or_else(|| panic!("{source}: concrete record `{concrete}`"));
        assert!(
            definition.zero_gated,
            "{source}: an unsupported fact cannot admit zero construction"
        );
        assert_eq!(
            typed
                .proof_facts
                .span_or_empty(definition.where_facts)
                .len(),
            1,
            "{source}: the concrete record retains its unsupported fact"
        );
        lower_typed_trees(typed, &CheckingRequest::settled())
            .unwrap_or_else(|errors| panic!("{source}: {errors:?}"));

        let literal = format!(
            "{declaration}\nmachine run() -> u64 {{ let value: {concrete} = Buffer {{ count: 9 }}; value.count }}"
        );
        let errors = lower_typed_trees(typed_with_sources(&literal), &CheckingRequest::settled())
            .map(|_| ())
            .expect_err("a literal violating the retained fact cannot establish it");
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("cannot prove the default domain")),
            "{literal}: {errors:?}"
        );
    }
}
