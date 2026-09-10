//! Operand-directed selection retains the declaration's result, not an operand carrier.

use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use tokens_to_syntax_trees::parse_syntax_trees;

const LARGE_RATIO: &str = "18446744073709551616 / 18446744073709551616";
const SUM: &str = "operator + u8::sum(left: u8, right: u8) -> u64;";

#[test]
fn declared_tokens_do_not_inherit_builtin_value_or_definedness_laws() {
    let source = "operator / u8::divide(left: u8, right: u8) -> u64; machine run(value: u8) -> u64 { value / 0 }";
    let errors = diagnostics(source);
    assert!(errors.is_empty(), "{source}: {errors:?}");
    for source in [
        "operator - u8::difference(left: u8, right: u8) -> u64; machine run() -> u64 { (1u8 - 1u8) as u64 [0..=0] }".to_owned(),
        format!("{SUM} machine run(left: u8, right: u8) -> u8 {{ (left + right) as u8 }}"),
        format!("{SUM} machine run(value: u8) -> u64 {{ (255u8 * 2u8) + value }}"),
        format!("{SUM} machine run(value: u8) -> u64 {{ (1u8 / 0u8) + value }}"),
    ] {
        let errors = diagnostics(&source);
        assert!(!errors.is_empty(), "unproved builtin law or unchecked operand: {source}");
    }
}

#[test]
fn generic_operands_do_not_make_a_concrete_declared_result_open() {
    let source = format!(
        "operator + Math::sum<T>(left: T, right: T) -> u64; machine run(flag: bool, left: u8, right: u8) -> u64 {{ match flag {{ true -> left + right, false -> {LARGE_RATIO} }} }}"
    );
    let errors = diagnostics(&source);
    assert!(errors.is_empty(), "{source}: {errors:?}");
}

fn diagnostics(source: &str) -> Vec<String> {
    let tokens = Lexer::new(source).tokenize().expect("tokens");
    let syntax = parse_syntax_trees(&tokens).expect("syntax");
    let resolved = lower_syntax_trees(&syntax).expect("resolution");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing");
    validation::validate_program(&typed)
        .err()
        .unwrap_or_default()
        .into_iter()
        .map(|diagnostic| diagnostic.message)
        .collect()
}

#[test]
fn declared_heterogeneous_result_supplies_match_peer_landing_in_both_orders() {
    for arms in [
        format!("true -> left + right, false -> {LARGE_RATIO}"),
        format!("true -> {LARGE_RATIO}, false -> left + right"),
    ] {
        let source = format!(
            "{SUM} machine run(flag: bool, left: u8, right: u8) -> u64 {{ match flag {{ {arms} }} }}"
        );
        let errors = diagnostics(&source);
        assert!(errors.is_empty(), "{source}: {errors:?}");
    }
}

#[test]
fn surrounding_operations_consume_the_declared_result_carrier() {
    for (result, expression) in [
        ("u64", format!("(left + right) | ({LARGE_RATIO})")),
        ("u64", format!("({LARGE_RATIO}) | (left + right)")),
        ("bool", "(left + right) == 256u64".to_owned()),
        ("bool", "256u64 < (left + right)".to_owned()),
    ] {
        let source =
            format!("{SUM} machine run(left: u8, right: u8) -> {result} {{ {expression} }}");
        let errors = diagnostics(&source);
        assert!(errors.is_empty(), "{source}: {errors:?}");
    }
}

#[test]
fn declared_result_does_not_license_fractional_or_out_of_carrier_peers() {
    for value in ["7 / 2", "18446744073709551616"] {
        for arms in [
            format!("true -> left + right, false -> {value}"),
            format!("true -> {value}, false -> left + right"),
        ] {
            let source = format!(
                "{SUM} machine run(flag: bool, left: u8, right: u8) -> u64 {{ match flag {{ {arms} }} }}"
            );
            let errors = diagnostics(&source);
            assert!(!errors.is_empty(), "invalid peer accepted: {source}");
            assert!(
                errors
                    .iter()
                    .any(|error| error.contains("integer") || error.contains("destination")),
                "{source}: {errors:?}"
            );
        }
    }
}

#[test]
fn participating_ambiguous_declarations_cannot_supply_a_result() {
    let source = format!(
        "{SUM} operator + u8::another(left: u8, right: u8) -> u64;
         machine run(left: u8, right: u8) -> u64 {{ left + right }}"
    );
    let errors = diagnostics(&source);
    assert!(
        errors
            .iter()
            .any(|error| error.contains("ambiguous") || error.contains("duplicate")),
        "{source}: {errors:?}"
    );
}

#[test]
fn unrelated_operand_candidate_does_not_change_result_selection() {
    let source = format!(
        "{SUM} operator + bool::unrelated(left: bool, right: bool) -> bool;
         machine run(flag: bool, left: u8, right: u8) -> u64 {{ match flag {{ true -> left + right, false -> {LARGE_RATIO} }} }}"
    );
    let errors = diagnostics(&source);
    assert!(errors.is_empty(), "{source}: {errors:?}");
}

#[test]
fn declared_result_policy_survives_match_and_outer_erasure() {
    for policy in ["Wrapping", "Saturating"] {
        for (other, accepted) in [
            (policy, true),
            (
                if policy == "Wrapping" {
                    "Saturating"
                } else {
                    "Wrapping"
                },
                false,
            ),
        ] {
            let source = format!(
                "operator + u8::sum(left: u8, right: u8) -> u64 in {policy};
                 machine run(flag: bool, left: u8, right: u8) -> u64 {{ (match flag {{ true -> left + right, false -> 1 as u64 in {other} }}) as u64 }}"
            );
            let errors = diagnostics(&source);
            if accepted {
                assert!(errors.is_empty(), "{source}: {errors:?}");
            } else {
                assert!(
                    errors
                        .iter()
                        .any(|error| error.contains("match arms produce incompatible")),
                    "{source}: {errors:?}"
                );
            }
        }
    }
}

#[test]
fn dependent_declared_result_does_not_rebind_formals_to_caller_names() {
    let source = "operator + u8::bounded(left: u8, right: u8) -> u64 [0..=left];
        machine run(value: u8) -> u64 { let left: u64 = 0; (value + value) as u64 [0..=left] }";
    let errors = diagnostics(source);
    assert!(
        !errors.is_empty(),
        "uninstantiated result predicate became caller evidence"
    );
}
