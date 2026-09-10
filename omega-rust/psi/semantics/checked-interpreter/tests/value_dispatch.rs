use checked_interpreter::interpret_entry;

fn execute(source: &str) -> checked_interpreter::InterpretOutcome {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("dispatch tokens");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("dispatch syntax");
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax)
        .expect("dispatch symbols");
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("dispatch types");
    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed)
        .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
    interpret_entry(&checked, "main", &[])
}

#[test]
fn value_dispatch_selects_only_first_matching_arm() {
    let outcome = execute("machine main() -> i64 { match 0i64 { 0 -> 7, 0 -> 99, _ -> 88 } }");
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 7);
}

#[test]
fn value_dispatch_does_not_execute_unselected_trapping_calls() {
    let outcome = execute(
        "machine divide(value: i64) -> i64 { 7 / value } machine main() -> i64 { match 0i64 { 0 -> 7, _ -> divide(0) } }",
    );
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 7);
}

#[test]
fn value_dispatch_evaluates_effectful_subject_once() {
    let outcome = execute(
        "machine read(calls: &mut i32 in Wrapping) -> i64 { calls = calls + 1; 2 } machine main() -> i32 { let mut calls: i32 in Wrapping = 0; let result: i64 = match read(&mut calls) { 0 -> 11, 1 -> 22, 2 -> 7, _ -> 44 }; transition result == 7 && calls == 1 { true -> 7 false -> 0 } }",
    );
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 7);
}

#[test]
fn value_dispatch_forwards_exact_integer_destination_to_selected_arm() {
    let outcome = execute("machine main() -> i32 { match true { true -> 7 / 2 * 2, false -> 0 } }");
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 7);
}
