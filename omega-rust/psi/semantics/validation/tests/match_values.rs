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

#[test]
fn match_result_destinations_admit_exact_large_intermediates() {
    let result =
        "match flag { true -> 18446744073709551616 / 18446744073709551616, false -> 7 / 2 * 2 }";
    for body in [
        result.to_owned(),
        format!("let saved: u64 = {result}; saved"),
        format!("let mut saved: u64 = 0; saved = {result}; saved"),
        format!("take({result})"),
        format!(
            "transition {{ _ -> finish({result}) }} state finish(value: u64) -> u64 {{ value }}"
        ),
    ] {
        let source = format!(
            "machine take(value: u64) -> u64 {{ value }} machine run(flag: bool) -> u64 {{ {body} }}"
        );
        let errors = diagnostics(&source);
        assert!(errors.is_empty(), "{source}: {errors:?}");
    }
}

#[test]
fn match_result_destinations_do_not_repair_invalid_numeric_arms() {
    for policy in ["", " in Wrapping", " in Saturating", " in Trapping"] {
        let source = format!(
            "machine run(flag: bool) -> u8{policy} {{ match flag {{ _ -> 1, false -> 256 }} }}"
        );
        let errors = diagnostics(&source);
        assert!(
            errors
                .iter()
                .any(|error| error.contains("does not fit destination `u8`")),
            "{source}: {errors:?}"
        );
    }
    for value in [
        "7 / 2",
        "18446744073709551616",
        "18446744073709551616 / 0",
        "18446744073709551616 % 2",
        "1i32",
    ] {
        for pattern in ["true", "_"] {
            let source = format!(
                "machine run(flag: bool) -> u64 {{ match flag {{ {pattern} -> 1, _ -> {value} }} }}"
            );
            assert!(!diagnostics(&source).is_empty(), "{source}");
        }
    }
}

#[test]
fn operand_and_cast_destinations_check_every_anonymous_match_result() {
    for value in ["7 / 2", "256"] {
        let dispatch = format!("match flag {{ _ -> 1, false -> {value} }}");
        for expression in [
            format!("({dispatch}) | 8u8"),
            format!("8u8 | ({dispatch})"),
            format!("({dispatch}) as u8"),
        ] {
            let source = format!("machine run(flag: bool) -> u8 {{ {expression} }}");
            let errors = diagnostics(&source);
            assert!(
                errors.iter().any(|error| error.contains("not an integer")
                    || error.contains("does not fit destination `u8`")),
                "{source}: {errors:?}"
            );
        }
    }
}

#[test]
fn a_widening_cast_cannot_retarget_a_typed_match_arm() {
    for expression in [
        "(match flag { true -> 1u32, false -> 4294967296 }) as u64",
        "(match flag { true -> 4294967296, false -> 1u32 }) as u64",
    ] {
        let errors = diagnostics(&format!(
            "machine run(flag: bool) -> u64 {{ {expression} }}"
        ));
        assert!(
            errors
                .iter()
                .any(|error| error.contains("does not fit destination `u32`")),
            "{expression}: {errors:?}"
        );
    }
}

#[test]
fn a_widening_cast_cannot_retarget_a_computed_integer_match_arm() {
    for arms in [
        "true -> 1u32 + 1u32, false -> 4294967296",
        "true -> 4294967296, false -> 1u32 | 1u32",
    ] {
        let source =
            format!("machine run(flag: bool) -> u64 {{ (match flag {{ {arms} }}) as u64 }}");
        let errors = diagnostics(&source);
        assert!(
            errors
                .iter()
                .any(|error| error.contains("does not fit destination `u32`")),
            "{source}: {errors:?}"
        );
    }
}

#[test]
fn integer_cast_does_not_reland_a_typed_float_match_arm() {
    for arms in [
        "true -> value, false -> 7 / 2",
        "true -> 7 / 2, false -> value",
    ] {
        let source = format!(
            "machine run(flag: bool, value: f64) -> u8 {{ (match flag {{ {arms} }}) as u8 }}"
        );
        let errors = diagnostics(&source);
        // The Exact conversion may still require a finite/in-range proof.
        // Its source arm first lands in the retained f64 result carrier;
        // conversion is not anonymous integer initial landing.
        assert!(
            !errors.iter().any(|error| {
                error.contains("anonymous value") && error.contains("not an integer")
            }),
            "{source}: {errors:?}"
        );
    }
}

#[test]
fn integer_cast_does_not_reland_a_suffixed_float_match_arm() {
    for arms in [
        "true -> 3.5f64, false -> 7 / 2",
        "true -> 7 / 2, false -> 3.5f64",
    ] {
        let source = format!("machine run(flag: bool) -> u8 {{ (match flag {{ {arms} }}) as u8 }}");
        let errors = diagnostics(&source);
        assert!(
            !errors.iter().any(|error| {
                error.contains("anonymous value") && error.contains("not an integer")
            }),
            "{source}: {errors:?}"
        );
    }
}
