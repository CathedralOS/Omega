//! Cast predicates may weaken at a result join; arithmetic policy may not.

use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use tokens_to_syntax_trees::parse_syntax_trees;

const LARGE_RATIO: &str = "18446744073709551616 / 18446744073709551616";

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
fn ranged_policy_cast_supplies_match_peer_integer_landing() {
    for policy in ["Wrapping", "Saturating", "Trapping"] {
        for arms in [
            format!("true -> 1 as u64 [0..=10] in {policy}, false -> {LARGE_RATIO}"),
            format!("true -> {LARGE_RATIO}, false -> 1 as u64 [0..=10] in {policy}"),
        ] {
            let source =
                format!("machine run(flag: bool) -> u64 {{ (match flag {{ {arms} }}) as u64 }}");
            let errors = diagnostics(&source);
            assert!(errors.is_empty(), "{source}: {errors:?}");
        }
    }
}

#[test]
fn match_cannot_hide_failed_range_cast_membership() {
    for policy in ["Wrapping", "Saturating", "Trapping"] {
        let source = format!(
            "machine run(flag: bool) -> u64 {{ (match flag {{ true -> 11 as u64 [0..=10] in {policy}, false -> {LARGE_RATIO} }}) as u64 }}"
        );
        let errors = diagnostics(&source);
        assert!(
            errors
                .iter()
                .any(|error| error.contains("cast target range")),
            "{source}: {errors:?}"
        );
    }
}

#[test]
fn match_predicate_weakening_does_not_erase_incompatible_policies() {
    for arms in [
        "true -> 1 as u64 [0..=10] in Wrapping, false -> 2 as u64 [0..=20] in Saturating",
        "true -> 2 as u64 [0..=20] in Saturating, false -> 1 as u64 [0..=10] in Wrapping",
    ] {
        let source =
            format!("machine run(flag: bool) -> u64 {{ (match flag {{ {arms} }}) as u64 }}");
        let errors = diagnostics(&source);
        assert!(
            errors
                .iter()
                .any(|error| error.contains("match arms produce incompatible")),
            "{source}: {errors:?}"
        );
    }
}

#[test]
fn anonymous_match_peer_does_not_inherit_another_arms_range_predicate() {
    for arms in [
        "true -> 1 as u64 [0..=10] in Wrapping, false -> 11",
        "true -> 11, false -> 1 as u64 [0..=10] in Wrapping",
        "true -> 1 as u64 [0..=10] in Wrapping, false -> 11 as u64 [0..=20] in Wrapping",
        "true -> 11 as u64 [0..=20] in Wrapping, false -> 1 as u64 [0..=10] in Wrapping",
    ] {
        let source =
            format!("machine run(flag: bool) -> u64 {{ (match flag {{ {arms} }}) as u64 }}");
        let errors = diagnostics(&source);
        assert!(errors.is_empty(), "{source}: {errors:?}");
    }
}

#[test]
fn arithmetic_after_ranged_cast_drops_input_predicates_but_keeps_policy() {
    for policy in ["Wrapping", "Saturating", "Trapping"] {
        let source = format!(
            "machine run(flag: bool) -> u64 {{ (match flag {{ true -> (10 as u64 [0..=10] in {policy}) + 1, false -> 11 }}) as u64 }}"
        );
        let errors = diagnostics(&source);
        assert!(errors.is_empty(), "{source}: {errors:?}");
        let source = format!(
            "machine run() -> u64 {{ (((10 as u64 [0..=10] in {policy}) + 1) as u64 [0..=10]) as u64 }}"
        );
        let errors = diagnostics(&source);
        assert!(
            errors
                .iter()
                .any(|error| error.contains("cast target range")),
            "{source}: {errors:?}"
        );
    }
}
