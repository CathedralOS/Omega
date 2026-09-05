use super::checks::check_program;
use crate::build_borrow_facts;

#[test]
fn rejects_mutation_of_source_retained_by_aggregate_helper_call_leaf() {
    let source = r#"
        data View {
            body: &mut i32;
        }

        machine identity(value: &mut i32) -> &mut i32 {
            value
        }

        machine write(value: &mut i32) {
            value = 1;
        }

        machine exercise(source: &mut i32) {
            let held: View = View { body: identity(source) };
            write(source);
            write(held.body);
        }
    "#;

    let diagnostics = check_program(source)
        .expect_err("the aggregate leaf must retain the helper call's selected input loan");
    let combined = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("while local borrow `held` is still active"),
        "expected the retained aggregate loan conflict, got:\n{combined}"
    );
}

#[test]
fn accepts_unrelated_mutation_beside_aggregate_helper_call_leaf() {
    let source = r#"
        data View {
            body: &mut i32;
        }

        machine identity(value: &mut i32) -> &mut i32 {
            value
        }

        machine write(value: &mut i32) {
            value = 1;
        }

        machine exercise(first: &mut i32, second: &mut i32) {
            let held: View = View { body: identity(first) };
            write(second);
            write(held.body);
        }
    "#;

    check_program(source)
        .expect("the helper-linked aggregate loan must not capture an unrelated source");
}

#[test]
fn rejects_mutation_of_source_retained_by_nested_aggregate_call() {
    let source = nested_aggregate_source(
        "let held: Outer<'left, 'right> = Outer { pair: pair(left, right) };",
        "write(right); write(held.pair.right);",
    );

    let diagnostics = check_program(&source)
        .expect_err("a nested aggregate call result must retain its field-specific source loan");
    assert_borrow_conflict(&diagnostics, "right", "held");
}

#[test]
fn accepts_unrelated_mutation_beside_nested_aggregate_call_field() {
    let source = nested_aggregate_source(
        "let held: Outer<'left, 'right> = Outer { pair: pair(left, right) };",
        "write(left); write(held.pair.right);",
    );

    check_program(&source)
        .expect("using the nested right field must not retain the sibling left-field loan");
}

#[test]
fn rejects_mutation_of_source_retained_by_nested_aggregate_move() {
    let source = nested_aggregate_source(
        "let inner: Pair<'left, 'right> = pair(left, right);\n             let held: Outer<'left, 'right> = Outer { pair: inner };",
        "write(right); write(held.pair.right);",
    );

    let diagnostics = check_program(&source)
        .expect_err("moving an aggregate into a nested field must transfer its loans");
    assert_borrow_conflict(&diagnostics, "right", "held");
}

#[test]
fn rejects_mutation_of_source_retained_by_aggregate_call_in_array_element() {
    let source = nested_aggregate_source(
        "let held: [Pair<'left, 'right>; 1] = [pair(left, right)];",
        "write(right); write(held[0].right);",
    );

    let diagnostics = check_program(&source)
        .expect_err("an aggregate call in a fixed-array element must retain its exact loans");
    assert_borrow_conflict(&diagnostics, "right", "held");
}

#[test]
fn nested_aggregate_call_cast_preserves_field_paths_and_borrow_polarity() {
    let source = r#"
        data Pair<'left, 'right> {
            left: &'left i32;
            right: &'right mut i32;
        }

        data Outer<'left, 'right> {
            pair: Pair<'left, 'right>;
        }

        machine pair<'left, 'right>(
            left: &'left i32,
            right: &'right mut i32
        ) -> Pair<'left, 'right> {
            let result: Pair<'left, 'right> = Pair { left: left, right: right };
            transition { _ -> result }
        }

        machine exercise<'left, 'right>(
            left: &'left i32,
            right: &'right mut i32
        ) {
            let inner: Pair<'left, 'right> = pair(left, right);
            let held: Outer<'left, 'right> = Outer {
                pair: inner as Pair<'left, 'right>
            };
            let observed: &i32 = held.pair.left;
            held.pair.right = 1;
        }
    "#;

    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
    let resolved =
        syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("resolve");
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("type");
    let facts = build_borrow_facts(&typed);
    let nested = facts
        .loans
        .iter()
        .map(|(_, loan)| loan)
        .filter(|loan| facts.loan_owner_path(loan).len() == 2)
        .collect::<Vec<_>>();

    assert_eq!(
        nested.len(),
        2,
        "the outer field prefix must retain both pair fields"
    );
    assert!(
        nested
            .iter()
            .any(|loan| { loan.kind == checked_trees::BorrowAccessKind::Read })
    );
    assert!(
        nested
            .iter()
            .any(|loan| { loan.kind == checked_trees::BorrowAccessKind::Mutable })
    );
}

#[test]
fn rejects_source_mutation_through_root_same_carrier_aggregate_cast() {
    let source = aggregate_cast_source(
        "let inner: View = View { body: source };\n             let held: View = inner as View;",
        "write(source); write(held.body);",
    );

    let diagnostics = check_program(&source)
        .expect_err("a same-carrier aggregate cast must transfer the source loan");
    assert_borrow_conflict(&diagnostics, "source", "held");
}

#[test]
fn rejects_source_mutation_through_nested_same_carrier_aggregate_cast() {
    let source = aggregate_cast_source(
        "let inner: View = View { body: source };\n             let held: Outer = Outer { view: inner as View };",
        "write(source); write(held.view.body);",
    );

    let diagnostics = check_program(&source)
        .expect_err("a nested same-carrier aggregate cast must retain its prefixed loan");
    assert_borrow_conflict(&diagnostics, "source", "held");
}

#[test]
fn rejects_source_mutation_through_cast_aggregate_helper_result() {
    let source = aggregate_cast_source(
        "let inner: View = make_view(source);\n             let held: Outer = Outer { view: inner as View };",
        "write(source); write(held.view.body);",
    );

    let diagnostics = check_program(&source)
        .expect_err("a cast around a helper-produced aggregate must retain its selected input");
    assert_borrow_conflict(&diagnostics, "source", "held");
}

#[test]
fn rejects_source_mutation_through_cast_aggregate_literal() {
    let source = aggregate_cast_source(
        "let held: Outer = Outer { view: View { body: source } as View };",
        "write(source); write(held.view.body);",
    );

    let diagnostics = check_program(&source)
        .expect_err("a cast around an aggregate literal must retain its reference leaves");
    assert_borrow_conflict(&diagnostics, "source", "held");
}

#[test]
fn accepts_unrelated_source_mutation_beside_cast_aggregate_loan() {
    let source = aggregate_cast_source(
        "let inner: View = make_view(source);\n             let held: Outer = Outer { view: inner as View };",
        "write(other); write(held.view.body);",
    );

    check_program(&source).expect("a cast aggregate loan must not capture an unrelated source");
}

fn nested_aggregate_source(initializer: &str, body: &str) -> String {
    format!(
        r#"
        data Pair<'left, 'right> {{
            left: &'left mut i32;
            right: &'right mut i32;
        }}

        data Outer<'left, 'right> {{
            pair: Pair<'left, 'right>;
        }}

        machine pair<'left, 'right>(
            left: &'left mut i32,
            right: &'right mut i32
        ) -> Pair<'left, 'right> {{
            let result: Pair<'left, 'right> = Pair {{ left: left, right: right }};
            transition {{ _ -> result }}
        }}

        machine write(value: &mut i32) {{
            value = 1;
        }}

        machine exercise<'left, 'right>(
            left: &'left mut i32,
            right: &'right mut i32
        ) {{
            {initializer}
            {body}
        }}
        "#
    )
}

fn aggregate_cast_source(initializer: &str, body: &str) -> String {
    format!(
        r#"
        data View {{
            body: &mut i32;
        }}

        data Outer {{
            view: View;
        }}

        machine make_view(source: &mut i32) -> View {{
            let result: View = View {{ body: source }};
            transition {{ _ -> result }}
        }}

        machine write(value: &mut i32) {{
            value = 1;
        }}

        machine exercise(source: &mut i32, other: &mut i32) {{
            {initializer}
            {body}
        }}
        "#
    )
}

fn assert_borrow_conflict(diagnostics: &[diagnostics::Diagnostic], source: &str, owner: &str) {
    let combined = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains(&format!(
            "mutates `{source}` while local borrow `{owner}` is still active"
        )),
        "expected the nested aggregate loan conflict, got:\n{combined}"
    );
}
