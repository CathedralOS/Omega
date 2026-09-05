use super::*;

fn moved_aggregate_program(body: &str, scalar: &str) -> typed_trees::TypedTrees {
    let source = format!(
        r#"
        data View {{ body: &mut {scalar}; }}
        data OtherView {{ body: &mut {scalar}; }}
        data Outer {{ inner: View; }}
        data Choice {{ case Selected(view: View); case Empty; }}
        data OuterChoice {{ inner: Choice; }}
        data SharedView {{ body: &{scalar}; }}
        data SharedOuter {{ inner: SharedView; }}
        data MixedShared {{ inner: SharedView; writer: View; }}
        data Plain {{ tag: u64; }}
        data Main {{ value: {scalar}; other: {scalar}; outer: Outer; }}
        machine write_view(mut view: View) {{ view.body = 255; }}
        machine write_outer(mut outer: Outer) {{ outer.inner.body = 1; }}
        machine write_array(mut views: [View; 2]) {{ views[0].body = 1; }}
        machine write_choice(mut choice: Choice) {{ choice.view.body = 1; }}
        machine write_plain(mut plain: Plain) {{ plain.tag = 1; }}
        machine consume_choice(choice: Choice) {{}}
        machine write_mixed(mut mixed: MixedShared) {{ mixed.writer.body = 1; }}
        machine Main::run(&mut self, index: u64) {{ {body} }}
        machine Main::foreign(&mut self) {{ let first: View = View {{ body: &mut self.other }}; }}
        "#
    );
    let syntax =
        parse_syntax_trees(&Lexer::new(&source).tokenize().expect("tokenize")).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    lower_symbol_resolved_trees(&resolved).expect("type")
}

#[test]
fn moved_aggregate_declarations_preserve_complete_caller_origins() {
    let cases = [
        (
            "named",
            "let first: View = View { body: &mut self.value }; let second: View = first; write_view(second);",
            Some(vec!["self.value"]),
        ),
        (
            "explicit_move",
            "let first: View = View { body: &mut self.value }; let second: View = move first; write_view(second);",
            Some(vec!["self.value"]),
        ),
        (
            "multi_hop",
            "let first: View = View { body: &mut self.value }; let second: View = first; let third: View = second; write_view(third);",
            Some(vec!["self.value"]),
        ),
        (
            "nested_field",
            "let first: Outer = Outer { inner: View { body: &mut self.value } }; let second: View = first.inner; write_view(second);",
            Some(vec!["self.value"]),
        ),
        (
            "field_after_index",
            "let first: [Outer; 2] = [Outer { inner: View { body: &mut self.value } }, Outer { inner: View { body: &mut self.other } }]; let second: View = first[0].inner; write_view(second);",
            Some(vec!["self.value"]),
        ),
        (
            "fixed_element",
            "let first: [View; 2] = [View { body: &mut self.value }, View { body: &mut self.other }]; let second: View = first[0]; write_view(second);",
            Some(vec!["self.value"]),
        ),
        (
            "runtime_element",
            "let first: [View; 2] = [View { body: &mut self.value }, View { body: &mut self.other }]; let second: View = first[index]; write_view(second);",
            Some(vec!["self.other", "self.value"]),
        ),
        (
            "nested_record_literal",
            "let first: View = View { body: &mut self.value }; let second: Outer = Outer { inner: first }; write_outer(second);",
            Some(vec!["self.value"]),
        ),
        (
            "nested_array_literal",
            "let first: View = View { body: &mut self.value }; let other: View = View { body: &mut self.other }; let second: [View; 2] = [first, other]; write_array(second);",
            Some(vec!["self.other", "self.value"]),
        ),
        (
            "nested_case_literal",
            "let first: View = View { body: &mut self.value }; let second: Choice = Choice::Selected { view: first }; write_choice(second);",
            Some(vec!["self.value"]),
        ),
        (
            "selected_payload",
            "let first: Choice = Choice::Selected { view: View { body: &mut self.value } }; let second: View = first.view; write_view(second);",
            Some(vec!["self.value"]),
        ),
        (
            "zero_exclusive_leaves",
            "let first: Plain = Plain { tag: 0 }; let second: Plain = first; write_plain(second);",
            Some(vec![]),
        ),
        (
            "inactive_exclusive_payload",
            "let first: Choice = Choice::Empty {}; let second: Choice = first; consume_choice(second);",
            Some(vec![]),
        ),
        (
            "wrong_nominal",
            "let first: OtherView = OtherView { body: &mut self.value }; let second: View = first; write_view(second);",
            None,
        ),
        (
            "absent_reference_payload",
            "let first: Choice = Choice::Empty {}; let second: View = first.view; write_view(second);",
            None,
        ),
        (
            "empty_case_subtree",
            "let first: OuterChoice = OuterChoice { inner: Choice::Empty {} }; let second: Choice = first.inner; consume_choice(second);",
            Some(vec![]),
        ),
        (
            "fixed_mixed_case_payload",
            "let first: [Choice; 2] = [Choice::Selected { view: View { body: &mut self.value } }, Choice::Empty {}]; let second: View = first[0].view; write_view(second);",
            Some(vec!["self.value"]),
        ),
        (
            "runtime_mixed_case_payload",
            "let first: [Choice; 2] = [Choice::Selected { view: View { body: &mut self.value } }, Choice::Empty {}]; let second: View = first[index].view; write_view(second);",
            None,
        ),
        (
            "empty_carrier_borrow",
            "let mut first: Choice = Choice::Empty {}; unknown(&mut first);",
            None,
        ),
        (
            "shared_only_subrecord",
            "let first: SharedOuter = SharedOuter { inner: SharedView { body: &self.value } }; let second: MixedShared = MixedShared { inner: first.inner, writer: View { body: &mut self.other } }; write_mixed(second);",
            Some(vec!["self.other"]),
        ),
        (
            "wrong_array_length",
            "let first: [View; 1] = [View { body: &mut self.value }]; let second: [View; 2] = first; write_array(second);",
            None,
        ),
        (
            "missing_leaf",
            "let first: View = View {}; let second: View = first; write_view(second);",
            None,
        ),
        (
            "opaque_leaf",
            "let first: View = View { body: unknown(&mut self.value) }; let second: View = first; write_view(second);",
            None,
        ),
        (
            "missing_source",
            "let second: View = absent; write_view(second);",
            None,
        ),
        (
            "opaque_prefix",
            "unknown(View { body: &mut self.other }); let first: View = View { body: &mut self.value }; let second: View = first; write_view(second);",
            None,
        ),
        (
            "negative_index",
            "let first: [View; 2] = [View { body: &mut self.value }, View { body: &mut self.other }]; let second: View = first[-1]; write_view(second);",
            None,
        ),
        (
            "out_of_bounds_index",
            "let first: [View; 2] = [View { body: &mut self.value }, View { body: &mut self.other }]; let second: View = first[2]; write_view(second);",
            None,
        ),
        (
            "loaded_reference_carrier",
            "let first: &Outer = &self.outer; let second: View = first.inner; write_view(second);",
            None,
        ),
    ];
    let mut failures = Vec::new();
    for (name, body, expected) in cases {
        let program = moved_aggregate_program(body, "u64");
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Main::run")
            .expect("caller");
        let state = &program.machine_states(machine)[0];
        let statement = program
            .statement_table
            .statements(state.statement_nodes)
            .last()
            .expect("call");
        let typed_trees::statement::StatementNode::Call(call) = statement else {
            panic!("call");
        };
        let resolver = validation::CallFrameResolver::new(&program).expect("resolver");
        for (query, actual) in [
            (
                "state",
                resolver
                    .inferred_state_write_frame(machine, state)
                    .into_complete_paths(),
            ),
            ("public call", resolver.may_write_paths(machine, call)),
        ] {
            let actual = actual.map(|paths| {
                let mut caller_paths: Vec<_> = paths
                    .into_iter()
                    .filter(|path| path == "self" || path.starts_with("self."))
                    .collect();
                caller_paths.sort();
                caller_paths.dedup();
                caller_paths
            });
            let expected = expected
                .as_ref()
                .map(|paths| paths.iter().map(|path| (*path).to_owned()).collect());
            if actual != expected {
                failures.push(format!(
                    "{name} {query}: expected {expected:?}, got {actual:?}"
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{failures:?}");
}

fn moved_storage_label(
    program: &typed_trees::TypedTrees,
    place: &crate::flow::CanonicalPlace,
) -> String {
    let facts::PlaceRoot::Symbol(root) = place.root else {
        panic!("storage root: {place:?}");
    };
    let mut label = program.symbols.name(root).to_owned();
    for segment in &place.segments {
        match segment {
            facts::PlaceSegment::Field { symbol } => {
                label.push('.');
                label.push_str(program.symbols.name(*symbol));
            }
            facts::PlaceSegment::FixedIndex { index } => label.push_str(&format!("[{index}]")),
            _ => panic!("unexpected selector: {segment:?}"),
        }
    }
    label
}

#[test]
fn moved_aggregate_storage_keeps_selected_leaf_precision() {
    for (name, body, expected) in [
        (
            "fixed",
            "let first: [View; 2] = [View { body: &mut self.value }, View { body: &mut self.other }]; let mut second: View = first[0]; second.body = 1;",
            vec!["first[0].body", "second.body", "self.value"],
        ),
        (
            "runtime",
            "let first: [View; 2] = [View { body: &mut self.value }, View { body: &mut self.other }]; let mut second: View = first[index]; second.body = 1;",
            vec![
                "first[0].body",
                "first[1].body",
                "second.body",
                "self.other",
                "self.value",
            ],
        ),
        (
            "field",
            "let first: Outer = Outer { inner: View { body: &mut self.value } }; let mut second: View = first.inner; second.body = 1;",
            vec!["first.inner.body", "second.body", "self.value"],
        ),
        (
            "field_after_index",
            "let first: [Outer; 2] = [Outer { inner: View { body: &mut self.value } }, Outer { inner: View { body: &mut self.other } }]; let mut second: View = first[0].inner; second.body = 1;",
            vec!["first[0].inner.body", "second.body", "self.value"],
        ),
        (
            "multi_hop",
            "let first: View = View { body: &mut self.value }; let second: View = first; let mut third: View = second; third.body = 1;",
            vec!["first.body", "second.body", "self.value", "third.body"],
        ),
    ] {
        let program = moved_aggregate_program(body, "u64");
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Main::run")
            .expect("caller");
        let state = &program.machine_states(machine)[0];
        let statements = program.statement_table.statements(state.statement_nodes);
        let places = crate::flow::statement_storage_writes(
            &program,
            machine.symbol,
            state.symbol,
            statements.len() - 1,
            statements.last().expect("store"),
        )
        .unwrap_or_else(|| panic!("{name}: complete storage origin"));
        let mut actual: Vec<_> = places
            .iter()
            .map(|place| moved_storage_label(&program, place))
            .collect();
        actual.sort();
        assert_eq!(actual, expected, "{name}");
    }
}

#[test]
fn moved_aggregate_names_and_fields_reach_checked_trees() {
    for body in [
        "let first: View = View { body: &mut self.value }; let second: View = move first; write_view(second);",
        "let first: Outer = Outer { inner: View { body: &mut self.value } }; let second: View = first.inner; write_view(second);",
        "let first: OuterChoice = OuterChoice { inner: Choice::Empty {} }; let second: Choice = first.inner; consume_choice(second);",
        "let first: SharedOuter = SharedOuter { inner: SharedView { body: &self.value } }; let second: MixedShared = MixedShared { inner: first.inner, writer: View { body: &mut self.other } }; write_mixed(second);",
    ] {
        lower_typed_trees(moved_aggregate_program(body, "u64"))
            .expect("valid carrier move reaches checked trees");
    }
}

#[test]
fn moved_aggregate_source_requires_exact_live_local_identity() {
    use typed_trees::expression::ExpressionNode;
    use typed_trees::statement::StatementNode;
    let original = moved_aggregate_program(
        "let first: View = View { body: &mut self.value }; let second: View = first; write_view(second);",
        "u64",
    );
    let caller = original
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::run")
        .expect("caller");
    let statements = original.machine_states(caller)[0].statement_nodes;
    let StatementNode::LocalData(second) = &original.statement_table.statements(statements)[1]
    else {
        panic!("second");
    };
    let source = second.initial_value;
    let ExpressionNode::Name(name) = original.expression_table.expression(source) else {
        panic!("name");
    };
    let exact = name.symbol;
    let foreign = original
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::foreign")
        .expect("foreign");
    let StatementNode::LocalData(first) = &original
        .statement_table
        .statements(original.machine_states(foreign)[0].statement_nodes)[0]
    else {
        panic!("foreign first");
    };
    for (name, symbol, complete) in [
        ("exact", exact, true),
        ("foreign", first.symbol, false),
        (
            "stale",
            symbols::SymbolHandle::from_parts(exact.arena_index(), exact.generation() + 1),
            false,
        ),
    ] {
        let mut program = original.clone();
        let ExpressionNode::Name(path) = program.expression_table.expression_mut(source) else {
            panic!("source");
        };
        path.symbol = symbol;
        path.head_symbol = symbol;
        let member_symbols = path.member_symbols;
        program
            .expression_table
            .set_name_path_member_symbol_at_offset(member_symbols, 0, symbol);
        let caller = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Main::run")
            .expect("caller");
        let state = &program.machine_states(caller)[0];
        let resolver = validation::CallFrameResolver::new(&program).expect("resolver");
        let call_statement = program
            .statement_table
            .statements(statements)
            .last()
            .expect("call");
        let StatementNode::Call(call) = call_statement else {
            panic!("call");
        };
        assert_eq!(
            resolver
                .inferred_state_write_frame(caller, state)
                .is_complete(),
            complete,
            "{name}: state"
        );
        assert_eq!(
            resolver.may_write_frame(caller, call).is_complete(),
            complete,
            "{name}: demand"
        );
        assert_eq!(
            resolver
                .local_write_origins_before_statement(caller, call_statement)
                .is_some(),
            complete,
            "{name}: metadata"
        );
    }
}

#[test]
fn moved_aggregate_writes_cannot_preserve_stale_arithmetic_facts() {
    for (name, body) in [
        (
            "direct",
            "self.value = 0; let first: View = View { body: &mut self.value }; let mut second: View = first; second.body = 255; self.value = self.value + 1;",
        ),
        (
            "call",
            "self.value = 0; let first: View = View { body: &mut self.value }; let second: View = first; write_view(second); self.value = self.value + 1;",
        ),
        (
            "owner_to_moved_leaf",
            "let first: View = View { body: &mut self.value }; let mut second: View = first; second.body = 0; self.value = 255; self.value = second.body + 1;",
        ),
    ] {
        let program = moved_aggregate_program(body, "u8");
        match validation::validate_program(&program) {
            Err(diagnostics)
                if diagnostics.iter().any(|diagnostic| {
                    let message = diagnostic.to_string();
                    message.contains("Main::run") && message.contains("may overflow")
                }) => {}
            result => panic!("{name}: stale carrier fact survived: {result:?}"),
        }
    }
}
