use super::*;
use psi_typed_trees::expression::ExpressionNode;
use psi_typed_trees::statement::StatementNode;

fn parameter_aggregate_program(
    parameter_type: &str,
    body: &str,
    scalar: &str,
) -> psi_typed_trees::TypedTrees {
    let source = format!(
        r#"
        data View {{ body: &mut {scalar}; tag: u64; }}
        data OtherView {{ body: &mut {scalar}; }}
        data Outer {{ inner: View; }}
        data Choice {{ case Selected(view: View); case Empty; }}
        machine write_view(mut view: View) {{ view.body = 255; }}
        machine write_outer(mut outer: Outer) {{ outer.inner.body = 255; }}
        machine write_array(mut views: [View; 2]) {{ views[0].body = 255; }}
        machine write_choice(mut choice: Choice) {{ choice.view.body = 255; }}
        machine probe(mut input: {parameter_type}, index: u64, audit: &mut {scalar}) {{ {body} }}
        machine foreign(input: View) {{}}
        "#
    );
    let syntax =
        parse_syntax_trees(&Lexer::new(&source).tokenize().expect("tokenize")).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    lower_symbol_resolved_trees(&resolved).expect("type")
}

#[test]
fn parameter_aggregate_moves_preserve_caller_reference_origins() {
    let cases = [
        (
            "record",
            "View",
            "let first: View = input; write_view(first);",
            Some("input.body"),
        ),
        (
            "explicit_move",
            "View",
            "let first: View = move input; write_view(first);",
            Some("input.body"),
        ),
        (
            "multi_hop",
            "View",
            "let first: View = input; let second: View = first; write_view(second);",
            Some("input.body"),
        ),
        (
            "record_field",
            "Outer",
            "let first: View = input.inner; write_view(first);",
            Some("input.inner.body"),
        ),
        (
            "fixed_element",
            "[View; 2]",
            "let first: View = input[0]; write_view(first);",
            Some("input"),
        ),
        (
            "runtime_element",
            "[View; 2]",
            "let first: View = input[index]; write_view(first);",
            Some("input"),
        ),
        (
            "indexed_field",
            "[Outer; 2]",
            "let first: View = input[0].inner; write_view(first);",
            Some("input"),
        ),
        (
            "stored_fixed_element",
            "[View; 2]",
            "let first: [View; 2] = input; let second: View = first[1]; write_view(second);",
            Some("input"),
        ),
        (
            "stored_runtime_element",
            "[View; 2]",
            "let first: [View; 2] = input; let second: View = first[index]; write_view(second);",
            Some("input"),
        ),
        (
            "nested_record",
            "View",
            "let first: Outer = Outer { inner: input }; write_outer(first);",
            Some("input.body"),
        ),
        (
            "nested_array",
            "View",
            "let first: [View; 2] = [input, input]; write_array(first);",
            Some("input.body"),
        ),
        (
            "nested_case",
            "View",
            "let first: Choice = Choice::Selected { view: input }; write_choice(first);",
            Some("input.body"),
        ),
        (
            "unknown_case_payload",
            "Choice",
            "let first: View = input.view; write_view(first);",
            None,
        ),
        (
            "stored_unknown_case_payload",
            "Choice",
            "let first: Choice = input; let second: View = first.view; write_view(second);",
            None,
        ),
        (
            "fixed_unknown_case_payload",
            "[Choice; 2]",
            "let first: View = input[0].view; write_view(first);",
            None,
        ),
        (
            "runtime_unknown_case_payload",
            "[Choice; 2]",
            "let first: View = input[index].view; write_view(first);",
            None,
        ),
        (
            "wrong_nominal",
            "OtherView",
            "let first: View = input; write_view(first);",
            None,
        ),
        (
            "wrong_array_length",
            "[View; 1]",
            "let first: [View; 2] = input; write_array(first);",
            None,
        ),
        (
            "reference_parameter",
            "&View",
            "let first: View = input; write_view(first);",
            None,
        ),
        (
            "loaded_reference_carrier",
            "&Outer",
            "let first: View = input.inner; write_view(first);",
            None,
        ),
        (
            "missing_payload",
            "View",
            "let first: Outer = Outer {}; let second: View = first.inner; write_view(second);",
            None,
        ),
        (
            "missing_source",
            "View",
            "let first: View = absent; write_view(first);",
            None,
        ),
        (
            "opaque_prefix",
            "View",
            "unknown(View { body: audit }); let first: View = input; write_view(first);",
            None,
        ),
        (
            "carrier_borrow_before_move",
            "View",
            "unknown(&mut input); let first: View = input; write_view(first);",
            None,
        ),
        (
            "carrier_borrow_after_move",
            "View",
            "let first: View = input; unknown(&mut input); write_view(first);",
            None,
        ),
        (
            "reference_field_replacement",
            "View",
            "input.body = audit; let first: View = input; write_view(first);",
            None,
        ),
        (
            "whole_parameter_replacement",
            "View",
            "input = View { body: audit }; let first: View = input; write_view(first);",
            None,
        ),
        (
            "negative_index",
            "[View; 2]",
            "let first: View = input[-1]; write_view(first);",
            None,
        ),
        (
            "out_of_bounds_index",
            "[View; 2]",
            "let first: View = input[2]; write_view(first);",
            None,
        ),
    ];
    let mut failures = Vec::new();
    for (name, parameter_type, body, expected) in cases {
        let program = parameter_aggregate_program(parameter_type, body, "u64");
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "probe")
            .expect("probe");
        let state = &program.machine_states(machine)[0];
        let StatementNode::Call(call) = program
            .statement_table
            .statements(state.statement_nodes)
            .last()
            .expect("call")
        else {
            panic!("last statement must demand writes");
        };
        let resolver = psi_validation::CallFrameResolver::new(&program).expect("resolver");
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
                    .map(|path| {
                        path.strip_prefix("$P0")
                            .map_or_else(|| path.clone(), |suffix| format!("input{suffix}"))
                    })
                    .filter(|path| path == "input" || path.starts_with("input."))
                    .collect();
                paths.sort();
                paths.dedup();
                paths
            });
            let expected = expected.map(|path| vec![path.to_owned()]);
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
fn parameter_aggregate_record_moves_reach_checked_trees() {
    for (parameter_type, body) in [
        ("View", "let first: View = move input; write_view(first);"),
        ("Outer", "let first: View = input.inner; write_view(first);"),
        (
            "View",
            "let first: Outer = Outer { inner: input }; write_outer(first);",
        ),
    ] {
        lower_typed_trees(parameter_aggregate_program(parameter_type, body, "u64"))
            .expect("parameter carrier move reaches checked trees");
    }
}

#[test]
fn parameter_aggregate_moves_require_exact_live_parameter_identity() {
    let original =
        parameter_aggregate_program("View", "let first: View = input; write_view(first);", "u64");
    let machine = original
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "probe")
        .expect("probe");
    let state = &original.machine_states(machine)[0];
    let StatementNode::LocalData(local) =
        &original.statement_table.statements(state.statement_nodes)[0]
    else {
        panic!("local move");
    };
    let expression = local.initial_value;
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
        ("missing", psi_symbols::SymbolHandle::invalid(), false),
        (
            "stale",
            psi_symbols::SymbolHandle::from_parts(exact.arena_index(), exact.generation() + 1),
            false,
        ),
    ] {
        let mut program = original.clone();
        let ExpressionNode::Name(path) = program.expression_table.expression_mut(expression) else {
            panic!("named parameter");
        };
        path.symbol = symbol;
        path.head_symbol = symbol;
        let member_symbols = path.member_symbols;
        program
            .expression_table
            .set_name_path_member_symbol_at_offset(member_symbols, 0, symbol);
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "probe")
            .expect("probe");
        let state = &program.machine_states(machine)[0];
        let statement = program
            .statement_table
            .statements(state.statement_nodes)
            .last()
            .expect("call");
        let StatementNode::Call(call) = statement else {
            panic!("call");
        };
        let resolver = psi_validation::CallFrameResolver::new(&program).expect("resolver");
        assert_eq!(
            resolver
                .inferred_state_write_frame(machine, state)
                .is_complete(),
            complete,
            "{name}: state"
        );
        assert_eq!(
            resolver.may_write_frame(machine, call).is_complete(),
            complete,
            "{name}: call"
        );
        assert_eq!(
            resolver
                .local_write_origins_before_statement(machine, statement)
                .is_some(),
            complete,
            "{name}: origins"
        );
    }
}

#[test]
fn parameter_array_origin_metadata_is_independent_of_declared_length() {
    for length in [2, 1_000_000_000] {
        let parameter_type = format!("[Outer; {length}]");
        let body = format!(
            "let mut first: [Outer; {length}] = input; let mut second: [Outer; {length}] = first; second[0].inner.body = 255; second[0].inner.tag = 1;"
        );
        let program = parameter_aggregate_program(&parameter_type, &body, "u64");
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "probe")
            .expect("probe");
        let state = &program.machine_states(machine)[0];
        let statements = program.statement_table.statements(state.statement_nodes);
        let statement = statements.last().expect("store");
        let resolver = psi_validation::CallFrameResolver::new(&program).expect("resolver");
        let origins = resolver
            .local_write_origins_before_statement(machine, statement)
            .expect("complete array origins");
        assert_eq!(
            origins.len(),
            2,
            "one leaf per local, not per array element: {length}"
        );
        for origin in origins {
            assert_eq!(origin.source_path, "input");
            assert!(origin.collection_coarse);
            assert!(
                matches!(origin.local_segments.as_slice(), [
                psi_facts::PlaceSegment::Index { expression },
                psi_facts::PlaceSegment::Field { .. },
                psi_facts::PlaceSegment::Field { .. },
            ] if !expression.is_valid()),
                "compact element shape: {:?}",
                origin.local_segments
            );
        }
        let reference_store = crate::flow::statement_storage_writes(
            &program,
            machine.symbol,
            state.symbol,
            statements.len() - 2,
            &statements[statements.len() - 2],
        )
        .expect("reference write projects and reverse-closes");
        assert_eq!(
            reference_store.len(),
            3,
            "caller collection plus both aliases"
        );
        for place in reference_store {
            let psi_facts::PlaceRoot::Symbol(root) = place.root else {
                panic!("storage root");
            };
            match program.symbols.name(root) {
                "input" => assert!(place.segments.is_empty(), "caller array stays coarse"),
                "first" | "second" => {
                    assert!(
                        matches!(place.segments.as_slice(), [
                        psi_facts::PlaceSegment::Index { expression },
                        psi_facts::PlaceSegment::Field { symbol: inner },
                        psi_facts::PlaceSegment::Field { symbol: body },
                    ] if !expression.is_valid()
                        && program.symbols.name(*inner) == "inner"
                        && program.symbols.name(*body) == "body"),
                        "wildcard retains fields: {place:?}"
                    );
                }
                name => panic!("unexpected storage root {name}"),
            }
        }
        let owned_store = crate::flow::statement_storage_writes(
            &program,
            machine.symbol,
            state.symbol,
            statements.len() - 1,
            statement,
        )
        .expect("owned sibling remains private");
        assert_eq!(owned_store.len(), 1);
        assert!(matches!(owned_store[0].segments.as_slice(), [
            psi_facts::PlaceSegment::FixedIndex { index: 0 },
            psi_facts::PlaceSegment::Field { symbol: inner },
            psi_facts::PlaceSegment::Field { symbol: tag },
        ] if program.symbols.name(*inner) == "inner" && program.symbols.name(*tag) == "tag"));
    }
}

#[test]
fn parameter_aggregate_writes_invalidate_both_fact_spellings() {
    for (name, body) in [
        (
            "local_store",
            "input.body = 0; let mut first: View = input; first.body = 255; input.body = input.body + 1;",
        ),
        (
            "local_call",
            "input.body = 0; let first: View = input; write_view(first); input.body = input.body + 1;",
        ),
        (
            "parameter_store",
            "let mut first: View = input; first.body = 0; input.body = 255; input.body = first.body + 1;",
        ),
    ] {
        let program = parameter_aggregate_program("View", body, "u8");
        match psi_validation::validate_program(&program) {
            Err(diagnostics)
                if diagnostics.iter().any(|diagnostic| {
                    let message = diagnostic.to_string();
                    message.contains("probe") && message.contains("may overflow")
                }) => {}
            result => panic!("{name}: stale parameter carrier fact survived: {result:?}"),
        }
    }
}

#[test]
fn parameter_aggregate_helper_move_invalidates_literal_caller_storage() {
    let source = r#"
        data View { body: &mut u8; }
        data Main { value: u8; }
        machine write_view(mut view: View) { view.body = 255; }
        machine forward(input: View) {
            let local: View = input;
            write_view(local);
        }
        machine Main::run(&mut self) {
            self.value = 0;
            forward(View { body: &mut self.value });
            self.value = self.value + 1;
        }
    "#;
    let syntax =
        parse_syntax_trees(&Lexer::new(source).tokenize().expect("tokenize")).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let program = lower_symbol_resolved_trees(&resolved).expect("type");
    match psi_validation::validate_program(&program) {
        Err(diagnostics)
            if diagnostics.iter().any(|diagnostic| {
                let message = diagnostic.to_string();
                message.contains("Main::run") && message.contains("may overflow")
            }) => {}
        result => panic!("parameter move lost transitive caller storage: {result:?}"),
    }
}

#[test]
fn parameter_aggregate_unknown_shapes_and_empty_arrays_cannot_supply_leaves() {
    for (name, declaration, parameter_type, local_type, initializer) in [
        (
            "recursive",
            "data Carrier { body: &mut u64; next: Carrier; }",
            "Carrier",
            "Carrier",
            "input",
        ),
        (
            "generic",
            "data Carrier<T> { body: &mut T; }",
            "Carrier<u64>",
            "Carrier<u64>",
            "input",
        ),
        (
            "zero_array",
            "data Carrier { body: &mut u64; }",
            "[Carrier; 0]",
            "Carrier",
            "input[0]",
        ),
    ] {
        let source = format!(
            r#"
            {declaration}
            machine write_scalar(value: &mut u64) {{ value = 1; }}
            machine probe(input: {parameter_type}) {{
                let first: {local_type} = {initializer};
                write_scalar(first.body);
            }}
        "#
        );
        let syntax =
            parse_syntax_trees(&Lexer::new(&source).tokenize().expect("tokenize")).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let program = lower_symbol_resolved_trees(&resolved).expect("type");
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "probe")
            .expect("probe");
        let state = &program.machine_states(machine)[0];
        let StatementNode::Call(call) = program
            .statement_table
            .statements(state.statement_nodes)
            .last()
            .expect("call")
        else {
            panic!("write demand");
        };
        let resolver = psi_validation::CallFrameResolver::new(&program).expect("resolver");
        assert!(
            !resolver
                .inferred_state_write_frame(machine, state)
                .is_complete(),
            "{name}: state"
        );
        assert!(
            !resolver.may_write_frame(machine, call).is_complete(),
            "{name}: call"
        );
    }
}

#[test]
fn parameter_aggregate_move_survives_named_state_cycle() {
    let source = r#"
        data View { body: &mut u64; }
        machine parameter_cycle(value: View) {
            transition { _ -> cycle(value) }
            state cycle(item: View) {
                let mut local: View = item;
                local.body = 1;
                transition { _ -> cycle(item) }
            }
        }
    "#;
    let syntax =
        parse_syntax_trees(&Lexer::new(source).tokenize().expect("tokenize")).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let program = lower_symbol_resolved_trees(&resolved).expect("type");
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "parameter_cycle")
        .expect("machine");
    let entry = &program.machine_states(machine)[0];
    let resolver = psi_validation::CallFrameResolver::new(&program).expect("resolver");
    for _ in 0..2 {
        assert_eq!(
            resolver
                .inferred_state_write_frame(machine, entry)
                .complete_paths(),
            Some(["$P0.body".to_owned()].as_slice()),
            "cycle retains parameter reference field"
        );
    }
}
