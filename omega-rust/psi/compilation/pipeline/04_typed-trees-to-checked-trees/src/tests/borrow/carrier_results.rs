use super::checks::check_program;
use crate::tests::front_end::typed_program;

#[test]
fn a_direct_reference_field_result_keeps_its_source_loan_active() {
    for (declaration, use_held) in [
        ("let held: &mut i32 = select(input);", "held"),
        ("let held: &mut i32 = relay(select(input));", "held"),
        (
            "let held: View = View { body: select(input) };",
            "held.body",
        ),
        (
            "let held: [View; 1] = [View { body: select(input) }];",
            "held[0].body",
        ),
        ("let held: &mut i32 = forward(input).body;", "held"),
        ("let held: View = make_view(select(input));", "held.body"),
        (
            "let held: &mut i32 = select(forward_outer(Outer { inner: input }).inner);",
            "held",
        ),
        (
            "let held: &mut i32 = select(forward_array([input])[0]);",
            "held",
        ),
    ] {
        for (operation, admitted) in [("write(other);", true), ("write(source);", false)] {
            let source = format!(
                "data View {{ body: &mut i32; }}
             data Outer {{ inner: View; }}
             machine forward_outer(value: Outer) -> Outer {{ value }}
             machine forward_array(value: [View; 1]) -> [View; 1] {{ value }}
             machine select(value: View) -> &mut i32 {{ value.body }}
             machine relay(value: &mut i32) -> &mut i32 {{ value }}
             machine forward(value: View) -> View {{ value }}
             machine make_view(value: &mut i32) -> View {{ View {{ body: value }} }}
             machine write(value: &mut i32) {{ value = 1; }}
             machine exercise(source: &mut i32, other: &mut i32) {{
                 let input: View = View {{ body: source }};
                 {declaration}
                 {operation}
                 write({use_held});
             }}"
            );
            if declaration.contains("select(forward_outer(")
                || declaration.contains("select(forward_array(")
            {
                let program = typed_program(&source);
                let machine = program
                    .machines()
                    .iter()
                    .find(|machine| machine.name.as_str() == "exercise")
                    .unwrap();
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
                    .unwrap();
                let facts = crate::borrow::build_borrow_facts(&program);
                let loans = facts
                    .loans
                    .iter()
                    .map(|(_, loan)| loan)
                    .filter(|loan| loan.owner_symbol == held)
                    .collect::<Vec<_>>();
                assert_eq!(loans.len(), 1, "{declaration}");
                assert_eq!(
                    loans[0].root_symbol,
                    program.state_parameters(state)[0].symbol
                );
            }
            if declaration == "let held: &mut i32 = select(input);"
                || declaration.contains("select(forward_outer(")
                || declaration.contains("select(forward_array(")
            {
                let checked_source =
                    format!("data Main {{}} machine Main::run(&mut self) {{}} {source}");
                let result = crate::lower_typed_trees(
                    typed_program(&checked_source),
                    &crate::CheckingRequest::settled(),
                );
                if declaration.contains("select(forward_outer(")
                    || declaration.contains("select(forward_array(")
                {
                    let diagnostics = result.expect_err(
                        "nested value-call realization remains independent of borrow compatibility",
                    );
                    assert!(
                        diagnostics.iter().any(|diagnostic| diagnostic
                            .message
                            .contains("a value-call argument cannot itself be a machine call yet")),
                        "{diagnostics:#?}"
                    );
                } else if admitted {
                    result.expect("raw owned-carrier reference return reaches checked trees");
                } else {
                    let diagnostics = result.expect_err("full checking retains the source loan");
                    assert!(
                        diagnostics.iter().any(|diagnostic| diagnostic
                            .message
                            .contains("while local borrow `held` is still active")),
                        "{diagnostics:#?}"
                    );
                }
            }
            if admitted {
                check_program(&source).expect("a disjoint source remains usable");
            } else {
                let diagnostics = check_program(&source)
                    .expect_err("forwarding the stored reference retains its source loan");
                assert!(
                    diagnostics.iter().any(|diagnostic| diagnostic
                        .message
                        .contains("while local borrow `held` is still active")),
                    "{diagnostics:#?}"
                );
            }
        }
    }
}

#[test]
fn carrier_result_ambiguous_inputs_and_access_escalation_reject() {
    // Every input carrying the result's explicit lifetime is a possible
    // source: a shared `'source` across two carriers is a union of sources,
    // not an ambiguity.
    check_program(
        r#"
        data View<'source> { body: &'source mut i32; }
        machine choose<'source>(first: View<'source>, second: View<'source>) -> View<'source> {
            first
        }
    "#,
    )
    .expect("every 'source input is a possible source of the returned view");
    let cases = [
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
            typed_trees::statement::StatementNode::LocalData(local)
                if local.name.as_str() == "held" =>
            {
                Some(local.symbol)
            }
            _ => None,
        })
        .expect("held");
    let facts = crate::borrow::build_borrow_facts(&typed);
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
            Some(checked_trees::BorrowLoanOwnerSegment::FixedIndex(0))
        ) && facts.loan_owner_path(loan).len() == 2
    }));
    let parameters = typed.state_parameters(state);
    assert!(
        loans
            .iter()
            .any(|loan| loan.root_symbol == parameters[0].symbol
                && loan.kind == checked_trees::BorrowAccessKind::Read)
    );
    assert!(
        loans
            .iter()
            .any(|loan| loan.root_symbol == parameters[1].symbol
                && loan.kind == checked_trees::BorrowAccessKind::Mutable)
    );
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
            typed_trees::statement::StatementNode::LocalData(local)
                if local.name.as_str() == "held" =>
            {
                Some(local.symbol)
            }
            _ => None,
        })
        .expect("held");
    let facts = crate::borrow::build_borrow_facts(&typed);
    let loans = facts
        .loans
        .iter()
        .map(|(_, loan)| loan)
        .filter(|loan| loan.owner_symbol == held)
        .collect::<Vec<_>>();
    assert_eq!(loans.len(), 1);
    assert_eq!(loans[0].kind, checked_trees::BorrowAccessKind::WriteOnly);
}

#[test]
fn carrier_result_incomplete_input_frontier_cannot_select_partial_sources() {
    use crate::borrow::view_link::{
        ViewReturnAmbiguity, ViewReturnSource, resolve_view_return_source,
    };
    use typed_trees::types::{FixedArrayLength, TypeReferenceNode};
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
