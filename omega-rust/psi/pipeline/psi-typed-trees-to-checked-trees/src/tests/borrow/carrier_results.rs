use super::checks::check_program;

fn single_source(initializer: &str, body: &str, explicit: bool) -> String {
    let (binder, applied, reference) = if explicit {
        ("<'source>", "View<'source>", "&'source mut i32")
    } else {
        ("", "View", "&mut i32")
    };
    format!(
        r#"
        data View{binder} {{ body: {reference}; }}
        machine forward{binder}(value: {applied}) -> {applied} {{ value }}
        machine make_view{binder}(value: {reference}) -> {applied} {{ View {{ body: value }} }}
        machine write(value: &mut i32) {{ value = 1; }}
        machine exercise{binder}(source: {reference}, other: &mut i32) {{
            let input: {applied} = View {{ body: source }};
            let held: {applied} = {initializer};
            {body}
        }}
    "#
    )
}

#[test]
fn carrier_result_retains_single_explicit_or_elided_source() {
    for explicit in [false, true] {
        for initializer in [
            "forward(input)",
            "forward(View { body: source })",
            "forward(make_view(source))",
            "forward(forward(input))",
        ] {
            let positive = single_source(initializer, "write(other); write(held.body);", explicit);
            check_program(&positive).unwrap_or_else(|diagnostics| {
                panic!("{initializer}, explicit={explicit}: {diagnostics:#?}")
            });
            let hostile = single_source(initializer, "write(source); write(held.body);", explicit);
            let diagnostics =
                check_program(&hostile).expect_err("returned loan must remain active");
            assert!(
                diagnostics.iter().any(|diagnostic| {
                    diagnostic
                        .message
                        .contains("while local borrow `held` is still active")
                }),
                "{initializer}, explicit={explicit}: {diagnostics:#?}"
            );
        }
    }
}

fn differently_named_source(body: &str) -> String {
    format!(
        r#"
        data Input<'left, 'right> {{ first: &'left mut i32; second: &'right mut i32; }}
        data Output<'left, 'right> {{ first: &'right mut i32; second: &'left mut i32; }}
        machine swap<'left, 'right>(value: Input<'left, 'right>) -> Output<'left, 'right> {{
            Output {{ first: value.second, second: value.first }}
        }}
        machine write(value: &mut i32) {{ value = 1; }}
        machine exercise<'left, 'right>(left: &'left mut i32, right: &'right mut i32) {{
            let input: Input<'left, 'right> = Input {{ first: left, second: right }};
            let held: Output<'left, 'right> = swap(input);
            {body}
        }}
    "#
    )
}

#[test]
fn carrier_result_lifetime_mapping_does_not_match_output_field_names() {
    check_program(&differently_named_source("write(left); write(held.first);"))
        .expect("first output borrows the right input despite matching field names");
    let diagnostics = check_program(&differently_named_source(
        "write(right); write(held.first);",
    ))
    .expect_err("right source is still borrowed");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("while local borrow `held` is still active")
        }),
        "{diagnostics:#?}"
    );
}

#[test]
fn carrier_result_same_lifetime_leaves_are_unioned_within_one_input() {
    for source in ["left", "right"] {
        let program = format!(
            r#"
            data Input<'source> {{ left: &'source mut i32; right: &'source mut i32; }}
            data Output<'source> {{ kept: &'source mut i32; }}
            machine select<'source>(input: Input<'source>) -> Output<'source> {{
                Output {{ kept: input.right }}
            }}
            machine write(value: &mut i32) {{ value = 1; }}
            machine exercise<'source>(left: &'source mut i32, right: &'source mut i32) {{
                let input: Input<'source> = Input {{ left: left, right: right }};
                let held: Output<'source> = select(input);
                write({source});
                write(held.kept);
            }}
        "#
        );
        let diagnostics =
            check_program(&program).expect_err("both possible lifetime sources retained");
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic
                    .message
                    .contains("while local borrow `held` is still active")
            }),
            "{source}: {diagnostics:#?}"
        );
    }
}

#[test]
fn carrier_result_ambiguous_inputs_and_access_escalation_reject() {
    let cases = [
        (
            r#"
            data View<'source> { body: &'source mut i32; }
            machine choose<'source>(first: View<'source>, second: View<'source>) -> View<'source> {
                first
            }
        "#,
            "shared by multiple inputs",
        ),
        (
            r#"
            data Pair { left: &mut i32; right: &mut i32; }
            data View { body: &mut i32; }
            machine choose(value: Pair) -> View { View { body: value.left } }
        "#,
            "candidate ref inputs",
        ),
        (
            r#"
            data ReadView<'source> { body: &'source i32; }
            data View<'source> { body: &'source mut i32; }
            machine promote<'source>(value: ReadView<'source>) -> View<'source> {
                View { body: value.body }
            }
        "#,
            "access cannot be supplied",
        ),
        (
            r#"
            data Mixed<'source> { read: &'source i32; write: &'source mut i32; }
            data View<'source> { body: &'source mut i32; }
            machine select<'source>(value: Mixed<'source>) -> View<'source> {
                View { body: value.write }
            }
        "#,
            "access cannot be supplied",
        ),
    ];
    for (source, expected) in cases {
        let diagnostics = check_program(source).expect_err(expected);
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "{expected}: {diagnostics:#?}"
        );
    }
}

#[test]
fn carrier_result_nested_array_sources_keep_paths_and_polarity() {
    let source = r#"
        data Input<'read, 'write> { read: &'read i32; write: &'write mut i32; }
        data Output<'read, 'write> { peek: &'read i32; poke: &'write mut i32; }
        data Outer<'read, 'write> { elements: [Input<'read, 'write>; 1]; }
        machine relay<'read, 'write>(value: Outer<'read, 'write>) -> Output<'read, 'write> {
            Output { peek: value.elements[0].read, poke: value.elements[0].write }
        }
        machine exercise<'read, 'write>(read: &'read i32, write: &'write mut i32) {
            let input: Outer<'read, 'write> = Outer {
                elements: [Input { read: read, write: write }]
            };
            let held: [Output<'read, 'write>; 1] = [relay(input)];
            held[0].poke = 1;
        }
    "#;
    check_program(source).expect("nested parameter paths and output array prefix preserve access");
    let typed = typed_program(source);
    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "exercise")
        .expect("exercise");
    let state = &typed.machine_states(machine)[0];
    let held = typed
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .find_map(|statement| match statement {
            psi_typed_trees::statement::StatementNode::LocalData(local)
                if local.name.as_str() == "held" =>
            {
                Some(local.symbol)
            }
            _ => None,
        })
        .expect("held");
    let facts = crate::build_borrow_facts(&typed);
    let loans = facts
        .loans
        .iter()
        .map(|(_, loan)| loan)
        .filter(|loan| loan.owner_symbol == held)
        .collect::<Vec<_>>();
    assert_eq!(loans.len(), 2);
    assert!(loans.iter().all(|loan| {
        matches!(
            facts.loan_owner_path(loan).first(),
            Some(psi_checked_trees::BorrowLoanOwnerSegment::FixedIndex(0))
        ) && facts.loan_owner_path(loan).len() == 2
    }));
    let parameters = typed.state_parameters(state);
    assert!(
        loans
            .iter()
            .any(|loan| loan.root_symbol == parameters[0].symbol
                && loan.kind == psi_checked_trees::BorrowAccessKind::Read)
    );
    assert!(
        loans
            .iter()
            .any(|loan| loan.root_symbol == parameters[1].symbol
                && loan.kind == psi_checked_trees::BorrowAccessKind::Mutable)
    );
}

fn typed_program(source: &str) -> psi_typed_trees::TypedTrees {
    let tokens = psi_source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize");
    let syntax = psi_tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
    let resolved =
        psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("resolve");
    psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("type")
}

#[test]
fn carrier_result_write_only_source_never_gains_read_access() {
    let source = r#"
        data View<'source> { body: &'source write i32; }
        machine forward<'source>(value: View<'source>) -> View<'source> { value }
        machine exercise<'source>(source: &'source mut i32) {
            let input: View<'source> = View { body: &write source };
            let held: View<'source> = forward(input);
        }
    "#;
    let typed = typed_program(source);
    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "exercise")
        .expect("exercise");
    let state = &typed.machine_states(machine)[0];
    let held = typed
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .find_map(|statement| match statement {
            psi_typed_trees::statement::StatementNode::LocalData(local)
                if local.name.as_str() == "held" =>
            {
                Some(local.symbol)
            }
            _ => None,
        })
        .expect("held");
    let facts = crate::build_borrow_facts(&typed);
    let loans = facts
        .loans
        .iter()
        .map(|(_, loan)| loan)
        .filter(|loan| loan.owner_symbol == held)
        .collect::<Vec<_>>();
    assert_eq!(loans.len(), 1);
    assert_eq!(
        loans[0].kind,
        psi_checked_trees::BorrowAccessKind::WriteOnly
    );
}

#[test]
fn carrier_result_incomplete_input_frontier_cannot_select_partial_sources() {
    use crate::borrow::view_link::{
        ViewReturnAmbiguity, ViewReturnSource, resolve_view_return_source,
    };
    use psi_typed_trees::types::{FixedArrayLength, TypeReferenceNode};
    let mut program = typed_program(
        r#"
        data View<'source> { body: &'source mut i32; }
        machine select<'source>(input: [View<'source>; 1]) -> View<'source> { input[0] }
    "#,
    );
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    let input = program.state_parameters(state)[0].type_reference;
    let TypeReferenceNode::FixedArray { element_type, .. } =
        program.type_reference_table.type_reference(input).clone()
    else {
        panic!("array input");
    };
    program.type_reference_table.substitute_node(
        input,
        TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::ConstParameter {
                symbol: Default::default(),
                name: "unknown".into(),
            },
        },
    );
    let state = &program.machine_states(&program.machines()[0])[0];
    assert!(matches!(
        resolve_view_return_source(&program, state),
        ViewReturnSource::Ambiguous(ViewReturnAmbiguity::IncompleteStructure { .. })
    ));
}
