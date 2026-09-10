use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use tokens_to_syntax_trees::parse_syntax_trees;

fn diagnostics(source: &str) -> Vec<String> {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    validation::validate_program(&typed)
        .err()
        .unwrap_or_default()
        .into_iter()
        .map(|diagnostic| diagnostic.message)
        .collect()
}

#[test]
fn match_checks_every_arm_and_requires_actual_coverage() {
    for (source, expected) in [
        (
            "machine run(flag: bool) -> bool { match flag { true -> true } }",
            "does not cover",
        ),
        (
            "machine run(value: i64) -> i64 { match value { 0 -> 1, 1 -> 2 } }",
            "does not cover",
        ),
        (
            "machine run(flag: bool) -> bool { match flag { true -> true, _ -> 7 } }",
            "incompatible",
        ),
        (
            "machine run(flag: bool) -> bool { match flag { 7 -> true, _ -> false } }",
            "incompatible",
        ),
        (
            "machine run(flag: bool) -> bool { match flag { _ -> true, false -> 7 } }",
            "incompatible",
        ),
        (
            "data Value { number: i64; } machine run(subject: Value, pattern: Value) -> bool { match subject { pattern -> true, _ -> false } }",
            "require scalar",
        ),
    ] {
        let errors = diagnostics(source);
        assert!(
            errors.iter().any(|error| error.contains(expected)),
            "{source}: {errors:?}"
        );
    }
}

#[test]
fn match_boolean_coverage_and_explicit_integer_default_are_valid() {
    for source in [
        "machine run(flag: bool) -> bool { match flag { true -> false, false -> true } }",
        "machine run(value: i64) -> i64 { match value { 0 -> 1, _ -> 2 } }",
        "machine run(flag: bool) -> i32 { match flag { true -> 1, _ -> 2 } }",
        "machine run(flag: &mut bool) -> bool { match flag { true -> false, false -> true } }",
    ] {
        let errors = diagnostics(source);
        assert!(errors.is_empty(), "{source}: {errors:?}");
    }
}

#[test]
fn skipped_match_calls_keep_type_and_scope_checks_but_not_preconditions() {
    let prefix = "machine need(flag: bool) -> bool requires flag == true { true }";
    let accepted = format!(
        "{prefix} machine run() -> bool {{ match true {{ true -> true, _ -> need(false) }} }}"
    );
    assert!(diagnostics(&accepted).is_empty());
    for expression in ["need(7)", "need()", "need(missing)"] {
        let source = format!(
            "{prefix} machine run() -> bool {{ match true {{ true -> true, _ -> {expression} }} }}"
        );
        assert!(!diagnostics(&source).is_empty(), "{source}");
    }
}

#[test]
fn match_custody_limits_are_explicit() {
    for (source, expected) in [
        (
            "machine run(flag: bool, value: &mut i64) -> &mut i64 { match flag { true -> value, _ -> value } }",
            "branch custody join",
        ),
        (
            "data Payload { value: i64; } machine Payload::drop(&mut self) {} machine run(flag: bool) -> Payload { match flag { true -> Payload { value: 1 }, _ -> Payload { value: 2 } } }",
            "branch custody join",
        ),
        (
            "data Payload { value: i64; } machine run(flag: bool, value: Payload) -> Payload { match flag { true -> value, _ -> value } }",
            "branch custody join",
        ),
        (
            "data Payload { value: i64; } machine consume(payload: Payload) -> bool { true } machine run(flag: bool, payload: Payload) -> bool { match flag { true -> consume(payload), _ -> false } }",
            "branch-local transfer",
        ),
    ] {
        let errors = diagnostics(source);
        assert!(
            errors.iter().any(|error| error.contains(expected)),
            "{source}: {errors:?}"
        );
    }
}

#[test]
fn selected_scalar_calls_may_reborrow_and_mutate() {
    let source = "machine clear(flag: &mut bool) -> bool { flag = false; true } machine run(gate: bool, flag: &mut bool) -> bool { match gate { true -> clear(flag), _ -> false } }";
    let errors = diagnostics(source);
    assert!(errors.is_empty(), "{errors:?}");
}
