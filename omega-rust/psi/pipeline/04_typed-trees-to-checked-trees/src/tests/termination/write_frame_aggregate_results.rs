use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::front_end::typed_program;
use typed_trees::statement::StatementNode;

fn aggregate_result_program(body: &str, extra: &str, scalar: &str) -> typed_trees::TypedTrees {
    let source = format!(
        r#"
        data Cell {{ value: {scalar}; }}
        data View {{ body: &mut {scalar}; tag: u64; }}
        data OtherView {{ body: &mut {scalar}; tag: u64; }}
        data Outer {{ inner: View; }}
        data Choice {{ case Selected(view: View); case Empty; }}
        data Main {{ value: {scalar}; other: {scalar}; audit: {scalar}; cells: [{scalar}; 2]; }}
        machine make_view(value: &mut {scalar}) -> View {{ View {{ body: value }} }}
        machine make_local(value: &mut {scalar}) -> View {{ let result: View = View {{ body: value }}; result }}
        machine make_moved(value: &mut {scalar}) -> View {{ let first: View = View {{ body: value }}; let result: View = first; result }}
        machine make_nested(value: &mut {scalar}) -> View {{ make_view(value) }}
        machine make_reused(value: &mut {scalar}) -> View {{ write_view(View {{ body: value }}); View {{ body: value }} }}
        machine make_transition(value: &mut {scalar}) -> View {{ let result: View = View {{ body: value }}; transition {{ _ -> result }} }}
        machine forward_view(value: View) -> View {{ value }}
        machine make_outer(value: &mut {scalar}) -> Outer {{ Outer {{ inner: make_view(value) }} }}
        machine make_choice(value: &mut {scalar}) -> Choice {{ Choice::Selected {{ view: View {{ body: value }} }} }}
        machine make_empty() -> Choice {{ Choice::Empty {{}} }}
        machine make_array(values: &mut [{scalar}; 2]) -> [View; 2] {{ [View {{ body: &mut values[0] }}, View {{ body: &mut values[1] }}] }}
        machine foreign_view(value: &mut {scalar}) -> OtherView {{ OtherView {{ body: value }} }}
        machine write_view(mut view: View) {{ view.body = 255; }}
        machine write_outer(mut outer: Outer) {{ outer.inner.body = 255; }}
        machine write_choice(mut choice: Choice) {{ choice.view.body = 255; }}
        machine write_array(mut values: [View; 2]) {{ values[0].body = 255; }}
        machine Main::run(&mut self) {{ {body} }}
        {extra}
    "#
    );
    typed_program(&source)
}

#[test]
fn aggregate_helper_results_transport_complete_reference_origins() {
    let cases = [
        (
            "stored_result",
            "let local: View = make_view(&mut self.value); write_view(local);",
            "",
            Some(vec!["self.value"]),
        ),
        (
            "direct_result",
            "write_view(make_view(&mut self.value));",
            "",
            Some(vec!["self.value"]),
        ),
        (
            "finite_consumer_reuse_in_producer",
            "write_view(make_reused(&mut self.value));",
            "",
            Some(vec!["self.value"]),
        ),
        (
            "local_result",
            "let local: View = make_local(&mut self.value); write_view(local);",
            "",
            Some(vec!["self.value"]),
        ),
        (
            "moved_result",
            "let local: View = make_moved(&mut self.value); write_view(local);",
            "",
            Some(vec!["self.value"]),
        ),
        (
            "nested_result",
            "let local: View = make_nested(&mut self.value); write_view(local);",
            "",
            Some(vec!["self.value"]),
        ),
        (
            "owned_carrier_formal_result",
            "let first: View = make_view(&mut self.value); let local: View = forward_view(first); write_view(local);",
            "",
            Some(vec!["self.value"]),
        ),
        (
            "terminal_value_transition",
            "let local: View = make_transition(&mut self.value); write_view(local);",
            "",
            Some(vec!["self.value"]),
        ),
        (
            "result_moved_again",
            "let first: View = make_view(&mut self.value); let local: View = first; write_view(local);",
            "",
            Some(vec!["self.value"]),
        ),
        (
            "nested_literal",
            "let local: Outer = Outer { inner: make_view(&mut self.value) }; write_outer(local);",
            "",
            Some(vec!["self.value"]),
        ),
        (
            "immediate_literal",
            "write_outer(Outer { inner: make_view(&mut self.value) });",
            "",
            Some(vec!["self.value"]),
        ),
        (
            "outer_result",
            "let local: Outer = make_outer(&mut self.value); write_outer(local);",
            "",
            Some(vec!["self.value"]),
        ),
        (
            "outer_projection",
            "let first: Outer = make_outer(&mut self.value); let local: View = first.inner; write_view(local);",
            "",
            Some(vec!["self.value"]),
        ),
        (
            "selected_result",
            "let local: Choice = make_choice(&mut self.value); write_choice(local);",
            "",
            Some(vec!["self.value"]),
        ),
        (
            "selected_projection",
            "let first: Choice = make_choice(&mut self.value); let local: View = first.view; write_view(local);",
            "",
            Some(vec!["self.value"]),
        ),
        (
            "empty_result",
            "let local: Choice = make_empty(); write_choice(local);",
            "",
            Some(vec![]),
        ),
        (
            "array_result",
            "let local: [View; 2] = make_array(&mut self.cells); write_array(local);",
            "",
            Some(vec!["self.cells"]),
        ),
        (
            "array_projection",
            "let first: [View; 2] = make_array(&mut self.cells); let local: View = first[0]; write_view(local);",
            "",
            Some(vec!["self.cells"]),
        ),
        (
            "frozen_caller_alias",
            "let mut alias: &mut u64 = &mut self.value; let local: View = make_view(alias); alias = &mut self.other; write_view(local);",
            "",
            Some(vec!["self.value"]),
        ),
        (
            "frozen_helper_alias",
            "let local: View = make_frozen(&mut self.value, &mut self.other); write_view(local);",
            "machine make_frozen(first: &mut u64, second: &mut u64) -> View { let mut alias: &mut u64 = first; let held: View = View { body: alias }; alias = second; held }",
            Some(vec!["self.value"]),
        ),
        (
            "missing_producer",
            "let local: View = absent(&mut self.value); write_view(local);",
            "",
            None,
        ),
        (
            "private_owned_parameter",
            "let local: View = private_owned(Cell { value: 0 }); write_view(local);",
            "machine private_owned(mut cell: Cell) -> View { View { body: &mut cell.value } }",
            None,
        ),
        (
            "mixed_array_private_parameter_field",
            "let local: View = mixed_private([Mixed { body: &mut self.value, value: 0 }, Mixed { body: &mut self.other, value: 0 }]); write_view(local);",
            "data Mixed { body: &mut u64; value: u64; } machine mixed_private(mut values: [Mixed; 2]) -> View { View { body: &mut values[0].value } }",
            None,
        ),
        (
            "private_local",
            "let local: View = private_local(); write_view(local);",
            "machine private_local() -> View { let mut scratch: u64 = 0; View { body: &mut scratch } }",
            None,
        ),
        (
            "recursive_result",
            "let local: View = recursive_view(&mut self.value); write_view(local);",
            "machine recursive_view(value: &mut u64) -> View { recursive_view(value) }",
            None,
        ),
        (
            // The consumed result carries only the replacement's origins;
            // the state frame additionally claims the overwritten leaf.
            "reference_slot_replacement",
            "let local: View = replace_slot(&mut self.value, &mut self.other); write_view(local);",
            "machine replace_slot(value: &mut u64, other: &mut u64) -> View { let mut local: View = View { body: value }; local.body = other; local }",
            Some(vec!["self.other"]),
        ),
        (
            // The consumed result carries only the replacement's origins;
            // the state frame additionally claims the overwritten leaf.
            "whole_carrier_replacement",
            "let local: View = replace_carrier(&mut self.value, &mut self.other); write_view(local);",
            "machine replace_carrier(value: &mut u64, other: &mut u64) -> View { let mut local: View = View { body: value }; local = View { body: other }; local }",
            Some(vec!["self.other"]),
        ),
        (
            "wrong_result_nominal",
            "let local: View = wrong_view(&mut self.value); write_view(local);",
            "machine wrong_view(value: &mut u64) -> View { OtherView { body: value } }",
            None,
        ),
        (
            "empty_payload_projection",
            "let first: Choice = make_empty(); let local: View = first.view; write_view(local);",
            "",
            None,
        ),
        (
            "terminal_reference_binding_exposure",
            "let local: View = terminal_slot(&mut self.value); write_view(local);",
            "machine touch_slot(slot: &mut u64) -> u64 { 0 } machine terminal_slot(mut value: &mut u64) -> View { View { body: value, tag: touch_slot(&mut value) } }",
            None,
        ),
        (
            "prefix_reference_binding_exposure",
            "let local: View = prefix_slot(&mut self.value); write_view(local);",
            "machine touch_slot(slot: &mut u64) -> u64 { 0 } machine prefix_slot(mut value: &mut u64) -> View { let ignored: u64 = touch_slot(&mut value); View { body: value } }",
            None,
        ),
        (
            "named_state_result",
            "let local: View = routed_view(&mut self.value); write_view(local);",
            "machine routed_view(value: &mut u64) -> View { transition { _ -> finish(value) } state finish(source: &mut u64) { View { body: source } } }",
            Some(vec!["self.value"]),
        ),
        (
            "named_state_chained",
            "let local: View = routed_twice(&mut self.value); write_view(local);",
            "machine routed_twice(value: &mut u64) -> View { transition { _ -> middle(value) } state middle(input: &mut u64) { transition { _ -> finish(input) } } state finish(source: &mut u64) { View { body: source } } }",
            Some(vec!["self.value"]),
        ),
        (
            // The named edge's authored arguments select which input carries
            // the returned leaf; identity substitution is not assumed.
            "named_state_argument_redirect",
            "let local: View = routed_swap(&mut self.value, &mut self.other); write_view(local);",
            "machine routed_swap(first: &mut u64, second: &mut u64) -> View { transition { _ -> finish(second) } state finish(source: &mut u64) { View { body: source } } }",
            Some(vec!["self.other"]),
        ),
        (
            // Distinct routes union: the write may reach either carried leaf.
            "named_state_split",
            "let local: View = routed_split(&mut self.value, &mut self.other, self.audit); write_view(local);",
            "machine routed_split(value: &mut u64, other: &mut u64, pick: u64) -> View { transition pick > 0 { true -> finish(value) false -> View { body: other } } state finish(source: &mut u64) { View { body: source } } }",
            Some(vec!["self.other", "self.value"]),
        ),
        (
            "named_state_cycle",
            "let local: View = cyclic_view(&mut self.value); write_view(local);",
            "machine cyclic_view(value: &mut u64) -> View { transition { _ -> finish(value) } state finish(source: &mut u64) { transition { _ -> finish(source) } } }",
            None,
        ),
        (
            "named_state_private",
            "let local: View = routed_private(&mut self.value); write_view(local);",
            "machine routed_private(value: &mut u64) -> View { transition { _ -> finish(value) } state finish(source: &mut u64) { let mut scratch: u64 = 0; View { body: &mut scratch } } }",
            None,
        ),
    ];
    let mut failures = Vec::new();
    for (name, body, extra, expected) in cases {
        let program = aggregate_result_program(body, extra, "u64");
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Main::run")
            .expect("caller");
        let state = &program.machine_states(machine)[0];
        let StatementNode::Call(call) = program
            .statement_table
            .statements(state.statement_nodes)
            .last()
            .expect("call")
        else {
            panic!("write demand");
        };
        let resolver = validation::CallFrameResolver::new(&program).expect("resolver");
        for (query, paths) in [
            (
                "state",
                resolver
                    .inferred_state_write_frame(machine, state)
                    .into_complete_paths(),
            ),
            ("public call", resolver.may_write_paths(machine, call)),
        ] {
            let actual = paths.map(|paths| {
                let mut paths: Vec<_> = paths
                    .into_iter()
                    .filter(|path| path == "self" || path.starts_with("self."))
                    .collect();
                paths.sort();
                paths.dedup();
                paths
            });
            let expected = expected.as_ref().map(|paths| {
                let mut paths: Vec<String> = paths.iter().map(|path| (*path).to_owned()).collect();
                if matches!(
                    name,
                    "whole_carrier_replacement" | "reference_slot_replacement"
                ) && query == "state"
                {
                    paths.push("self.value".to_owned());
                    paths.sort();
                }
                paths
            });
            if actual != expected {
                failures.push(format!(
                    "{name} {query}: expected {expected:?}, got {actual:?}"
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{failures:?}");
}

#[test]
fn aggregate_helper_result_producer_writes_do_not_become_later_call_writes() {
    let source = r#"
        data View<'source> { body: &'source mut u64; }
        machine make_audited<'value, 'audit>(value: &'value mut u64, audit: &'audit mut u64) -> View<'value> {
            audit = 1;
            View { body: value }
        }
        machine write_view<'source>(mut view: View<'source>) { view.body = 255; }
        machine exercise<'value, 'audit>(value: &'value mut u64, audit: &'audit mut u64) {
            let local: View<'value> = make_audited(value, audit);
            write_view(local);
        }
    "#;
    let program = typed_program(source);
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "exercise")
        .expect("caller");
    let state = &program.machine_states(machine)[0];
    let StatementNode::Call(call) = program
        .statement_table
        .statements(state.statement_nodes)
        .last()
        .expect("call")
    else {
        panic!("call");
    };
    let resolver = validation::CallFrameResolver::new(&program).expect("resolver");
    for (query, paths, expected) in [
        (
            "state",
            resolver
                .inferred_state_write_frame(machine, state)
                .into_complete_paths(),
            vec!["$P0", "$P1"],
        ),
        (
            "later call",
            resolver.may_write_paths(machine, call),
            vec!["value"],
        ),
    ] {
        let mut paths: Vec<_> = paths
            .expect("complete frame")
            .into_iter()
            .filter(|path| path.starts_with("$P") || path == "value" || path == "audit")
            .collect();
        paths.sort();
        paths.dedup();
        assert_eq!(paths, expected, "{query}");
    }
    lower_typed_trees(program, &CheckingRequest::settled())
        .expect("audited result has an explicit retained input lifetime");
}

#[test]
fn aggregate_helper_results_reach_checked_trees() {
    for body in [
        "write_view(make_view(&mut self.value));",
        "write_view(make_reused(&mut self.value));",
        "let local: View = make_view(&mut self.value); write_view(local);",
        "let local: View = make_local(&mut self.value); write_view(local);",
        "let local: View = make_moved(&mut self.value); write_view(local);",
        "let local: View = make_nested(&mut self.value); write_view(local);",
        "let local: View = make_transition(&mut self.value); write_view(local);",
        "let local: Outer = Outer { inner: make_view(&mut self.value) }; write_outer(local);",
    ] {
        lower_typed_trees(
            aggregate_result_program(body, "", "u64"),
            &CheckingRequest::settled(),
        )
        .expect("aggregate result reaches checked trees");
    }
    lower_typed_trees(aggregate_result_program(
        "let local: View = routed_view(&mut self.value); write_view(local);",
        "machine routed_view(value: &mut u64) -> View { transition { _ -> finish(value) } state finish(source: &mut u64) { View { body: source } } }",
        "u64",
    ), &CheckingRequest::settled())
    .expect("named-state aggregate result reaches checked trees");
}

#[test]
fn aggregate_helper_result_from_exclusive_self_retains_owned_field_origin() {
    let program = aggregate_result_program(
        "",
        r#"
        data Holder { cell: Cell; }
        machine Cell::make_view(&mut self) -> View { View { body: &mut self.value } }
        machine Holder::run(&mut self) {
            let local: View = self.cell.make_view();
            write_view(local);
        }
    "#,
        "u64",
    );
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Holder::run")
        .expect("caller");
    let state = &program.machine_states(machine)[0];
    let StatementNode::Call(call) = program
        .statement_table
        .statements(state.statement_nodes)
        .last()
        .expect("call")
    else {
        panic!("call");
    };
    let resolver = validation::CallFrameResolver::new(&program).expect("resolver");
    for paths in [
        resolver
            .inferred_state_write_frame(machine, state)
            .into_complete_paths(),
        resolver.may_write_paths(machine, call),
    ] {
        let paths: Vec<_> = paths
            .expect("attached result origin")
            .into_iter()
            .filter(|path| path == "self" || path.starts_with("self."))
            .collect();
        assert_eq!(paths, vec!["self.cell.value"]);
    }
    lower_typed_trees(program, &CheckingRequest::settled())
        .expect("attached aggregate result reaches checked trees");
}
