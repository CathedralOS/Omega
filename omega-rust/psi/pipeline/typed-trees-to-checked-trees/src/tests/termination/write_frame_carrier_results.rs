use super::*;
use typed_trees::expression::ExpressionNode;
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
    let syntax =
        parse_syntax_trees(&Lexer::new(&source).tokenize().expect("tokenize")).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    lower_symbol_resolved_trees(&resolved).expect("type")
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

#[test]
fn owned_carrier_results_distinguish_reference_fields_after_indexing() {
    for (name, helper, selected_owner) in [
        (
            "left",
            "machine select(values: [Mixed; 1]) -> View { View { body: &mut values[0].left.owned } }",
            "self.cell",
        ),
        (
            "right",
            "machine select(values: [Mixed; 1]) -> View { View { body: &mut values[0].right.owned } }",
            "self.second_cell",
        ),
        (
            "left_alias",
            "machine select(values: [Mixed; 1]) -> View { let alias: &mut u64 = &mut values[0].left.owned; View { body: alias } }",
            "self.cell",
        ),
        (
            "right_stored",
            "machine select(values: [Mixed; 1]) -> View { let held: View = View { body: &mut values[0].right.owned }; held }",
            "self.second_cell",
        ),
        (
            "runtime_left",
            "machine select(values: [Mixed; 1]) -> View { let index: u64 = 0; View { body: &mut values[index].left.owned } }",
            "self.cell",
        ),
        (
            "runtime_right_alias",
            "machine select(values: [Mixed; 1]) -> View { let index: u64 = 0; let alias: &mut u64 = &mut values[index].right.owned; View { body: alias } }",
            "self.second_cell",
        ),
        (
            "copied_array_left",
            "machine select(values: [Mixed; 1]) -> View { let copied: [Mixed; 1] = values; View { body: &mut copied[0].left.owned } }",
            "self.cell",
        ),
        (
            "projected_record_right",
            "machine select(values: [Mixed; 1]) -> View { let copied: Mixed = values[0]; View { body: &mut copied.right.owned } }",
            "self.second_cell",
        ),
        (
            "nested_field_before_index",
            "machine select(values: [Mixed; 1]) -> View { let outer: MixedOuter = MixedOuter { values: values }; View { body: &mut outer.values[0].left.owned } }",
            "self.cell",
        ),
    ] {
        let body = "let result: View = select([Mixed { owned: 0, left: &mut self.cell, right: &mut self.second_cell }]); write_view(result);";
        for (query, paths) in caller_frames(&carrier_result_program(body, helper))
            .into_iter()
            .enumerate()
        {
            let paths = paths.unwrap_or_else(|| panic!("{name} query {query}: opaque"));
            assert!(!paths.is_empty(), "{name} query {query}: empty");
            assert!(
                paths.iter().all(
                    |path| path == selected_owner || path == &format!("{selected_owner}.owned")
                ),
                "{name} query {query}: wrong reference field {paths:?}"
            );
        }
    }
}

#[test]
fn owned_carrier_results_never_export_private_storage_beside_reference_leaves() {
    for (name, helper) in [
        (
            "direct",
            "machine select(values: [Mixed; 1]) -> View { View { body: &mut values[0].owned } }",
        ),
        (
            "alias",
            "machine select(values: [Mixed; 1]) -> View { let alias: &mut u64 = &mut values[0].owned; View { body: alias } }",
        ),
        (
            "stored",
            "machine select(values: [Mixed; 1]) -> View { let held: View = View { body: &mut values[0].owned }; held }",
        ),
        (
            "runtime_owned",
            "machine select(values: [Mixed; 1]) -> View { let index: u64 = 0; View { body: &mut values[index].owned } }",
        ),
        (
            "copied_array",
            "machine select(values: [Mixed; 1]) -> View { let copied: [Mixed; 1] = values; View { body: &mut copied[0].owned } }",
        ),
        (
            "copied_record",
            "machine select(values: [Mixed; 1]) -> View { let copied: Mixed = values[0]; View { body: &mut copied.owned } }",
        ),
        (
            "nested_helper",
            "machine private_value(values: [Mixed; 1]) -> View { View { body: &mut values[0].owned } } machine select(values: [Mixed; 1]) -> View { let held: View = private_value(values); held }",
        ),
        (
            "private_local",
            "machine select(values: [Mixed; 1]) -> View { let mut scratch: u64 = 0; View { body: &mut scratch } }",
        ),
    ] {
        let body = "let result: View = select([Mixed { owned: 0, left: &mut self.cell, right: &mut self.second_cell }]); write_view(result);";
        for (query, paths) in caller_frames(&carrier_result_program(body, helper))
            .into_iter()
            .enumerate()
        {
            assert_eq!(
                paths, None,
                "{name} query {query}: private source acquired a relation"
            );
        }
    }
}

#[test]
fn owned_carrier_result_producer_effects_remain_separate() {
    let program = carrier_result_program(
        "let result: View = audited(View { body: &mut self.value }, &mut self.audit); write_view(result);",
        "machine audited(input: View, audit: &mut u64) -> View { audit = 1; input }",
    );
    let [state, call] = caller_frames(&program);
    assert_eq!(
        state,
        Some(vec!["self.audit".to_owned(), "self.value".to_owned()])
    );
    assert_eq!(call, Some(vec!["self.value".to_owned()]));
}

#[test]
fn owned_carrier_result_source_requires_exact_parameter_identity() {
    let original = carrier_result_program(
        "let result: View = identity(View { body: &mut self.value }); write_view(result);",
        "machine foreign(input: View) -> View { input }",
    );
    let helper = original
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "identity")
        .expect("helper");
    let state = &original.machine_states(helper)[0];
    let StatementNode::Expression(expression) = original
        .statement_table
        .statements(state.statement_nodes)
        .last()
        .expect("result")
    else {
        panic!("result");
    };
    let expression = *expression;
    let exact = original.state_parameters(state)[0].symbol;
    let foreign = original
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "foreign")
        .expect("foreign");
    let foreign = original.state_parameters(&original.machine_states(foreign)[0])[0].symbol;
    for (name, symbol, complete) in [
        ("exact", exact, true),
        ("foreign", foreign, false),
        ("missing", symbols::SymbolHandle::invalid(), false),
        (
            "stale",
            symbols::SymbolHandle::from_parts(exact.arena_index(), exact.generation() + 1),
            false,
        ),
    ] {
        let mut program = original.clone();
        let ExpressionNode::Name(path) = program.expression_table.expression_mut(expression) else {
            panic!("source");
        };
        path.symbol = symbol;
        path.head_symbol = symbol;
        let member_symbols = path.member_symbols;
        program
            .expression_table
            .set_name_path_member_symbol_at_offset(member_symbols, 0, symbol);
        for (query, paths) in caller_frames(&program).into_iter().enumerate() {
            assert_eq!(paths.is_some(), complete, "{name} query {query}");
        }
    }
}

#[test]
fn copied_carrier_alias_results_preserve_runtime_candidates_and_frozen_sources() {
    let mut failures = Vec::new();
    for (name, body, helper, expected_owners) in [
        (
            "runtime_candidates",
            "let result: View = select([CellView { view: &mut self.cell }, CellView { view: &mut self.second_cell }]); write_view(result);",
            "machine select(values: [CellView; 2]) -> View { let copied: [CellView; 2] = values; let index: u64 = 0; let alias: &mut u64 = &mut copied[index].view.owned; View { body: alias } }",
            vec!["self.cell", "self.second_cell"],
        ),
        (
            "runtime_candidates_after_rebinding",
            "let result: View = select([CellView { view: &mut self.cell }, CellView { view: &mut self.second_cell }], &mut self.audit); write_view(result);",
            "machine select(values: [CellView; 2], spare: &mut u64) -> View { let copied: [CellView; 2] = values; let index: u64 = 0; let mut alias: &mut u64 = &mut copied[index].view.owned; let earlier: &mut u64 = alias; alias = spare; View { body: earlier } }",
            vec!["self.cell", "self.second_cell"],
        ),
        (
            "earlier_scalar_alias",
            "let result: View = select(CellView { view: &mut self.cell }, &mut self.other); write_view(result);",
            "machine select(input: CellView, spare: &mut u64) -> View { let copied: CellView = input; let mut alias: &mut u64 = &mut copied.view.owned; let earlier: &mut u64 = alias; alias = spare; View { body: earlier } }",
            vec!["self.cell"],
        ),
        (
            "earlier_stored_alias",
            "let result: View = select(CellView { view: &mut self.cell }, &mut self.other); write_view(result);",
            "machine select(input: CellView, spare: &mut u64) -> View { let copied: CellView = input; let mut alias: &mut u64 = &mut copied.view.owned; let earlier: &mut u64 = alias; let held: View = View { body: earlier }; alias = spare; held }",
            vec!["self.cell"],
        ),
    ] {
        for (query, paths) in caller_frames(&carrier_result_program(body, helper))
            .into_iter()
            .enumerate()
        {
            let actual = paths.map(|paths| {
                let mut owners = paths
                    .into_iter()
                    .map(|path| path.strip_suffix(".owned").unwrap_or(&path).to_owned())
                    .collect::<Vec<_>>();
                owners.sort();
                owners.dedup();
                owners
            });
            let expected = Some(
                expected_owners
                    .iter()
                    .map(|owner| (*owner).to_owned())
                    .collect(),
            );
            if actual != expected {
                failures.push(format!(
                    "{name} query {query}: expected {expected:?}, got {actual:?}"
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{failures:?}");
}

#[test]
fn copied_carrier_alias_results_cannot_export_selected_private_owned_fields() {
    for (name, helper) in [
        (
            "copied_array_alias",
            "machine select(values: [Mixed; 1]) -> View { let copied: [Mixed; 1] = values; let alias: &mut u64 = &mut copied[0].owned; View { body: alias } }",
        ),
        (
            "copied_record_alias",
            "machine select(values: [Mixed; 1]) -> View { let copied: Mixed = values[0]; let alias: &mut u64 = &mut copied.owned; View { body: alias } }",
        ),
        (
            "copied_runtime_alias",
            "machine select(values: [Mixed; 1]) -> View { let copied: [Mixed; 1] = values; let index: u64 = 0; let alias: &mut u64 = &mut copied[index].owned; let held: View = View { body: alias }; held }",
        ),
    ] {
        let body = "let result: View = select([Mixed { owned: 0, left: &mut self.cell, right: &mut self.second_cell }]); write_view(result);";
        for (query, paths) in caller_frames(&carrier_result_program(body, helper))
            .into_iter()
            .enumerate()
        {
            assert_eq!(
                paths, None,
                "{name} query {query}: copied private source acquired a relation"
            );
        }
    }
}

#[test]
fn carried_reference_leaf_indexes_preserve_independent_producer_writes() {
    let program = carrier_result_program(
        "let input: [View; 2] = [View { body: &mut self.value }, View { body: &mut self.other }]; let result: View = View { body: input[locate(&mut self.audit)].body }; write_view(result);",
        "machine locate(audit: &mut u64) -> u64 { audit = 1; 0 }",
    );
    let [state, call] = caller_frames(&program);
    assert_eq!(
        state,
        Some(vec![
            "self.audit".to_owned(),
            "self.other".to_owned(),
            "self.value".to_owned(),
        ])
    );
    assert_eq!(
        call,
        Some(vec!["self.other".to_owned(), "self.value".to_owned()])
    );
}

#[test]
fn carried_reference_leaf_indexes_cannot_expose_carrier_replacement_access() {
    for (name, index, helper) in [
        (
            "whole_carrier",
            "locate(&mut input)",
            "machine locate(values: &mut [View; 2]) -> u64 { 0 }",
        ),
        (
            "reference_slot",
            "locate(&mut input[0].body)",
            "machine locate(slot: &mut u64) -> u64 { 0 }",
        ),
    ] {
        let body = format!(
            "let mut input: [View; 2] = [View {{ body: &mut self.value }}, View {{ body: &mut self.other }}]; let result: View = View {{ body: input[{index}].body }}; write_view(result);"
        );
        for (query, paths) in caller_frames(&carrier_result_program(&body, helper))
            .into_iter()
            .enumerate()
        {
            assert_eq!(
                paths, None,
                "{name} query {query}: an index exposed replacement access"
            );
        }
    }
}
