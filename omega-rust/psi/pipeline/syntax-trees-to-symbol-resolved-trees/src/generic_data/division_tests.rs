use super::*;
use diagnostics::DiagnosticSeverity;
use source::{SourceId, SourceSpan, Span};
use source_files_to_tokens::Lexer;
use tokens_to_syntax_trees::parse_syntax_trees_with_id;

fn normalize(source: &str) -> Result<(SyntaxTrees, Vec<Diagnostic>), Vec<Diagnostic>> {
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize division case");
    let syntax = parse_syntax_trees_with_id(SourceId(17), &tokens).expect("parse division case");
    normalize_generic_data_with_warnings(syntax)
}

fn instance<'syntax>(
    syntax: &'syntax SyntaxTrees,
    base: &str,
    value: i128,
) -> &'syntax DataDefinition {
    let name = format!("{base}<{value}>");
    let definition = syntax
        .root_items()
        .find_map(|item| match item {
            Item::Data(definition) if definition.name.as_str() == name => Some(definition),
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing instance {name}"));
    let application = definition
        .generic_instance
        .expect("retained generic identity");
    let TypeReferenceNode::Generic {
        base_name,
        arguments,
        ..
    } = syntax.tables.type_references.type_reference(application)
    else {
        panic!("instance must retain its structural generic application");
    };
    assert_eq!(base_name.as_str(), base);
    let [argument] = syntax
        .tables
        .type_references
        .type_reference_handles(*arguments)
    else {
        panic!("expected one const argument");
    };
    assert!(
        matches!(syntax.tables.type_references.type_reference(*argument),
        TypeReferenceNode::Named(name) if name.as_str() == value.to_string())
    );
    definition
}

fn assert_buffer(syntax: &SyntaxTrees, value: usize) {
    let definition = instance(syntax, "Buffer", value as i128);
    let [DataMember::Field(field)] = syntax.tables.items.data_members(definition.members) else {
        panic!("expected the concrete buffer field");
    };
    assert!(
        matches!(syntax.tables.type_references.type_reference(field.type_reference),
        TypeReferenceNode::FixedArray { length: FixedArrayLength::Literal(length), .. }
            if *length == value),
        "array length must contain the evaluated argument"
    );
}

fn assert_fractional_warning(source: &str, warnings: &[Diagnostic], fraction: &str, integer: i128) {
    let [warning] = warnings else {
        panic!("expected one fractional warning: {warnings:?}");
    };
    assert_eq!(warning.severity, DiagnosticSeverity::Warning);
    assert!(
        warning.message.contains(&format!("`{fraction}`")),
        "{warning:?}"
    );
    assert!(
        warning.message.contains(&format!("integer `{integer}`")),
        "{warning:?}"
    );
    assert!(warning.message.contains("type an operand"), "{warning:?}");
    let offset = source.find('/').expect("authored division");
    assert_eq!(
        warning.source_span,
        Some(SourceSpan::new(SourceId(17), Span::new(offset, offset + 1)))
    );
}

fn assert_rejected(source: &str, expected: &str) {
    let errors = normalize(source).expect_err("invalid integer landing must reject");
    assert!(!errors.is_empty());
    assert!(
        errors.iter().all(Diagnostic::is_error),
        "failed normalization returned warnings: {errors:?}"
    );
    assert!(
        errors.iter().any(|error| error.message.contains(expected)),
        "{source}: {errors:?}"
    );
}

#[test]
fn anonymous_division_and_typed_division_instantiate_distinct_values() {
    for (expression, expected, fraction) in [
        ("7 / 2 * 2", 7, Some("7/2")),
        ("7u64 / 2 * 2", 6, None),
        ("4097 / 4096 * 4096", 4097, Some("4097/4096")),
        ("(4097 / 4096) * 4096", 4097, Some("4097/4096")),
        ("4097u64 / 4096 * 4096", 4096, None),
        ("(4097u64 / 4096) * 4096", 4096, None),
        ("8 / 2", 4, None),
    ] {
        let source = format!(
            "data Buffer<const N: u64> {{ values: [u8; N]; }}
             data Main {{ value: Buffer<{expression}>; }}"
        );
        let (syntax, warnings) = normalize(&source).expect("closed integral argument");
        assert_buffer(&syntax, expected);
        if let Some(fraction) = fraction {
            assert_fractional_warning(&source, &warnings, fraction, expected as i128);
        } else {
            assert!(
                warnings.is_empty(),
                "typed or integral division warned: {warnings:?}"
            );
        }
    }
}

#[test]
fn negative_anonymous_cancellation_preserves_the_signed_value() {
    for (expression, expected, fraction) in [
        ("-7 / 2 * 2", -7, Some("-7/2")),
        ("7 / -2 * 2", -7, Some("-7/2")),
        ("-7 / -2 * 2", 7, Some("7/2")),
        ("-7i64 / 2 * 2", -6, None),
    ] {
        let source = format!(
            "data Integer<const N: i64> {{ value: i64; }}
             data Main {{ value: Integer<{expression}>; }}"
        );
        let (syntax, warnings) = normalize(&source).expect("signed integral argument");
        instance(&syntax, "Integer", expected);
        if let Some(fraction) = fraction {
            assert_fractional_warning(&source, &warnings, fraction, expected);
        } else {
            assert!(warnings.is_empty());
        }
    }
}

#[test]
fn named_and_typed_operands_require_nested_anonymous_values_to_land_exactly() {
    for operand in ["Sizes::COUNT", "2u64"] {
        let source = format!(
            "const Sizes::COUNT: u64 = 2;
             data Buffer<const N: u64> {{ values: [u8; N]; }}
             data Main {{ value: Buffer<{operand} * (7 / 2 * 2)>; }}"
        );
        let (syntax, warnings) = normalize(&source).expect("nested exact integer lands");
        assert_buffer(&syntax, 14);
        assert_fractional_warning(&source, &warnings, "7/2", 7);

        assert_rejected(
            &source.replace("7 / 2 * 2", "7 / 2"),
            "exact anonymous value `7/2`",
        );
        let (syntax, warnings) =
            normalize(&source.replace("7 / 2 * 2", "7u64 / 2")).expect("nested typed quotient");
        assert_buffer(&syntax, 6);
        assert!(warnings.is_empty());
    }
}

#[test]
fn fractional_zero_divisor_and_out_of_range_arguments_return_only_errors() {
    for (integer_type, expression, expected) in [
        ("u64", "7 / 2", "exact anonymous value `7/2`"),
        ("u64", "7 / 2 / 2", "exact anonymous value `7/4`"),
        ("u64", "7 / 2 * 2u64", "exact anonymous value `7/2`"),
        ("u64", "7 / 0", "division by zero"),
        ("u64", "7u64 / 0", "division by zero"),
        ("u64", "7 / 0 * 0", "division by zero"),
        ("u8", "511 / 2 * 2", "const value `511` does not fit `u8`"),
        ("u64", "18446744073709551616 / 2 * 2", "64-bit envelope"),
    ] {
        assert_rejected(
            &format!(
                "data Buffer<const N: {integer_type}> {{ values: [u8; N]; }}
             data Main {{ value: Buffer<{expression}>; }}"
            ),
            expected,
        );
    }
    // A warning collected for an earlier successful argument is also discarded.
    assert_rejected(
        "data Buffer<const N: u64> { values: [u8; N]; }
         data Main { good: Buffer<7 / 2 * 2>; bad: Buffer<7 / 2>; }",
        "exact anonymous value `7/2`",
    );
}

#[test]
fn fractional_argument_errors_retain_the_destination_and_source_origin() {
    for integer_type in ["u8", "i32", "u64"] {
        let source = format!(
            "data Buffer<const N: {integer_type}> {{ values: [u8; N]; }}
            data Main {{ value: Buffer<7 / 2>; }}"
        );
        let errors = normalize(&source).expect_err("fractional integer argument");
        let [error] = errors.as_slice() else {
            panic!("one precise landing error: {errors:?}");
        };
        assert!(
            error
                .message
                .contains(&format!("invalid for `{integer_type}`"))
        );
        assert!(error.message.contains("exact anonymous value `7/2`"));
        let offset = source.find('/').expect("division source");
        assert_eq!(
            error.source_span,
            Some(SourceSpan::new(SourceId(17), Span::new(offset, offset + 1)))
        );
    }
}

#[test]
fn unbounded_anonymous_intermediates_cancel_before_the_final_envelope_check() {
    let large = "340282366920938463463374607431768211456";
    for (expression, fraction) in [
        (format!("{large} * {large} / {large} / {large}"), None),
        (
            format!("{large} / 3 * 3 / {large}"),
            Some(format!("{large}/3")),
        ),
    ] {
        let source = format!(
            "data Buffer<const N: u64> {{ values: [u8; N]; }}
             data Main {{ value: Buffer<{expression}>; }}"
        );
        let (syntax, warnings) = normalize(&source).expect("unbounded intermediates cancel to one");
        assert_buffer(&syntax, 1);
        if let Some(fraction) = fraction {
            assert_fractional_warning(&source, &warnings, &fraction, 1);
        } else {
            assert!(warnings.is_empty());
        }
    }
}

#[test]
fn substituted_generic_clones_report_each_authored_fraction_once() {
    let source = "data Buffer<const N: u64> { values: [u8; N]; }
        data Wrapper<const N: u64> { value: Buffer<N * (7 / 2 * 2)>; }
        data Main { first: Wrapper<2>; second: Wrapper<3>; }";
    let (syntax, warnings) = normalize(source).expect("concrete generic clones");
    assert_buffer(&syntax, 14);
    assert_buffer(&syntax, 21);
    assert_fractional_warning(source, &warnings, "7/2", 7);
}

#[test]
fn unused_generic_template_retains_successful_fractional_warning() {
    let source = "data Buffer<const N: u64> { values: [u8; N]; }
        data Wrapper<T> { tag: T; value: Buffer<7 / 2 * 2>; }";
    let (syntax, warnings) = normalize(source).expect("closed expression in open template");
    assert_fractional_warning(source, &warnings, "7/2", 7);
    let template = syntax
        .root_items()
        .find_map(|item| match item {
            Item::Data(definition) if definition.name.as_str() == "Wrapper" => Some(definition),
            _ => None,
        })
        .expect("generic template");
    let field = syntax
        .tables
        .items
        .data_members(template.members)
        .iter()
        .find_map(|member| match member {
            DataMember::Field(field) if field.name.as_str() == "value" => Some(field),
            _ => None,
        })
        .expect("template buffer field");
    let TypeReferenceNode::Generic { arguments, .. } = syntax
        .tables
        .type_references
        .type_reference(field.type_reference)
    else {
        panic!("open template retains its generic application");
    };
    let [argument] = syntax
        .tables
        .type_references
        .type_reference_handles(*arguments)
    else {
        panic!("one template const argument");
    };
    assert!(
        matches!(syntax.tables.type_references.type_reference(*argument),
        TypeReferenceNode::Named(name) if name.as_str() == "7")
    );
}

#[test]
fn closed_indexed_domains_share_exact_division_and_final_range_checks() {
    for (expression, expected, fraction) in [
        ("7 / 2 * 2", "7", Some("7/2")),
        ("7u64 / 2 * 2", "6", None),
        ("4097 / 4096 * 4096", "4097", Some("4097/4096")),
        ("(4097 / 4096) * 4096", "4097", Some("4097/4096")),
        ("4097u64 / 4096 * 4096", "4096", None),
        ("(4097u64 / 4096) * 4096", "4096", None),
    ] {
        let source = format!(
            "domain<T, const N: u64> T::Indexed<N>;
             data Main {{ value: u64 in Indexed<{expression}>; }}"
        );
        let (syntax, warnings) = normalize(&source).expect("closed domain index");
        let domains = syntax.tables.type_references.domain_constraints();
        let [domain] = domains.as_slice() else {
            panic!("one domain application");
        };
        let [argument] = syntax
            .tables
            .type_references
            .type_reference_handles(domain.arguments)
        else {
            panic!("one domain index");
        };
        assert!(
            matches!(syntax.tables.type_references.type_reference(*argument),
            TypeReferenceNode::Named(name) if name.as_str() == expected)
        );
        if let Some(fraction) = fraction {
            assert_fractional_warning(
                &source,
                &warnings,
                fraction,
                expected.parse().expect("integer"),
            );
        } else {
            assert!(warnings.is_empty());
        }
    }
    for (integer_type, expression, expected) in [
        ("u64", "7 / 2", "exact anonymous value `7/2`"),
        ("u64", "7 / 0", "division by zero"),
        ("u8", "511 / 2 * 2", "const value `511` does not fit `u8`"),
    ] {
        assert_rejected(
            &format!(
                "domain<T, const N: {integer_type}> T::Indexed<N>;
             data Main {{ value: u64 in Indexed<{expression}>; }}"
            ),
            expected,
        );
    }
}

#[test]
fn failed_substitution_discards_only_its_own_warnings() {
    let source = "data Buffer<const N: u64> { values: [u8; N]; }
        data Wrapper<const N: u64> { value: Buffer<N * (7 / 2 * 2) / (N - N)>; }";
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize substitution");
    let mut syntax = parse_syntax_trees_with_id(SourceId(17), &tokens).expect("parse substitution");
    let field_type = syntax
        .root_items()
        .find_map(|item| match item {
            Item::Data(definition) if definition.name.as_str() == "Wrapper" => {
                match syntax.tables.items.data_members(definition.members) {
                    [DataMember::Field(field)] => Some(field.type_reference),
                    _ => None,
                }
            }
            _ => None,
        })
        .expect("wrapper field");
    let argument = syntax
        .tables
        .type_references
        .insert_named(Identifier::generated("2"));
    let substitution = HashMap::from([("N".to_owned(), argument)]);
    let previous = Diagnostic::warning("earlier successful evaluation");
    let mut warnings = vec![previous.clone()];
    let substituted = substitute_type_reference(
        &mut syntax,
        field_type,
        &substitution,
        &HashMap::new(),
        &mut warnings,
    );
    assert_eq!(warnings, vec![previous]);
    let TypeReferenceNode::Generic { arguments, .. } =
        syntax.tables.type_references.type_reference(substituted)
    else {
        panic!("substitution retains the generic application");
    };
    let [argument] = syntax
        .tables
        .type_references
        .type_reference_handles(*arguments)
    else {
        panic!("one const argument");
    };
    assert!(matches!(
        syntax.tables.type_references.type_reference(*argument),
        TypeReferenceNode::ConstExpression(_)
    ));
}

#[test]
fn warning_deduplication_preserves_distinct_and_invalid_origins() {
    let origin = SourceSpan::new(SourceId(17), Span::new(4, 5));
    let first = Diagnostic::warning("fraction").with_source_span(origin);
    let other_file = Diagnostic::warning("fraction")
        .with_source_span(SourceSpan::new(SourceId(18), origin.span));
    let other_span = Diagnostic::warning("fraction")
        .with_source_span(SourceSpan::new(origin.source_id, Span::new(8, 9)));
    let missing = Diagnostic::warning("missing source");
    let empty = Diagnostic::warning("empty span").with_source_span(SourceSpan::default());
    let reversed = Diagnostic::warning("reversed span")
        .with_source_span(SourceSpan::new(origin.source_id, Span::new(5, 4)));
    let invalid_source = Diagnostic::warning("invalid source")
        .with_source_span(SourceSpan::new(SourceId(usize::MAX), origin.span));
    let expected = vec![
        first.clone(),
        other_file,
        other_span,
        missing.clone(),
        missing,
        empty.clone(),
        empty,
        reversed.clone(),
        reversed,
        invalid_source.clone(),
        invalid_source,
    ];
    let mut warnings = expected.clone();
    warnings.insert(1, first);
    deduplicate_generic_warnings(&mut warnings);
    assert_eq!(warnings, expected);
}
