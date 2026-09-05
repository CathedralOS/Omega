use super::*;

fn check(source: &str) -> Result<checked_trees::CheckedTrees, Vec<diagnostics::Diagnostic>> {
    let tokens = Lexer::new(source).tokenize().unwrap();
    let syntax = parse_syntax_trees(&tokens).unwrap();
    let resolved = lower_syntax_trees(&syntax)?;
    let typed = lower_symbol_resolved_trees(&resolved).map_err(|diagnostic| vec![diagnostic])?;
    lower_typed_trees(typed)
}

fn messages(source: &str) -> String {
    check(source)
        .expect_err("invalid proof term must reject")
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn embedding_a_pure_terminating_call_does_not_create_runtime_execution() {
    let checked = check(
        r#"
        machine identity(value: u8) -> u8 terminates; { value }
        machine law(value: u8) -> u8
        ensures embed(identity(value)) == embed(identity(value))
        { value }
    "#,
    )
    .expect("pure total call is a denotational source");
    assert!(
        checked
            .facts
            .flow
            .control
            .calls
            .iter()
            .all(|(_, call)| checked.symbols.name(call.target_symbol) != "identity")
    );
}

#[test]
fn embedding_equality_uses_the_actual_exit_value_and_exact_call_arguments() {
    let different_result = messages(
        r#"
        machine identity(value: u64, other: u64) -> u64
        ensures embed(result) == embed(value)
        { other }
    "#,
    );
    assert!(
        different_result.contains("cannot prove"),
        "{different_result}"
    );
    let different_call = messages(
        r#"
        machine identity(value: u8) -> u8 terminates; { value }
        machine law(value: u8, other: u8) -> u8
        ensures embed(identity(value)) == embed(identity(other))
        { value }
    "#,
    );
    assert!(different_call.contains("cannot prove"), "{different_call}");
}

#[test]
fn embedding_boundary_and_effectful_calls_rejects() {
    for declaration in [
        "boundary machine source(value: u8) -> u8;",
        "boundary trait PortIo {} machine source(value: u8) -> u8 reaches PortIo terminates; { value }",
    ] {
        let source = format!(
            "{declaration} machine law(value: u8) -> u8 requires embed(source(value)) >= 0 {{ value }}"
        );
        let diagnostics = messages(&source);
        assert!(diagnostics.contains("not denotational"), "{diagnostics}");
    }
}

#[test]
fn embedded_calls_reject_missing_and_extra_value_arguments() {
    for arguments in ["", "value, value"] {
        let source = format!(
            "machine identity(value: u8) -> u8 terminates; {{ value }} machine law(value: u8) -> u8 requires embed(identity({arguments})) >= 0 {{ value }}"
        );
        let diagnostics = messages(&source);
        assert!(
            diagnostics.contains("argument"),
            "{arguments:?}: {diagnostics}"
        );
    }
}

#[test]
fn embedded_calls_cannot_enter_another_machines_nested_state() {
    let source = r#"
        machine source(value: u8) -> u8 terminates; {
            transition { _ -> value }
            state inner(next: u8) { next }
        }
        machine law(value: u8) -> u8
        requires embed(source::inner(value)) >= 0
        { value }
    "#;
    let diagnostics = messages(source);
    assert!(
        diagnostics.contains("inner") || diagnostics.contains("denotational"),
        "{diagnostics}"
    );
}

#[test]
fn embedded_call_arguments_require_exact_types_and_literal_ranges() {
    for argument in ["flag", "true", "value == value", "wide", "256"] {
        let source = format!(
            "machine identity(value: u8) -> u8 terminates; {{ value }} machine law(value: u8, flag: bool, wide: u64) -> u8 requires embed(identity({argument})) >= 0 {{ value }}"
        );
        let diagnostics = messages(&source);
        assert!(
            diagnostics.contains("argument"),
            "{argument}: {diagnostics}"
        );
    }
    check(
        "machine identity(value: u8) -> u8 terminates; { value } machine law(value: u8) -> u8 requires embed(identity(255)) >= 0 { value }",
    ).expect("in-range literal establishes the exact parameter carrier");
}

#[test]
fn embedding_crashing_and_nonterminating_calls_rejects() {
    let crashing = messages(
        r#"
        machine source(value: u8) -> u8 crashes Abort terminates; { crash Abort; }
        machine law(value: u8) -> u8 requires embed(source(value)) >= 0 { value }
    "#,
    );
    assert!(crashing.contains("has a crash route"), "{crashing}");
    let looping = messages(
        r#"
        machine source(value: u8) -> u8 { transition { _ -> source(value) } }
        machine law(value: u8) -> u8 requires embed(source(value)) >= 0 { value }
    "#,
    );
    assert!(
        looping.contains("not unconditionally terminating")
            || looping.contains("call cycle")
            || looping.contains("recursive"),
        "{looping}"
    );
}

#[test]
fn computed_proof_body_embedding_still_checks_nested_arithmetic_totality() {
    let diagnostics = messages(
        r#"
        machine computation(value: i32 in Trapping) -> Int { embed(value + 1) }
    "#,
    );
    assert!(diagnostics.contains("Trapping arithmetic"), "{diagnostics}");
}

#[test]
fn natural_coercion_rejects_a_same_spelled_different_recursive_carrier() {
    let diagnostics = messages(
        r#"
        data Nat { case Empty; case Pair(left: Nat, right: Nat); }
        machine computation(value: u8) -> Nat { embed(value) as Nat }
    "#,
    );
    assert!(diagnostics.contains("not a scalar"), "{diagnostics}");
}

#[test]
fn natural_coercion_requires_the_toolchain_owner_when_sources_are_known() {
    let source = r#"
        data Nat { case Zero; case Succ(previous: Nat); }
        machine computation(value: u8) -> Nat { embed(value) as Nat }
    "#;
    for (origin, accepted) in [
        (source::SourceOrigin::Toolchain, true),
        (source::SourceOrigin::User, false),
    ] {
        let mut sources = source::SourceMap::default();
        let source_id = sources
            .add_with_metadata(
                std::path::PathBuf::from("nat.omg"),
                source.to_owned(),
                std::path::PathBuf::from("."),
                None,
                origin,
            )
            .source_id;
        let tokens = Lexer::new(source).tokenize().unwrap();
        let syntax =
            tokens_to_syntax_trees::parse_syntax_trees_with_id(source_id, &tokens).unwrap();
        let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees_with_sources(
            &syntax,
            Arc::new(sources),
        )
        .unwrap();
        let typed = lower_symbol_resolved_trees(&resolved).unwrap();
        let checked = lower_typed_trees(typed);
        assert_eq!(checked.is_ok(), accepted, "{origin:?}: {checked:?}");
    }
}
