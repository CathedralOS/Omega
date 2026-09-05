use super::*;

#[test]
fn aggregate_actual_reference_leaves_transport_complete_write_sets() {
    let cases = [
        (
            "local_reference",
            "let alias: &mut u64 = &mut self.value; write_view(View { body: alias, tag: 0 });",
            Some(vec!["self.value"]),
        ),
        (
            "selected_case",
            "write_choice(Choice::Reference { body: &mut self.value });",
            Some(vec!["self.value"]),
        ),
        (
            "direct",
            "write_view(View { body: &mut self.value, tag: 0 });",
            Some(vec!["self.value"]),
        ),
        (
            "helper",
            "write_view(View { body: identity(&mut self.value), tag: 0 });",
            Some(vec!["self.value"]),
        ),
        (
            "nested",
            "write_outer(Outer { inner: View { body: &mut self.value, tag: 0 } });",
            Some(vec!["self.value"]),
        ),
        (
            "owned_scalar",
            "write_tag(View { body: &mut self.value, tag: 0 });",
            Some(vec![]),
        ),
        (
            "mixed_writes",
            "write_both(View { body: &mut self.value, tag: 0 });",
            Some(vec!["self.value"]),
        ),
        (
            "array",
            "write_array([View { body: &mut self.value, tag: 0 }, View { body: &mut self.other, tag: 0 }]);",
            Some(vec!["self.other", "self.value"]),
        ),
        (
            "duplicate_array_origin",
            "write_array([View { body: &mut self.value, tag: 0 }, View { body: &mut self.value, tag: 0 }]);",
            Some(vec!["self.value"]),
        ),
        (
            "producer",
            "write_view(View { body: audited(&mut self.value, &mut self.audit), tag: 0 });",
            Some(vec!["self.audit", "self.value"]),
        ),
        ("missing_leaf", "write_view(View { tag: 0 });", None),
        (
            "replaced_reference_field",
            "rebind_view(View { body: &mut self.value, tag: 0 }, &mut self.other);",
            None,
        ),
        (
            "replaced_whole_carrier",
            "replace_view(View { body: &mut self.value, tag: 0 }, &mut self.other);",
            None,
        ),
        (
            "array_projected_reference_replacement",
            "replace_projected_view(View { body: &mut self.value, tag: 0 }, &mut self.other);",
            None,
        ),
        (
            "array_projected_borrow_replacement",
            "replace_projected_borrow(View { body: &mut self.value, tag: 0 }, &mut self.other);",
            None,
        ),
        (
            "unknown_leaf",
            "write_view(View { body: unknown(&mut self.value), tag: 0 });",
            None,
        ),
        (
            "shared_leaf",
            "write_view(View { body: &self.value, tag: 0 });",
            None,
        ),
        (
            "wrong_nominal",
            "write_view(OtherView { body: &mut self.value, tag: 0 });",
            None,
        ),
        (
            "wrong_length",
            "write_array([View { body: &mut self.value, tag: 0 }]);",
            None,
        ),
    ];
    let mut source = String::from(
        r#"
        data View { body: &mut u64; tag: u64; }
        data OtherView { body: &mut u64; tag: u64; }
        data Outer { inner: View; }
        data Choice { case Reference(body: &mut u64); case Empty; }
        data Main { value: u64; other: u64; audit: u64; }
        machine write_view(mut view: View) { view.body = 1; }
        machine write_outer(mut outer: Outer) { outer.inner.body = 1; }
        machine write_tag(mut view: View) { view.tag = 1; }
        machine write_both(mut view: View) { view.body = 1; view.tag = 1; }
        machine write_array(mut views: [View; 2]) { views[0].body = 1; }
        machine write_choice(mut choice: Choice) { choice.body = 1; }
        machine rebind_view(mut view: View, other: &mut u64) { view.body = other; view.body = 1; }
        machine replace_view(mut view: View, other: &mut u64) { view = View { body: other, tag: 0 }; view.body = 1; }
        machine replace_projected_view(mut view: View, other: &mut u64) { view.body = [other][0]; view.body = 1; }
        machine replace_projected_borrow(mut view: View, other: &mut u64) { view.body = [&mut other][0]; view.body = 1; }
        machine identity(value: &mut u64) -> &mut u64 { value }
        machine audited<'value, 'audit>(value: &'value mut u64, audit: &'audit mut u64) -> &'value mut u64 { audit = 1; value }
    "#,
    );
    for (name, body, _) in &cases {
        source.push_str(&format!(
            "machine Main::case_{name}(&mut self) {{ {body} }}"
        ));
    }
    let syntax =
        parse_syntax_trees(&Lexer::new(&source).tokenize().expect("tokenize")).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let resolver = validation::CallFrameResolver::new(&typed).expect("resolver");
    let mut failures = Vec::new();
    for (name, _, expected) in cases {
        let qualified = format!("Main::case_{name}");
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == qualified)
            .expect("caller");
        let state = &typed.machine_states(machine)[0];
        let statement = typed
            .statement_table
            .statements(state.statement_nodes)
            .last()
            .expect("call statement");
        let typed_trees::statement::StatementNode::Call(call) = statement else {
            panic!("call");
        };
        let direct = resolver.may_write_paths(machine, call);
        let operands = resolver.statement_value_may_write_paths(machine, statement);
        if name == "producer"
            && (direct.as_deref() != Some(["self.value".to_owned()].as_slice())
                || operands.as_deref() != Some(["self.audit".to_owned()].as_slice()))
        {
            failures.push(format!(
                "producer: direct {direct:?}; operands {operands:?}"
            ));
        }
        let combined = direct.zip(operands).map(|(mut paths, operands)| {
            paths.extend(operands);
            paths.sort();
            paths.dedup();
            paths
        });
        for (query, frame) in [
            resolver
                .inferred_state_write_frame(machine, state)
                .into_complete_paths(),
            combined,
        ]
        .into_iter()
        .enumerate()
        {
            let actual = frame.map(|mut paths| {
                paths.sort();
                paths
            });
            let expected = expected.as_ref().map(|paths| {
                let mut paths: Vec<_> = paths.iter().map(|path| (*path).to_owned()).collect();
                if name == "local_reference" && query == 1 {
                    paths.push("alias".to_owned());
                }
                paths.sort();
                paths
            });
            if actual != expected {
                failures.push(format!(
                    "{name} query {query}: expected {expected:?}, actual {actual:?}"
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{failures:?}");
}

#[test]
fn aggregate_literal_reference_argument_reaches_checked_trees() {
    let source = r#"
        data View { body: &mut u64; }
        data Main { value: u64; }
        machine write_view(mut view: View) { view.body = 1; }
        machine Main::run(&mut self) { write_view(View { body: &mut self.value }); }
    "#;
    let syntax =
        parse_syntax_trees(&Lexer::new(source).tokenize().expect("tokenize")).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed).expect("literal-carried reference reaches the checked callee");
}

#[test]
fn mutable_argument_bindings_do_not_grant_reference_access() {
    let cases = [
        (
            "owned_scalar",
            "machine take(mut value: u64) { value = 1; } machine Main::run(&mut self) { take(0); }",
            None,
        ),
        (
            "owned_record",
            "machine take(mut view: View) { view.body = 1; } machine Main::run(&mut self) { take(View { body: &mut self.value }); }",
            None,
        ),
        (
            "mutable_shared_formal",
            "machine take(mut value: &u64) {} machine Main::run(&mut self) { take(&self.value); }",
            None,
        ),
        (
            "exclusive_forward",
            "machine take(value: &mut u64) { value = 1; } machine forward(value: &mut u64) { take(value); }",
            None,
        ),
        (
            "shared_cannot_forward_exclusive",
            "machine take(value: &mut u64) {} machine forward(mut value: &u64) { take(value); }",
            Some("caller lends only immutable access"),
        ),
        (
            "owned_cannot_forward_exclusive",
            "machine take(value: &mut u64) {} machine forward(mut value: u64) { take(value); }",
            Some("caller lends only immutable access"),
        ),
        (
            "mutable_shared_local_cannot_forward_exclusive",
            "machine take(value: &mut u64) {} machine Main::run(&mut self) { let mut value: &u64 = &self.value; take(value); }",
            Some("caller lends only immutable access"),
        ),
        (
            "shared_actual_cannot_lend_exclusive",
            "machine take(value: &mut u64) {} machine Main::run(&mut self) { take(&self.value); }",
            Some("caller lends only immutable access"),
        ),
        (
            "exclusive_actual_can_lend_shared",
            "machine take(mut value: &u64) {} machine Main::run(&mut self) { take(&mut self.value); }",
            None,
        ),
        (
            "exclusive_reference_is_not_owned_value",
            "machine take(mut value: u64) {} machine Main::run(&mut self) { take(&mut self.value); }",
            Some("expects `u64`"),
        ),
    ];
    for (name, machines, expected_error) in cases {
        let source =
            format!("data View {{ body: &mut u64; }} data Main {{ value: u64; }} {machines}");
        let syntax =
            parse_syntax_trees(&Lexer::new(&source).tokenize().expect("tokenize")).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");
        match (lower_typed_trees(typed), expected_error) {
            (Ok(_), None) => {}
            (Err(diagnostics), Some(expected)) => assert!(
                format!("{diagnostics:?}").contains(expected),
                "{name}: expected {expected}: {diagnostics:?}"
            ),
            (Err(diagnostics), None) => panic!("{name}: {diagnostics:?}"),
            (Ok(_), Some(expected)) => panic!("{name}: accepted; expected {expected}"),
        }
    }
}
