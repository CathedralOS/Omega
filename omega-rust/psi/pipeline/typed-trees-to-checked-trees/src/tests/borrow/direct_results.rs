use super::checks::check_program;
use checked_trees::BorrowAccessKind;

fn rejects(source: &str, expected: &str) {
    let diagnostics = check_program(source).expect_err(expected);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains(expected)),
        "{expected}: {diagnostics:#?}"
    );
}

fn typed_program(source: &str) -> typed_trees::TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
    let resolved =
        syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("resolve");
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("type")
}

#[test]
fn direct_result_selects_the_named_lifetime_instead_of_field_order_or_spelling() {
    for (operation, admitted) in [("write(right);", true), ("write(left);", false)] {
        let source = format!(
            "data Input<'left, 'right> {{ first: &'right mut i32; second: &'left mut i32; }}
             machine select<'left, 'right>(value: Input<'left, 'right>) -> &'left mut i32 {{
                 value.second
             }}
             machine write(value: &mut i32) {{ value = 1; }}
             machine exercise<'left, 'right>(left: &'left mut i32, right: &'right mut i32) {{
                 let input: Input<'left, 'right> = Input {{ first: right, second: left }};
                 let held: &'left mut i32 = select(input);
                 {operation}
                 write(held);
             }}"
        );
        if admitted {
            check_program(&source).expect("the differently named unselected field is independent");
        } else {
            rejects(&source, "while local borrow `held` is still active");
        }
    }
}

#[test]
fn direct_result_keeps_every_candidate_leaf_with_the_same_lifetime() {
    for operation in ["write(left);", "write(right);", "write(other);"] {
        let source = format!(
            "data Input<'source> {{ first: &'source mut i32; second: &'source mut i32; }}
             machine select<'source>(value: Input<'source>) -> &'source mut i32 {{ value.second }}
             machine write(value: &mut i32) {{ value = 1; }}
             machine exercise<'source>(left: &'source mut i32, right: &'source mut i32, other: &mut i32) {{
                 let input: Input<'source> = Input {{ first: left, second: right }};
                 let held: &'source mut i32 = select(input);
                 {operation}
                 write(held);
             }}"
        );
        if operation == "write(other);" {
            check_program(&source).expect("an unrelated source is outside the candidate union");
        } else {
            rejects(&source, "while local borrow `held` is still active");
        }
    }
}

#[test]
fn direct_result_rejects_ambiguity_between_owned_and_direct_inputs() {
    for (binder, application, reference, expected) in [
        (
            "<'source>",
            "View<'source>",
            "&'source mut i32",
            "shared by multiple inputs",
        ),
        ("", "View", "&mut i32", "candidate ref inputs"),
    ] {
        let source = format!(
            "data View{binder} {{ body: {reference}; }}
             machine select{binder}(value: {application}, other: {reference}) -> {reference} {{
                 value.body
             }}"
        );
        rejects(&source, expected);
    }
}

#[test]
fn every_candidate_source_must_supply_the_direct_results_access() {
    for (source_access, result_access) in [
        ("", "mut "),
        ("", "write "),
        ("write ", ""),
        ("write ", "mut "),
    ] {
        // The body selects the mutable field. The same-lifetime restricted
        // sibling must still prevent promoting any possible input source.
        let source = format!(
            "data Input<'source> {{ restricted: &'source {source_access}i32; selected: &'source mut i32; }}
             machine select<'source>(value: Input<'source>) -> &'source {result_access}i32 {{
                 value.selected
             }}"
        );
        rejects(&source, "access cannot be supplied");
    }
}

#[test]
fn direct_result_loan_facts_preserve_read_and_write_only_access() {
    for (field_access, result_access, local_access, initializer, expected) in [
        ("", "", "", "source", BorrowAccessKind::Read),
        ("mut ", "", "", "source", BorrowAccessKind::Read),
        ("mut ", "mut ", "", "source", BorrowAccessKind::Read),
        (
            "write ",
            "write ",
            "write ",
            "&write source",
            BorrowAccessKind::WriteOnly,
        ),
    ] {
        // Inspect attribution independently of source-body admission for
        // write-only field reads. This does not claim the whole source checks.
        let source = format!(
            "data View<'source> {{ body: &'source {field_access}i32; }}
             machine select<'source>(value: View<'source>) -> &'source {result_access}i32 {{ value.body }}
             machine exercise<'source>(source: &'source mut i32) {{
                 let input: View<'source> = View {{ body: {initializer} }};
                 let held: &'source {local_access}i32 = select(input);
             }}"
        );
        let program = typed_program(&source);
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "exercise")
            .expect("exercise");
        let state = &program.machine_states(machine)[0];
        let held = program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .find_map(|statement| match statement {
                typed_trees::statement::StatementNode::LocalData(local)
                    if local.name.as_str() == "held" =>
                {
                    Some(local.symbol)
                }
                _ => None,
            })
            .expect("held");
        let facts = crate::build_borrow_facts(&program);
        let loans = facts
            .loans
            .iter()
            .map(|(_, loan)| loan)
            .filter(|loan| loan.owner_symbol == held)
            .collect::<Vec<_>>();
        assert_eq!(loans.len(), 1, "{source}");
        assert_eq!(
            loans[0].root_symbol,
            program.state_parameters(state)[0].symbol
        );
        assert_eq!(loans[0].kind, expected, "{source}");
        assert!(facts.loan_owner_path(loans[0]).is_empty());
    }
}
