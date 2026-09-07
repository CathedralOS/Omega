use super::*;
use source_files_to_tokens::Lexer;
use tokens_to_syntax_trees::parse_syntax_trees;

fn normalize(source: &str) -> Result<SyntaxTrees, Vec<Diagnostic>> {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    normalize_generic_data(parse_syntax_trees(&tokens).expect("parse"))
}

#[test]
fn anonymous_remainder_const_arguments_reject_before_folding() {
    for value in ["7 % 2", "8 % 2", "7 + 8 % 2", "7u64 + 8 % 2"] {
        let source = format!(
            "data Buffer<const N: u64> {{ values: [u8; N]; }} data Main {{ value: Buffer<{value}>; }}"
        );
        let errors = match normalize(&source) {
            Err(errors) => errors,
            Ok(_) => panic!("anonymous remainder acquired the parameter type: {value}"),
        };
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("integer-typed operand")),
            "{source}: {errors:?}"
        );
    }
}

#[test]
fn anonymous_remainder_const_facts_reject_before_discharge() {
    let source = "data Buffer<const N: u64> where 8 % 2 == 0, { values: [u8; N]; } data Main { value: Buffer<2>; }";
    let errors = match normalize(source) {
        Err(errors) => errors,
        Ok(_) => panic!("anonymous remainder became a true fact"),
    };
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("integer-typed operand")),
        "{errors:?}"
    );
}

#[test]
fn anonymous_remainder_const_domain_facts_reject_before_discharge() {
    for expression in [
        "8 % 2 == 0",
        "7 % 2 == 1",
        "-3 % 2 == -1",
        "(7 + 1) % 2 == 0",
        "(7 / 2) % 2 == 1",
        "self > 0 && 8 % 2 == 0",
    ] {
        for membership in ["Checked", "Transitive"] {
            let source = format!(
                "domain u64::Checked requires {expression};
                domain u64::Transitive requires self in Checked;
                data Buffer<const N: u64> where N in {membership}, {{ values: [u8; N]; }}
                data Main {{ value: Buffer<2>; }}"
            );
            let errors = match normalize(&source) {
                Err(errors) => errors,
                Ok(_) => panic!("anonymous remainder became a discharged domain fact: {source}"),
            };
            assert!(
                errors
                    .iter()
                    .any(|error| error.message.contains("integer-typed operand")),
                "{source}: {errors:?}"
            );
        }
    }
}

#[test]
fn typed_const_domain_remainders_preserve_truth_and_divisor_checks() {
    for (expression, expected_error) in [
        ("8u64 % 2 == 0", None),
        ("7 % 2u64 == 1", None),
        ("-3i32 % 2 == -1", None),
        ("self % 2 == 0", None),
        ("Sizes::COUNT % 2 == 1", None),
        ("8 / 2 == 4", None),
        ("8u64 % 2 == 1", Some("is false")),
        ("-3i32 % 2 == 1", Some("is false")),
        ("8u64 % 0 == 0", Some("remainder by zero")),
        ("self % 0 == 0", Some("remainder by zero")),
    ] {
        let source = format!(
            "const Sizes::COUNT: u64 = 7;
            domain u64::Checked requires {expression};
            domain u64::Transitive requires self in Checked;
            data Buffer<const N: u64> where N in Transitive, {{ values: [u8; N]; }}
            data Main {{ value: Buffer<2>; }}"
        );
        match (normalize(&source), expected_error) {
            (Ok(syntax), None) => {
                let instance = syntax
                    .root_items()
                    .find_map(|item| match item {
                        Item::Data(definition) if definition.name.as_str() == "Buffer<2>" => {
                            Some(definition)
                        }
                        _ => None,
                    })
                    .expect("closed generic instance");
                assert!(
                    instance.where_facts.is_empty(),
                    "true domain fact was not discharged"
                );
            }
            (Err(errors), Some(expected)) => assert!(
                errors.iter().any(|error| error.message.contains(expected)),
                "{source}: {errors:?}"
            ),
            (result, expected) => panic!("{source}: expected {expected:?}, got {result:?}"),
        }
    }
}

#[test]
fn typed_and_named_remainder_operands_and_exact_quotients_still_normalize() {
    for value in ["7u64 % 2", "7 % 2u64", "Sizes::COUNT % 2", "8 / 2"] {
        let source = format!(
            "const Sizes::COUNT: u64 = 7; data Buffer<const N: u64> {{ values: [u8; N]; }} data Main {{ value: Buffer<{value}>; }}"
        );
        normalize(&source).unwrap_or_else(|errors| panic!("{source}: {errors:?}"));
    }
    normalize("operator % u64::remainder(left: u64, right: u64) -> u64; machine value(left: u64, right: u64) -> u64 { left % right }")
        .expect("authored typed operator application remains for selection");
}

#[test]
fn authored_const_operator_spelling_does_not_trigger_builtin_remainder_rejection() {
    for (operator, value) in [("%", "7 % 2"), ("*", "7 * 2 % 4")] {
        let source = format!(
            "operator {operator} u64::operation(left: u64, right: u64) -> u64; data Buffer<const N: u64> {{ values: [u8; N]; }} data Main {{ value: Buffer<{value}>; }}"
        );
        normalize(&source).unwrap_or_else(|errors| {
            panic!(
                "authored spelling must not be rejected as builtin anonymous remainder: {errors:?}"
            )
        });
    }
}
