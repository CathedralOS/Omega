fn checked(source: &str) -> Result<checked_trees::CheckedTrees, Vec<diagnostics::Diagnostic>> {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    crate::lower_typed_trees(typed)
}

#[test]
fn immutable_result_aliases_follow_exact_entry_origins_in_both_orientations() {
    for guarantee in ["result == input", "input == result"] {
        for body in [
            "input",
            "transition { _ -> finish(input) } state finish(renamed: u16) -> u16 { renamed }",
        ] {
            let source =
                format!("machine value(input: u16) -> u16 ensures {guarantee} {{ {body} }}");
            checked(&source).unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
        }
    }
}

#[test]
fn immutable_result_identity_proves_nonstrict_comparison_combinations() {
    for guarantee in [
        "result <= input",
        "input <= result",
        "result >= input",
        "input >= result",
        "result <= input && result >= input",
        "result < input || result == input",
        "(result <= input && input <= result) || result != input",
    ] {
        for body in [
            "input",
            "transition { _ -> finish(input) } state finish(renamed: u16) -> u16 { renamed }",
        ] {
            let source =
                format!("machine value(input: u16) -> u16 ensures {guarantee} {{ {body} }}");
            checked(&source).unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
        }
    }
}

#[test]
fn identity_does_not_establish_strict_or_selected_comparisons() {
    for guarantee in [
        "result < input",
        "result > input",
        "result != input",
        "result == input && result < input",
        "result < input || result > input",
    ] {
        let source = format!("machine value(input: u16) -> u16 ensures {guarantee} {{ input }}");
        let diagnostics = checked(&source).expect_err("identity cannot prove a strict comparison");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("cannot prove ensures")),
            "{source}: {diagnostics:#?}"
        );
    }
    for spelling in ["==", "<=", ">="] {
        let source = format!(
            r#"
            boundary operator {spelling} Meaning::compare(left: u16, right: u16) -> bool;
            machine value(input: u16) -> u16 ensures result {spelling} input {{ input }}
        "#
        );
        let diagnostics =
            checked(&source).expect_err("selected comparator is not reflexive by builtin identity");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("cannot prove ensures")),
            "{spelling}: {diagnostics:#?}"
        );
    }
}

#[test]
fn equal_carriers_and_parameter_spellings_do_not_replace_value_identity() {
    for source in [
        "machine value(input: u16, other: u16) -> u16 ensures result == input { other }",
        "machine value(input: u16, other: u16) -> u16 ensures result == input { transition { _ -> finish(other) } state finish(input: u16) -> u16 { input } }",
        "machine value(mut input: u16) -> u16 ensures result == input { input }",
        "machine value(input: u16, other: u16, selected: bool) -> u16 ensures result == input { transition selected { true -> finish(input) false -> finish(other) } state finish(saved: u16) -> u16 { saved } }",
        "machine value(mut input: u16) -> u16 ensures result == input { input = 9u16; transition { _ -> finish(input) } state finish(saved: u16) -> u16 { saved } }",
    ] {
        let diagnostics = checked(source).expect_err("no immutable entry-result identity");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("cannot prove ensures")),
            "{source}: {diagnostics:#?}"
        );
    }
}

#[test]
fn changed_entry_backedges_do_not_preserve_the_original_parameter_identity() {
    for target in ["entry", "value"] {
        let source = format!(
            r#"
            machine value(input: u16, finished: bool) -> u16
            ensures result == input
            {{
                transition finished {{
                    true -> (input)
                    false -> {target}(0u16, true)
                }}
            }}
        "#
        );
        let diagnostics = checked(&source).expect_err("entry backedge changes the original input");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("cannot prove ensures")),
            "{target}: {diagnostics:#?}"
        );
    }
}

#[test]
fn identity_preserving_entry_backedges_retain_the_original_parameter() {
    for target in ["entry", "value"] {
        let source = format!(
            r#"
            machine value(input: u16, finished: bool) -> u16
            ensures result == input
            {{
                transition finished {{
                    true -> (input)
                    false -> {target}(input, true)
                }}
            }}
        "#
        );
        checked(&source).unwrap_or_else(|diagnostics| panic!("{target}: {diagnostics:#?}"));
    }
}

#[test]
fn arithmetic_argument_bounds_discharge_the_exact_formal_requirement() {
    for argument in [
        "input % 256u16",
        "(input % 128u16) + 1u16",
        "(input % 128u16) * 2u16",
    ] {
        let source = format!(
            r#"
            machine bounded(ignored: bool, input: u16) -> u16
            requires input < 256u16
            ensures result == input
            {{ input }}
            machine caller(input: u16) -> u16 {{ bounded(false, {argument}) }}
        "#
        );
        checked(&source).unwrap_or_else(|diagnostics| panic!("{argument}: {diagnostics:#?}"));
    }
}

#[test]
fn insufficient_argument_bounds_do_not_satisfy_a_stronger_requirement() {
    let source = r#"
        machine bounded(input: u16) -> u16
        requires input < 128u16
        ensures result == input
        { input }
        machine caller(input: u16) -> u16 { bounded(input % 256u16) }
    "#;
    let diagnostics = checked(source).expect_err("256-valued interval is not below128");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove requires")),
        "{diagnostics:#?}"
    );
}

#[test]
fn negative_division_does_not_preserve_a_positive_argument_bound() {
    let source = r#"
        machine nonnegative(input: i16) -> i16
        requires input >= 0i16
        ensures result == input
        { input }
        machine caller(input: i16 [2..=10], divisor: i16 [-2..=-1]) -> i16
        { nonnegative(input / divisor) }
    "#;
    let diagnostics = checked(source).expect_err("negative quotient cannot satisfy nonnegativity");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove requires")),
        "{diagnostics:#?}"
    );
}

#[test]
fn selected_comparison_and_remainder_meanings_do_not_supply_builtin_bounds() {
    for declaration in [
        "boundary operator < Meaning::before(left: u16, right: u16) -> bool;",
        "boundary operator % Meaning::remainder(left: u16, right: u16) -> u16;",
    ] {
        let source = format!(
            r#"
            {declaration}
            machine bounded(input: u16) -> u16
            requires input < 256u16
            ensures result == input
            {{ input }}
            machine caller(input: u16) -> u16 {{ bounded(input % 256u16) }}
        "#
        );
        assert!(checked(&source).is_err(), "{declaration}");
    }
}

#[test]
fn surviving_scalar_context_proves_stronger_bounds_for_statement_and_expression_calls() {
    for primitive in ["u64", "i64"] {
        for terminator in [";", ""] {
            let source = format!(
                r#"
                data Helper {{}}
                machine Helper::bounded(value: {primitive})
                requires 1{primitive} <= value {{}}
                machine caller(input: {primitive})
                requires 2{primitive} <= input
                {{ Helper::bounded(input){terminator} }}
            "#
            );
            checked(&source).unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
        }
    }
}

#[test]
fn surviving_scalar_context_rejects_weaker_bounds_and_other_parameter_subjects() {
    for primitive in ["u64", "i64"] {
        for terminator in [";", ""] {
            for (callee_bound, caller_bound, subject) in [(2, 1, "input"), (1, 2, "other")] {
                let source = format!(
                    r#"
                    data Helper {{}}
                    machine Helper::bounded(value: {primitive})
                    requires {callee_bound}{primitive} <= value {{}}
                    machine caller(input: {primitive}, other: {primitive})
                    requires {caller_bound}{primitive} <= {subject}
                    {{ Helper::bounded(input){terminator} }}
                "#
                );
                assert_call_requirement_rejected(&source);
            }
        }
    }
}

#[test]
fn surviving_scalar_context_does_not_reuse_mutable_parameter_entry_bounds_after_assignment() {
    for primitive in ["u64", "i64"] {
        for terminator in [";", ""] {
            let source = format!(
                r#"
                data Helper {{}}
                machine Helper::bounded(value: {primitive})
                requires 1{primitive} <= value {{}}
                machine caller(mut input: {primitive})
                requires 2{primitive} <= input
                {{ input = 0{primitive}; Helper::bounded(input){terminator} }}
            "#
            );
            assert_call_requirement_rejected(&source);
        }
    }
}

#[test]
fn surviving_scalar_context_does_not_apply_builtin_order_to_selected_comparators() {
    for primitive in ["u64", "i64"] {
        for terminator in [";", ""] {
            let source = format!(
                r#"
                boundary operator <= Meaning::compare(left: {primitive}, right: {primitive}) -> bool;
                data Helper {{}}
                machine Helper::bounded(value: {primitive})
                requires 1{primitive} <= value {{}}
                machine caller(input: {primitive})
                requires 2{primitive} <= input
                {{ Helper::bounded(input){terminator} }}
            "#
            );
            assert_call_requirement_rejected(&source);
        }
    }
}

fn assert_call_requirement_rejected(source: &str) {
    let diagnostics =
        checked(source).expect_err("caller context cannot discharge this requirement");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove requires")),
        "{source}: {diagnostics:#?}"
    );
}
