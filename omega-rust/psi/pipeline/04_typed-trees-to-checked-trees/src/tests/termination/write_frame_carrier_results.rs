use crate::tests::front_end::typed_program;
use typed_trees::statement::StatementNode;

fn carrier_result_program(body: &str, extra: &str) -> typed_trees::TypedTrees {
    let source = format!(
        r#"
        data Cell {{ owned: u64; }}
        data View {{ body: &mut u64; }}
        data CellView {{ view: &mut Cell; }}
        data Outer {{ inner: View; }}
        data Choice {{ case Selected(view: View); case Empty; }}
        data Mixed {{ owned: u64; left: &mut Cell; right: &mut Cell; }}
        data MixedOuter {{ values: [Mixed; 1]; }}
        data Main {{ value: u64; other: u64; audit: u64; cell: Cell; second_cell: Cell; }}
        machine make_view(value: &mut u64) -> View {{ View {{ body: value }} }}
        machine identity(input: View) -> View {{ input }}
        machine identity_outer(input: Outer) -> Outer {{ input }}
        machine identity_choice(input: Choice) -> Choice {{ input }}
        machine identity_array(input: [View; 2]) -> [View; 2] {{ input }}
        machine project_outer(input: Outer) -> View {{ input.inner }}
        machine project_array(input: [View; 2]) -> View {{ input[0] }}
        machine wrap_outer(input: View) -> Outer {{ Outer {{ inner: input }} }}
        machine wrap_choice(input: View) -> Choice {{ Choice::Selected {{ view: input }} }}
        machine cell_value(input: CellView) -> View {{ View {{ body: &mut input.view.owned }} }}
        machine write_view(mut value: View) {{ value.body = 1; }}
        machine write_outer(mut value: Outer) {{ value.inner.body = 1; }}
        machine write_choice(mut value: Choice) {{ value.view.body = 1; }}
        machine write_array(mut values: [View; 2]) {{ values[0].body = 1; }}
        machine Main::run(&mut self) {{ {body} }}
        {extra}
        "#
    );
    typed_program(&source)
}

fn caller_frames(program: &typed_trees::TypedTrees) -> [Option<Vec<String>>; 2] {
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
        .expect("consumer")
    else {
        panic!("consumer");
    };
    let resolver = validation::CallFrameResolver::new(program).expect("resolver");
    [
        resolver
            .inferred_state_write_frame(machine, state)
            .into_complete_paths(),
        resolver.may_write_paths(machine, call),
    ]
    .map(|paths| {
        paths.map(|paths| {
            let mut visible = paths
                .into_iter()
                .filter(|path| path == "self" || path.starts_with("self."))
                .collect::<Vec<_>>();
            visible.sort();
            visible.dedup();
            visible
        })
    })
}

#[test]
fn owned_carrier_results_preserve_reference_origins_through_value_shapes() {
    let mut failures = Vec::new();
    for (name, body, extra, expected) in [
        (
            "literal_actual",
            "let result: View = identity(View { body: &mut self.value }); write_view(result);",
            "",
            vec!["self.value"],
        ),
        (
            "local_actual",
            "let input: View = View { body: &mut self.value }; let result: View = identity(input); write_view(result);",
            "",
            vec!["self.value"],
        ),
        (
            "helper_actual",
            "let result: View = identity(make_view(&mut self.value)); write_view(result);",
            "",
            vec!["self.value"],
        ),
        (
            "finite_repeated_helper",
            "let result: View = identity(identity(View { body: &mut self.value })); write_view(result);",
            "",
            vec!["self.value"],
        ),
        (
            "direct_consumer",
            "write_view(identity(View { body: &mut self.value }));",
            "",
            vec!["self.value"],
        ),
        (
            "nested_value_move",
            "let result: Outer = identity_outer(Outer { inner: View { body: &mut self.value } }); write_outer(result);",
            "",
            vec!["self.value"],
        ),
        (
            "selected_value_move",
            "let result: Choice = identity_choice(Choice::Selected { view: View { body: &mut self.value } }); write_choice(result);",
            "",
            vec!["self.value"],
        ),
        (
            "array_value_move",
            "let result: [View; 2] = identity_array([View { body: &mut self.value }, View { body: &mut self.other }]); write_array(result);",
            "",
            vec!["self.other", "self.value"],
        ),
        (
            "projected_owned_input",
            "let result: View = project_outer(Outer { inner: View { body: &mut self.value } }); write_view(result);",
            "",
            vec!["self.value"],
        ),
        (
            "projected_array_input",
            "let result: View = project_array([View { body: &mut self.value }, View { body: &mut self.other }]); write_view(result);",
            "",
            vec!["self.value"],
        ),
        (
            "returned_nested_literal",
            "let result: Outer = wrap_outer(View { body: &mut self.value }); write_outer(result);",
            "",
            vec!["self.value"],
        ),
        (
            "returned_selected_literal",
            "let result: Choice = wrap_choice(View { body: &mut self.value }); write_choice(result);",
            "",
            vec!["self.value"],
        ),
        (
            "reference_to_owned_field",
            "let result: View = cell_value(CellView { view: &mut self.cell }); write_view(result);",
            "",
            vec!["self.cell.owned"],
        ),
        (
            "copied_projected_local",
            "let result: View = copy_cell(CellView { view: &mut self.cell }); write_view(result);",
            "machine copy_cell(input: CellView) -> View { let copied: CellView = input; let alias: &mut u64 = &mut copied.view.owned; View { body: alias } }",
            vec!["self.cell.owned"],
        ),
        (
            "frozen_helper_alias",
            "let result: View = freeze_cell(CellView { view: &mut self.cell }, &mut self.other); write_view(result);",
            "machine freeze_cell(input: CellView, spare: &mut u64) -> View { let mut alias: &mut u64 = &mut input.view.owned; let held: View = View { body: alias }; alias = spare; held }",
            vec!["self.cell.owned"],
        ),
        (
            "frozen_caller_alias",
            "let mut alias: &mut u64 = &mut self.value; let input: View = View { body: alias }; alias = &mut self.other; let result: View = identity(input); write_view(result);",
            "",
            vec!["self.value"],
        ),
        (
            "stored_helper_result",
            "let result: View = store_cell(CellView { view: &mut self.cell }); write_view(result);",
            "machine store_cell(input: CellView) -> View { let first: View = cell_value(input); let copied: View = first; copied }",
            vec!["self.cell.owned"],
        ),
    ] {
        for (query, actual) in caller_frames(&carrier_result_program(body, extra))
            .into_iter()
            .enumerate()
        {
            let expected = Some(expected.iter().map(|path| (*path).to_owned()).collect());
            if actual != expected {
                failures.push(format!(
                    "{name} query {query}: expected {expected:?}, got {actual:?}"
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{failures:?}");
}

// A helper result routed through another helper's carrier leaf, a bound
// local, or a match-refined binding names the same declared leaf: the callee
// relation keeps the carrier parameter's root and the caller's actual
// instantiates the leaf through its own carrier.
#[test]
fn nested_and_bound_carrier_leaf_routes_retain_caller_origins() {
    let consume = "machine consume(value: &mut u64) { value = 1; }";
    for (name, helpers, body, expected) in [
        (
            "transitive_call_result",
            "machine pf(a: View) -> &mut u64 { a.body }\nmachine qf(a: View) -> &mut u64 { pf(a) }",
            "let view: View = View { body: &mut self.value }; let alias: &mut u64 = qf(view); alias = 1; consume(&mut self.audit);",
            vec!["self.audit", "self.value"],
        ),
        (
            "bound_transitive_result",
            "machine pf(a: View) -> &mut u64 { a.body }\nmachine qf(a: View) -> &mut u64 { let r: &mut u64 = pf(a); r }",
            "let view: View = View { body: &mut self.value }; let alias: &mut u64 = qf(view); alias = 1; consume(&mut self.audit);",
            vec!["self.audit", "self.value"],
        ),
        (
            "case_bound_leaf",
            "machine pf(a: View, tag: u64) -> &mut u64 { let pick: &mut u64 = match tag { 0 -> a.body, _ -> a.body }; pick }",
            "let view: View = View { body: &mut self.value }; let alias: &mut u64 = pf(view, self.value); alias = 1; consume(&mut self.audit);",
            vec!["self.audit", "self.value"],
        ),
        (
            "divergent_case_leaves_through_nested_call",
            "machine pf(a: View, b: View, tag: u64) -> &mut u64 { match tag { 0 -> a.body, _ -> b.body } }\nmachine qf(a: View, b: View, tag: u64) -> &mut u64 { pf(a, b, tag) }",
            "let view: View = View { body: &mut self.value }; let other: View = View { body: &mut self.other }; let alias: &mut u64 = qf(view, other, self.value); alias = 1; consume(&mut self.audit);",
            vec!["self.audit", "self.other", "self.value"],
        ),
    ] {
        let program = carrier_result_program(body, &format!("{consume}\n{helpers}"));
        let [state, call] = caller_frames(&program);
        assert_eq!(
            state,
            Some(expected.iter().map(|p| p.to_string()).collect()),
            "{name} state frame"
        );
        assert_eq!(
            call,
            Some(vec!["self.audit".to_owned()]),
            "{name} public answer"
        );
    }
}
