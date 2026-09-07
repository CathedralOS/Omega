use super::*;
use source::SourceId;
use source_files_to_tokens::Lexer;
use tokens_to_syntax_trees::parse_syntax_trees_with_id;

fn parse(source: &str) -> SyntaxTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize const fact");
    parse_syntax_trees_with_id(SourceId(17), &tokens).expect("parse const fact")
}

fn generic_fact(expression: &str) -> String {
    format!(
        "const Sizes::COUNT: i64 = 7;
         data Checked<const N: i64> where {expression}, {{ field: i64; }}
         data Main {{ value: Checked<7>; }}"
    )
}

fn domain_fact(expression: &str, membership: &str) -> String {
    format!(
        "domain i64::Direct requires {expression};
         domain i64::Transitive requires self in Direct;
         {}",
        generic_fact(&format!("N in {membership}"))
    )
}

fn assert_case(source: &str, expected_error: Option<&str>, retained: bool, warning_count: usize) {
    let (syntax, warnings) = match normalize_generic_data_with_warnings(parse(source)) {
        Ok(result) => {
            assert!(
                expected_error.is_none(),
                "expected {expected_error:?}: {source}"
            );
            result
        }
        Err(errors) => {
            let Some(expected) = expected_error else {
                panic!("{source}: {errors:?}");
            };
            assert!(
                errors.iter().any(|error| error.message.contains(expected)),
                "{source}: {errors:?}"
            );
            return;
        }
    };
    let instance = syntax
        .root_items()
        .find_map(|item| match item {
            Item::Data(definition) if definition.name.as_str() == "Checked<7>" => Some(definition),
            _ => None,
        })
        .expect("closed generic instance");
    assert_eq!(!instance.where_facts.is_empty(), retained, "{source}");
    assert_eq!(warnings.len(), warning_count, "{source}: {warnings:?}");
    for warning in warnings {
        assert!(
            warning.message.contains("fractional intermediate `7/2`"),
            "{warning:?}"
        );
        assert_eq!(
            warning
                .source_span
                .expect("authored fractional origin")
                .source_id,
            SourceId(17)
        );
    }
}

#[test]
fn anonymous_const_facts_compare_exact_rationals_without_integer_landing() {
    for (true_fact, false_fact) in [
        ("7 / 2 != 3", "7 / 2 == 3"),
        ("7 / 2 > 3", "7 / 2 <= 3"),
        ("7 / 2 < 4", "7 / 2 >= 4"),
        ("7 / 2 >= 7 / 2", "7 / 2 < 7 / 2"),
        ("7 / 2 <= 7 / 2", "7 / 2 != 7 / 2"),
        ("7 / 2 / 2 == 7 / 4", "7 / 2 / 2 == 1"),
        ("7 / 2 * 2 == 7", "7 / 2 * 2 == 6"),
        ("7 / 2 + 1 / 2 == 4", "7 / 2 + 1 / 2 == 3"),
        ("7 / 2 - 1 / 2 == 3", "7 / 2 - 1 / 2 != 3"),
        ("-7 / 2 < -3", "-7 / 2 == -3"),
        ("7 / -2 * 2 == -7", "7 / -2 * 2 == -6"),
        ("-7 / -2 == 7 / 2", "-7 / -2 == 3"),
    ] {
        assert_case(&generic_fact(true_fact), None, false, 0);
        assert_case(&generic_fact(false_fact), Some("is false"), false, 0);
    }
    let large = "1844674407370955161618446744073709551616";
    for expression in [
        format!("{large} * {large} / {large} / {large}"),
        format!("{large} / 3 * 3 / {large}"),
    ] {
        assert_case(&generic_fact(&format!("{expression} == 1")), None, false, 0);
        assert_case(
            &generic_fact(&format!("{expression} == 0")),
            Some("is false"),
            false,
            0,
        );
    }
}

#[test]
fn typed_literals_and_bound_values_keep_integer_quotients_and_remainders() {
    for (expression, expected_error) in [
        ("7i64 / 2 * 2 == 6", None),
        ("7 / 2i64 * 2 == 6", None),
        ("-7i64 / 2 == -3", None),
        ("-7i64 % 2 == -1", None),
        ("N / 2 * 2 == 6", None),
        ("N % 2 == 1", None),
        ("Sizes::COUNT / 2 == 3", None),
        ("N / 2 * 2 == 7", Some("is false")),
        ("7i64 / 2 * 2 == 7", Some("is false")),
        ("N * (7 / 2) == 21", Some("exact anonymous value `7/2`")),
        ("7 / 2 == 3i64", Some("exact anonymous value `7/2`")),
        ("N % (7 / 2) == 0", Some("exact anonymous value `7/2`")),
    ] {
        assert_case(&generic_fact(expression), expected_error, false, 0);
    }
}

#[test]
fn invalid_anonymous_operations_cannot_certify_facts() {
    for (expression, error) in [
        ("7 / 0 == 0", "division by zero"),
        ("7 / (2 - 2) == 3", "division by zero"),
        ("8 % 2 == 0", "integer-typed operand"),
        ("-7 % 2 == -1", "integer-typed operand"),
        ("(7 / 2) % 2 == 1", "integer-typed operand"),
        ("7i64 / 0 == 0", "division by zero"),
        ("7i64 % 0 == 0", "remainder by zero"),
    ] {
        assert_case(&generic_fact(expression), Some(error), false, 0);
        assert_case(
            &domain_fact(expression, "Transitive"),
            Some(error),
            false,
            0,
        );
    }
}

#[test]
fn direct_and_transitive_const_domains_preserve_anonymous_and_typed_facts() {
    for membership in ["Direct", "Transitive"] {
        for (expression, error, warning_count) in [
            ("7 / 2 > 3", None, 0),
            ("7 / 2 * 2 == 7", None, 0),
            ("-7 / 2 < -3", None, 0),
            ("7 / 2 == 3", Some("is false"), 0),
            ("7 / 2 * 2 == 6", Some("is false"), 0),
            ("self / 2 == 3", None, 0),
            ("self % 2 == 1", None, 0),
            ("self / 2 * 2 == 7", Some("is false"), 0),
            (
                "self * (7 / 2) == 21",
                Some("exact anonymous value `7/2`"),
                0,
            ),
            ("self == 7 / 2 * 2", None, 1),
        ] {
            assert_case(
                &domain_fact(expression, membership),
                error,
                false,
                warning_count,
            );
        }
    }
}

#[test]
fn domain_membership_lands_only_integral_anonymous_values() {
    for membership in ["Direct", "Transitive"] {
        for (value, error, warning_count) in [
            ("7 / 2 * 2", None, 1),
            ("7 / 2", Some("exact anonymous value `7/2`"), 0),
            ("7i64 / 2 * 2", Some("is false"), 0),
        ] {
            for expression in [
                format!("({value}) in Seven"),
                format!("self > 0 && (({value}) in Seven)"),
            ] {
                let source = format!(
                    "domain i64::Seven requires self == 7; {}",
                    domain_fact(&expression, membership)
                );
                assert_case(&source, error, false, warning_count);
            }
        }
    }
}

#[test]
fn integer_landing_warnings_require_a_discharged_operation() {
    for (expression, retained, warning_count) in [
        ("N == 7 / 2 * 2", false, 1),
        ("7 / 2 * 2 == N", false, 1),
        ("N * (7 / 2 * 2) == 49", false, 1),
        ("7 / 2 * 2 == 7", false, 0),
        ("(N == 7 / 2 * 2) && field == N", true, 0),
        ("N * (7 / 2 * 2) == field", true, 0),
        ("field == 7 / 2", true, 0),
    ] {
        assert_case(&generic_fact(expression), None, retained, warning_count);
    }
    for expression in [
        "self == 7 / 2 * 2, unknown(self)",
        "(self == 7 / 2 * 2) && unknown(self)",
        "(7 / 2 * 2) in Missing",
    ] {
        assert_case(&domain_fact(expression, "Transitive"), None, true, 0);
    }
}

#[test]
fn authored_operator_facts_remain_for_typed_declaration_selection() {
    for (operator, result_type, expression) in [
        ("/", "i64", "7 / 2 == 3"),
        ("*", "i64", "7 / 2 * 2 == 6"),
        ("+", "i64", "7 / 2 + 1 / 2 == 3"),
        ("%", "i64", "8 % 2 == 0"),
        ("==", "bool", "7 / 2 == 3"),
        (">", "bool", "7 / 2 > 4"),
    ] {
        let declaration =
            format!("operator {operator} i64::authored(left: i64, right: i64) -> {result_type};");
        assert_case(
            &format!("{declaration} {}", generic_fact(expression)),
            None,
            true,
            0,
        );
        assert_case(
            &format!("{declaration} {}", domain_fact(expression, "Transitive")),
            None,
            true,
            0,
        );
    }
}

#[test]
fn unresolved_or_failed_evaluation_preserves_earlier_warnings() {
    for (expression, expected_error) in [
        ("N * (7 / 2 * 2) == field", false),
        ("N * (7 / 2 * 2) == 7 / 2", true),
    ] {
        let syntax = parse(&generic_fact(expression));
        let expression = syntax
            .root_items()
            .find_map(|item| match item {
                Item::Data(definition) if definition.name.as_str() == "Checked" => {
                    match syntax.items.proof_facts(definition.where_facts) {
                        [ProofFact::Expression(expression)] => Some(*expression),
                        _ => None,
                    }
                }
                _ => None,
            })
            .expect("generic expression fact");
        let previous = Diagnostic::warning("earlier successful landing");
        let mut warnings = vec![previous.clone()];
        let result = evaluate_const_fact_expression(
            &syntax,
            expression,
            &HashMap::new(),
            &HashMap::from([("N".to_owned(), 7)]),
            None,
            &mut warnings,
        );
        if expected_error {
            assert!(result.is_err());
        } else {
            assert!(matches!(result, Ok(None)));
        }
        assert_eq!(warnings, vec![previous]);
    }
}
