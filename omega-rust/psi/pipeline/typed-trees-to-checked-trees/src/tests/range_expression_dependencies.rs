use super::*;

fn check(source: &str, accepted: bool) {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    match lower_typed_trees(typed) {
        Ok(_) => assert!(accepted, "stale computed bounds accepted: {source}"),
        Err(diagnostics) => {
            assert!(!accepted, "{diagnostics:#?}\n{source}");
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains("cannot prove")),
                "expected a range rejection: {diagnostics:#?}\n{source}"
            );
        }
    }
}

#[test]
fn unrelated_assignment_preserves_computed_endpoint_proofs() {
    check(
        r#"
        machine window(items: &[i32; 4], original: i64 [0..=5]) -> u64
            requires 0 <= original - 1 && original - 1 <= 4;
        {
            let mut unrelated: i64 = 0;
            unrelated = 1;
            let cut: i64 = original - 1;
            let view: &[i32] = items[0..cut];
            view.len
        }
    "#,
        true,
    );
}

fn typed_fixture(source: &str) -> typed_trees::TypedTrees {
    let tokens = Lexer::new(source)
        .tokenize()
        .unwrap_or_else(|diagnostics| panic!("tokenize: {diagnostics:#?}\n{source}"));
    let syntax = parse_syntax_trees(&tokens)
        .unwrap_or_else(|diagnostics| panic!("parse: {diagnostics:#?}\n{source}"));
    let resolved = lower_syntax_trees(&syntax)
        .unwrap_or_else(|diagnostics| panic!("resolve: {diagnostics:#?}\n{source}"));
    lower_symbol_resolved_trees(&resolved)
        .unwrap_or_else(|diagnostics| panic!("type: {diagnostics:#?}\n{source}"))
}

fn assert_range_rejection(diagnostics: &[diagnostics::Diagnostic], source: &str) {
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.message.contains("cannot prove subslice range") }),
        "expected subslice range rejection, not an earlier proof failure: {diagnostics:#?}\n{source}"
    );
}

fn check_range(source: &str, accepted: bool) {
    match lower_typed_trees(typed_fixture(source)) {
        Ok(_) => assert!(accepted, "stale computed bounds accepted:\n{source}"),
        Err(diagnostics) => {
            assert!(!accepted, "check: {diagnostics:#?}\n{source}");
            assert_range_rejection(&diagnostics, source);
        }
    }
}

fn arithmetic_source(body: &str, boundary: &str) -> String {
    format!(
        "machine set(output: &mut i64 [0..=5], replacement: i64 [0..=4]) {{
             output = replacement;
         }}
         machine inspect(value: &i64 [0..=5]) {{}}
         machine window(items: &[i32; 4], mut original: i64 [0..=5],
             replacement: i64 [0..=4]) -> u64
         requires 0 <= original - 1 && original - 1 <= 4; {{
             let mut unrelated: i64 [0..=5] = 0;
             {body}
             let view: &[i32] = items[0..{boundary}];
             view.len
         }}"
    )
}

#[test]
fn known_calls_preserve_computed_bounds_for_disjoint_and_readonly_storage() {
    for body in [
        "set(&mut unrelated, replacement);",
        "inspect(&original);",
        "let alias: &mut i64 [0..=5] = &mut unrelated; set(alias, replacement);",
    ] {
        for capture in [false, true] {
            let body = if capture {
                format!("{body} let cut: i64 = original - 1;")
            } else {
                body.to_owned()
            };
            check_range(
                &arithmetic_source(&body, if capture { "cut" } else { "original - 1" }),
                true,
            );
        }
    }
}

#[test]
fn operand_writes_retire_direct_uses_and_later_captures_but_keep_earlier_copies() {
    for mutation in [
        "original = replacement;",
        "set(&mut original, replacement);",
        "let alias: &mut i64 [0..=5] = &mut original; alias = replacement;",
        "let alias: &mut i64 [0..=5] = &mut original; set(alias, replacement);",
    ] {
        for (body, boundary, accepted) in [
            (mutation.to_owned(), "original - 1", false),
            (
                format!("{mutation} let cut: i64 = original - 1;"),
                "cut",
                false,
            ),
            (
                format!("let cut: i64 = original - 1; {mutation} let last: i64 = cut;"),
                "last",
                true,
            ),
        ] {
            check_range(&arithmetic_source(&body, boundary), accepted);
        }
    }
}

#[test]
fn both_binary_operands_and_nested_arithmetic_dependencies_are_tracked() {
    // These declared operand ranges discharge Exact overflow independently of
    // the computed nonnegativity premise that the mutation must invalidate.
    for expression in ["left - right", "(left + 1) - (right + 1)"] {
        for (mutation, accepted) in [
            ("", true),
            ("unrelated = replacement;", true),
            ("set(&mut unrelated, replacement);", true),
            ("left = replacement;", false),
            ("right = replacement;", false),
            ("set(&mut left, replacement);", false),
            ("set(&mut right, replacement);", false),
        ] {
            let source = format!(
                "machine set(output: &mut i64 [0..=5], replacement: i64 [0..=4]) {{
                     output = replacement;
                 }}
                 machine window(items: &[i32; 4], mut left: i64 [0..=5],
                     mut right: i64 [0..=5], replacement: i64 [0..=4]) -> u64
                 requires 0 <= {expression} && {expression} <= 4; {{
                     let mut unrelated: i64 [0..=5] = 0;
                     {mutation}
                     let cut: i64 = {expression};
                     let view: &[i32] = items[0..cut]; view.len
                 }}"
            );
            check_range(&source, accepted);
        }
    }
}

#[test]
fn builtin_unary_and_cast_operands_keep_their_storage_dependencies() {
    for (scalar, expression) in [
        ("i64 [0..=5]", "-(original - 1)"),
        ("i32 [0..=5]", "(original as i64) - 1"),
    ] {
        for (mutation, accepted) in [
            ("", true),
            ("unrelated = 1;", true),
            ("original = replacement;", false),
        ] {
            check_range(
                &format!(
                    "machine window(items: &[i32; 4], mut original: {scalar},
                         replacement: {scalar}) -> u64
                     requires 0 <= {expression} && {expression} <= 4; {{
                         let mut unrelated: i64 = 0;
                         {mutation}
                         let cut: i64 = {expression};
                         let view: &[i32] = items[0..cut]; view.len
                     }}"
                ),
                accepted,
            );
        }
    }
}

#[test]
fn receiver_field_dependencies_match_explicit_and_bare_write_paths() {
    for original in ["self.original", "original"] {
        for (mutation, accepted) in [
            ("", true),
            ("self.unrelated = replacement;", true),
            ("unrelated = replacement;", true),
            ("set(&mut self.unrelated, replacement);", true),
            ("self.original = replacement;", false),
            ("original = replacement;", false),
            ("set(&mut self.original, replacement);", false),
        ] {
            check_range(
                &format!(
                    "data Main {{ original: i64 [0..=5]; unrelated: i64 [0..=5]; }}
                     machine set(output: &mut i64 [0..=5], replacement: i64 [0..=4]) {{
                         output = replacement;
                     }}
                     machine Main::window(&mut self, items: &[i32; 4],
                         replacement: i64 [0..=4]) -> u64
                     requires 0 <= {original} - 1 && {original} - 1 <= 4; {{
                         {mutation}
                         let cut: i64 = {original} - 1;
                         let view: &[i32] = items[0..cut]; view.len
                     }}"
                ),
                accepted,
            );
        }
    }
}

#[test]
fn local_field_name_conflict_rejects_ambiguous_computed_boundary_source() {
    for (declaration, accepted) in [
        ("", true),
        ("let original: i64 [0..=5] = replacement;", false),
    ] {
        let source = format!(
            "data Main {{ original: i64 [0..=5]; }}
                 machine Main::window(&self, items: &[i32; 4],
                     replacement: i64 [0..=4]) -> u64
                 requires 0 <= original - 1 && original - 1 <= 4; {{
                     {declaration}
                     let mut unrelated: i64 = 0;
                     unrelated = 1;
                     let cut: i64 = original - 1;
                     let view: &[i32] = items[0..cut]; view.len
                 }}"
        );
        if accepted {
            check_range(&source, true);
        } else {
            // This source is rejected by the existing declaration fence before
            // range checking; it is not a computed-dependency identity test.
            let Err(diagnostics) = lower_typed_trees(typed_fixture(&source)) else {
                panic!("a local cannot shadow the attached field");
            };
            assert!(
                diagnostics.iter().any(|diagnostic| diagnostic
                    .message
                    .contains("local data `original` conflicts with an existing name")),
                "expected the declaration conflict: {diagnostics:#?}\n{source}"
            );
            assert!(
                diagnostics
                    .iter()
                    .all(|diagnostic| !diagnostic.message.contains("cannot prove")),
                "the declaration fence must reject before proof checking: {diagnostics:#?}"
            );
        }
    }
}

#[test]
fn guarded_true_and_false_routes_preserve_only_unwritten_computed_bounds() {
    for (guard, first, selected) in [
        ("original - 1 >= 0", "false", "true"),
        ("original - 1 < 0", "true", "false"),
    ] {
        for (prefix, accepted) in [
            ("0", true),
            ("set(&mut unrelated, replacement)", true),
            ("set(&mut original, replacement)", false),
        ] {
            // Chapter 5 schedules arguments left to right. Both the write and
            // the subslice access run under this arm's same-state guard facts.
            check_range(
                &format!(
                    "machine set(output: &mut i64 [0..=5], replacement: i64 [0..=4]) -> u64 {{
                         output = replacement; 0
                     }}
                     machine combine(ignored: u64, result: u64) -> u64 {{ result }}
                     machine window(items: &[i32; 4], mut original: i64 [0..=5],
                         replacement: i64 [0..=4]) -> u64
                     requires original - 1 <= 4; {{
                         let mut unrelated: i64 [0..=5] = 0;
                         transition {guard} {{
                             {first} -> 0
                             {selected} -> (combine({prefix}, items[0..original - 1].len))
                         }}
                     }}"
                ),
                accepted,
            );
        }
    }
}

#[test]
fn an_opaque_call_frame_retires_computed_bounds() {
    let source = "data Borrowed { value: &mut i64 [0..=5]; }
         machine observe(borrowed: Borrowed) {}
         machine window(items: &[i32; 4], mut original: i64 [0..=5]) -> u64
         requires 0 <= original - 1 && original - 1 <= 4; {
             observe(Borrowed { value: &mut original });
             let cut: i64 = original - 1;
             let view: &[i32] = items[0..cut]; view.len
         }";
    let mut checked = lower_typed_trees(typed_fixture(source))
        .unwrap_or_else(|diagnostics| panic!("known empty frame: {diagnostics:#?}\n{source}"));
    let machine = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "window")
        .expect("window");
    let statements = checked.typed.machine_states(machine)[0].statement_nodes;
    for statement in checked.typed.statement_table.statements_mut(statements) {
        if let typed_trees::statement::StatementNode::Call(call) = statement {
            call.target_symbol = symbols::SymbolHandle::invalid();
            call.target = typed_trees::name::Identifier::generated("unknown");
        }
    }
    let machine = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "window")
        .expect("window");
    let call = checked
        .typed
        .statement_table
        .statements(statements)
        .iter()
        .find_map(|statement| match statement {
            typed_trees::statement::StatementNode::Call(call) => Some(call),
            _ => None,
        })
        .expect("aggregate argument call");
    let frames = validation::CallFrameResolver::new(&checked.typed).expect("frame resolver");
    assert!(frames.may_write_paths(machine, call).is_none());
    let diagnostics =
        crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts)
            .expect_err("an opaque frame must retire the computed endpoint premise");
    assert_range_rejection(&diagnostics, source);
}
