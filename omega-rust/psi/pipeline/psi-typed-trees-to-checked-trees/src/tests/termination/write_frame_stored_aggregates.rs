use super::*;

fn stored_aggregate_program(body: &str) -> psi_typed_trees::TypedTrees {
    let source = format!(
        r#"
        data View {{ body: &mut u64; tag: u64; }}
        data PairView {{ left: &mut u64; right: &mut u64; }}
        data Outer {{ inner: View; }}
        data Choice {{ case Reference(body: &mut u64); case Empty; }}
        data Cell {{ value: u64; }}
        data CellView {{ body: &mut Cell; }}
        data Main {{ value: u64; other: u64; cell: Cell; cells: [Cell; 2]; }}
        machine store(mut view: View) {{ view.body = 1; }}
        machine relay(value: &mut u64) {{ store(View {{ body: value, tag: 0 }}); }}
        machine identity(value: &mut u64) -> &mut u64 {{ value }}
        machine Main::run(&mut self, index: u64) {{ {body} }}
        "#
    );
    let syntax =
        parse_syntax_trees(&Lexer::new(&source).tokenize().expect("tokenize")).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    lower_symbol_resolved_trees(&resolved).expect("type")
}

#[test]
fn stored_aggregate_reference_leaves_reach_caller_frames() {
    let cases = [
        (
            "nested_record",
            "let mut outer: Outer = Outer { inner: View { body: &mut self.value, tag: 0 } }; outer.inner.body = 1;",
            Some(vec!["self.value"]),
        ),
        (
            "selected_case",
            "let mut choice: Choice = Choice::Reference { body: &mut self.value }; choice.body = 1;",
            Some(vec!["self.value"]),
        ),
        (
            "record",
            "let mut view: View = View { body: &mut self.value, tag: 0 }; view.body = 1;",
            Some(vec!["self.value"]),
        ),
        (
            "independent_references",
            "let mut pair: PairView = PairView { left: &mut self.value, right: &mut self.other }; pair.left = 1; pair.right = 2;",
            Some(vec!["self.other", "self.value"]),
        ),
        (
            "fixed_array",
            "let mut views: [View; 2] = [View { body: &mut self.value, tag: 0 }, View { body: &mut self.other, tag: 0 }]; views[0].body = 1;",
            Some(vec!["self.other", "self.value"]),
        ),
        (
            "runtime_array",
            "let mut views: [View; 2] = [View { body: &mut self.value, tag: 0 }, View { body: &mut self.other, tag: 0 }]; views[index].body = 1;",
            Some(vec!["self.other", "self.value"]),
        ),
        (
            "owned_sibling",
            "let mut view: View = View { body: &mut self.value, tag: 0 }; view.tag = 1;",
            Some(vec![]),
        ),
        (
            "stored_call_argument",
            "let view: View = View { body: &mut self.value, tag: 0 }; store(view);",
            Some(vec!["self.value"]),
        ),
        (
            "prior_alias",
            "let mut selected: &mut u64 = &mut self.value; let prior: &mut u64 = selected; let mut view: View = View { body: prior, tag: 0 }; selected = &mut self.other; view.body = 1; selected = 2;",
            Some(vec!["self.other", "self.value"]),
        ),
        (
            "missing_leaf",
            "let mut view: View = View { tag: 0 }; view.body = 1;",
            None,
        ),
        (
            "opaque_leaf",
            "let mut view: View = View { body: unknown(&mut self.value), tag: 0 }; view.body = 1;",
            None,
        ),
        (
            "reference_field_replacement",
            "let mut view: View = View { body: &mut self.value, tag: 0 }; view.body = &mut self.other; view.body = 1;",
            None,
        ),
        (
            "whole_carrier_replacement",
            "let mut view: View = View { body: &mut self.value, tag: 0 }; view = View { body: &mut self.other, tag: 0 }; view.body = 1;",
            None,
        ),
        (
            "stored_reference_slot_reborrow",
            "let first: View = View { body: &mut self.value, tag: 0 }; let mut second: View = View { body: &mut first.body, tag: 0 }; second.body = 1;",
            None,
        ),
        (
            "helper_wrapped_stored_reference_slot_reborrow",
            "let first: View = View { body: &mut self.value, tag: 0 }; let mut second: View = View { body: identity(&mut first.body), tag: 0 }; second.body = 1;",
            None,
        ),
        (
            "stored_member_origin",
            "let mut first: CellView = CellView { body: &mut self.cell }; let mut second: View = View { body: &mut first.body.value, tag: 0 }; second.body = 1;",
            Some(vec!["self.cell.value"]),
        ),
        (
            "coarse_stored_array_origin",
            "let mut first: [CellView; 2] = [CellView { body: &mut self.cell }, CellView { body: &mut self.cells[0] }]; let mut second: View = View { body: &mut first[index].body.value, tag: 0 }; second.body = 1;",
            Some(vec!["self.cell", "self.cells"]),
        ),
    ];
    let mut failures = Vec::new();
    for (name, body, expected) in cases {
        let program = stored_aggregate_program(body);
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Main::run")
            .expect("caller");
        let state = &program.machine_states(machine)[0];
        let resolver = psi_validation::CallFrameResolver::new(&program).expect("resolver");
        let actual = resolver
            .inferred_state_write_frame(machine, state)
            .into_complete_paths()
            .map(|mut paths| {
                paths.sort();
                paths
            });
        let expected = expected.map(|paths| paths.into_iter().map(str::to_owned).collect());
        if actual != expected {
            failures.push(format!("{name}: expected {expected:?}, got {actual:?}"));
        }
    }
    assert!(failures.is_empty(), "{failures:?}");
}

fn storage_place_label(
    program: &psi_typed_trees::TypedTrees,
    place: &crate::flow::CanonicalPlace,
) -> String {
    let psi_facts::PlaceRoot::Symbol(root) = place.root else {
        panic!("expected declared storage root: {place:?}");
    };
    let mut label = program.symbols.name(root).to_owned();
    for segment in &place.segments {
        match segment {
            psi_facts::PlaceSegment::Case { variant } => {
                label.push_str("::");
                label.push_str(program.symbols.name(*variant));
            }
            psi_facts::PlaceSegment::Field { symbol } => {
                label.push('.');
                label.push_str(program.symbols.name(*symbol));
            }
            psi_facts::PlaceSegment::FixedIndex { index } => label.push_str(&format!("[{index}]")),
            _ => panic!("unexpected storage selector: {segment:?}"),
        }
    }
    label
}

#[test]
fn stored_aggregate_storage_projection_keeps_leaf_selectors() {
    let cases = [
        (
            "nested_record",
            "let mut outer: Outer = Outer { inner: View { body: &mut self.value, tag: 0 } }; outer.inner.body = 1;",
            vec!["outer.inner.body", "self.value"],
        ),
        (
            "selected_case",
            "let mut choice: Choice = Choice::Reference { body: &mut self.value }; choice.body = 1;",
            vec!["choice::Reference.body", "self.value"],
        ),
        (
            "record",
            "let mut view: View = View { body: &mut self.value, tag: 0 }; view.body = 1;",
            vec!["self.value", "view.body"],
        ),
        (
            "fixed_array",
            "let mut views: [View; 2] = [View { body: &mut self.value, tag: 0 }, View { body: &mut self.other, tag: 0 }]; views[0].body = 1;",
            vec!["self.value", "views[0].body"],
        ),
        (
            "runtime_array",
            "let mut views: [View; 2] = [View { body: &mut self.value, tag: 0 }, View { body: &mut self.other, tag: 0 }]; views[index].body = 1;",
            vec!["self.other", "self.value", "views[0].body", "views[1].body"],
        ),
        (
            "owned_sibling",
            "let mut view: View = View { body: &mut self.value, tag: 0 }; view.tag = 1;",
            vec!["view.tag"],
        ),
        (
            "reference_suffix",
            "let mut view: CellView = CellView { body: &mut self.cell }; view.body.value = 1;",
            vec!["self.cell.value", "view.body.value"],
        ),
        (
            "coarse_source",
            "let mut view: CellView = CellView { body: &mut self.cells[0] }; view.body.value = 1;",
            vec!["self.cells", "view.body"],
        ),
        (
            "independent_sibling",
            "let mut pair: PairView = PairView { left: &mut self.value, right: &mut self.other }; pair.left = 1;",
            vec!["pair.left", "self.value"],
        ),
        (
            "prior_alias_after_rebinding",
            "let mut selected: &mut u64 = &mut self.value; let prior: &mut u64 = selected; let mut view: View = View { body: prior, tag: 0 }; selected = &mut self.other; view.body = 1;",
            vec!["prior", "self.value", "view.body"],
        ),
        (
            "stored_member_origin",
            "let mut first: CellView = CellView { body: &mut self.cell }; let mut second: View = View { body: &mut first.body.value, tag: 0 }; second.body = 1;",
            vec!["first.body.value", "second.body", "self.cell.value"],
        ),
        (
            "coarse_stored_array_origin",
            "let mut first: [CellView; 2] = [CellView { body: &mut self.cell }, CellView { body: &mut self.cells[0] }]; let mut second: View = View { body: &mut first[index].body.value, tag: 0 }; second.body = 1;",
            vec![
                "first[0].body",
                "first[1].body",
                "second.body",
                "self.cell",
                "self.cells",
            ],
        ),
    ];
    let mut failures = Vec::new();
    for (name, body, expected) in cases {
        let program = stored_aggregate_program(body);
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Main::run")
            .expect("caller");
        let state = &program.machine_states(machine)[0];
        let statements = program.statement_table.statements(state.statement_nodes);
        let actual = crate::flow::statement_storage_writes(
            &program,
            machine.symbol,
            state.symbol,
            statements.len() - 1,
            statements.last().expect("store"),
        )
        .map(|places| {
            let mut paths: Vec<_> = places
                .iter()
                .map(|place| storage_place_label(&program, place))
                .collect();
            paths.sort();
            paths
        });
        let expected = Some(expected.into_iter().map(str::to_owned).collect());
        if actual != expected {
            failures.push(format!("{name}: expected {expected:?}, got {actual:?}"));
        }
    }
    assert!(failures.is_empty(), "{failures:?}");
}

#[test]
fn stored_aggregate_call_argument_reaches_checked_trees() {
    for body in [
        "let view: View = View { body: &mut self.value, tag: 0 }; store(view);",
        "let mut first: CellView = CellView { body: &mut self.cell }; let second: View = View { body: &mut first.body.value, tag: 0 }; store(second);",
    ] {
        let program = stored_aggregate_program(body);
        lower_typed_trees(program).expect("stored reference-bearing value reaches checked callee");
    }
}

#[test]
fn stored_aggregate_call_storage_and_access_routes_are_distinct() {
    let program = stored_aggregate_program(
        "let view: View = View { body: &mut self.value, tag: 0 }; store(view);",
    );
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::run")
        .expect("caller");
    let state = &program.machine_states(machine)[0];
    let borrow = crate::build_borrow_facts(&program);
    let borrow_state = borrow
        .states
        .iter()
        .map(|(_, state)| state)
        .find(|candidate| candidate.state_symbol == state.symbol)
        .expect("borrow state");
    let calls = borrow.calls.span_or_empty(borrow_state.calls);
    assert_eq!(calls.len(), 1);
    let mut cache = crate::flow::StateMutationSummaryCache::default();
    let storage = crate::flow::call_mutated_places(
        &program,
        machine.symbol,
        state.symbol,
        &borrow,
        &calls[0],
        &mut cache,
    )
    .expect("complete storage frame");
    assert_eq!(
        storage
            .iter()
            .map(|place| storage_place_label(&program, place))
            .collect::<Vec<_>>(),
        vec!["self.value"]
    );
    let access = crate::flow::call_write_accesses(
        &program,
        machine.symbol,
        state.symbol,
        &borrow,
        &calls[0],
        &mut cache,
    );
    assert_eq!(
        access
            .iter()
            .map(|place| storage_place_label(&program, place))
            .collect::<Vec<_>>(),
        vec!["view.body"]
    );
}

#[test]
fn aggregate_literal_storage_origins_reach_direct_and_transitive_calls() {
    for body in [
        "store(View { body: &mut self.value, tag: 0 });",
        "relay(&mut self.value);",
    ] {
        let program = stored_aggregate_program(body);
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Main::run")
            .expect("caller");
        let state = &program.machine_states(machine)[0];
        let borrow = crate::build_borrow_facts(&program);
        let borrow_state = borrow
            .states
            .iter()
            .map(|(_, state)| state)
            .find(|candidate| candidate.state_symbol == state.symbol)
            .expect("borrow state");
        let calls = borrow.calls.span_or_empty(borrow_state.calls);
        assert_eq!(calls.len(), 1);
        let mut cache = crate::flow::StateMutationSummaryCache::default();
        let storage = crate::flow::call_mutated_places(
            &program,
            machine.symbol,
            state.symbol,
            &borrow,
            &calls[0],
            &mut cache,
        )
        .expect("literal storage origin is complete");
        assert_eq!(
            storage
                .iter()
                .map(|place| storage_place_label(&program, place))
                .collect::<Vec<_>>(),
            vec!["self.value"],
            "{body}"
        );
    }
}

#[test]
fn unproven_stored_aggregate_origins_never_become_private_storage() {
    for body in [
        "let mut view: View = View { tag: 0 }; view.body = 1;",
        "let mut view: View = View { body: unknown(&mut self.value), tag: 0 }; view.body = 1;",
        "let mut view: View = View { body: &mut self.value, tag: 0 }; view.body = &mut self.other; view.body = 1;",
        "let mut view: View = View { body: &mut self.value, tag: 0 }; view = View { body: &mut self.other, tag: 0 }; view.body = 1;",
        "let first: View = View { body: &mut self.value, tag: 0 }; let mut second: View = View { body: &mut first.body, tag: 0 }; second.body = 1;",
        "let first: View = View { body: &mut self.value, tag: 0 }; let mut second: View = View { body: identity(&mut first.body), tag: 0 }; second.body = 1;",
    ] {
        let program = stored_aggregate_program(body);
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Main::run")
            .expect("caller");
        let state = &program.machine_states(machine)[0];
        let statements = program.statement_table.statements(state.statement_nodes);
        assert!(
            crate::flow::statement_storage_writes(
                &program,
                machine.symbol,
                state.symbol,
                statements.len() - 1,
                statements.last().expect("store")
            )
            .is_none(),
            "unproven origin became complete: {body}"
        );
    }
}

#[test]
fn stored_aggregate_reference_origin_survives_named_state_cycle() {
    let source = r#"
        data View { body: &mut u64; }
        machine stored_cycle(value: &mut u64) {
            transition { _ -> cycle(value) }
            state cycle(item: &mut u64) {
                let mut view: View = View { body: item };
                view.body = 1;
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
        .find(|machine| machine.name.as_str() == "stored_cycle")
        .expect("machine");
    let entry = &program.machine_states(machine)[0];
    let resolver = psi_validation::CallFrameResolver::new(&program).expect("resolver");
    for _ in 0..2 {
        assert_eq!(
            resolver
                .inferred_state_write_frame(machine, entry)
                .complete_paths(),
            Some(["$P0".to_owned()].as_slice()),
            "cycle preserves the entry parameter origin"
        );
    }
}

#[test]
fn stored_aggregate_writes_invalidate_arithmetic_facts_in_both_spellings() {
    let cases = [
        (
            "carrier to owner",
            "self.value = 0; let mut view: View = View { body: &mut self.value }; view.body = 255; self.value = self.value + 1;",
        ),
        (
            "owner to carrier",
            "let mut view: View = View { body: &mut self.value }; view.body = 0; self.value = 255; self.value = view.body + 1;",
        ),
        (
            "stored carrier call to owner",
            "self.value = 0; let view: View = View { body: &mut self.value }; store(view); self.value = self.value + 1;",
        ),
    ];
    let mut failures = Vec::new();
    for (name, body) in cases {
        let source = format!(
            "data View {{ body: &mut u8; }} data Main {{ value: u8; }}
             machine store(mut view: View) {{ view.body = 255; }}
             machine Main::run(&mut self) {{ {body} }}"
        );
        let syntax =
            parse_syntax_trees(&Lexer::new(&source).tokenize().expect("tokenize")).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let program = lower_symbol_resolved_trees(&resolved).expect("type");
        match psi_validation::validate_program(&program) {
            Err(diagnostics)
                if diagnostics.iter().any(|diagnostic| {
                    let message = diagnostic.to_string();
                    message.contains("Main::run") && message.contains("may overflow")
                }) => {}
            result => failures.push(format!("{name}: {result:?}")),
        }
    }
    assert!(
        failures.is_empty(),
        "stale carrier facts cannot establish u8 overflow safety: {failures:?}"
    );
}

#[test]
fn stored_aggregate_metadata_requires_exact_live_local_identity() {
    use psi_typed_trees::statement::StatementNode;
    let source = r#"
        data View { body: &mut u64; }
        data Main { value: u64; }
        machine Main::run(&mut self) {
            let mut view: View = View { body: &mut self.value };
            view.body = 1;
        }
        machine Main::foreign(&mut self) {
            let mut view: View = View { body: &mut self.value };
            view.body = 2;
        }
    "#;
    let syntax =
        parse_syntax_trees(&Lexer::new(source).tokenize().expect("tokenize")).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let original = lower_symbol_resolved_trees(&resolved).expect("type");
    let caller = original
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::run")
        .expect("caller");
    let statements = original.machine_states(caller)[0].statement_nodes;
    let StatementNode::LocalData(local) = &original.statement_table.statements(statements)[0]
    else {
        panic!("local declaration");
    };
    let exact = local.symbol;
    let foreign = original
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::foreign")
        .expect("foreign machine");
    let foreign_statements = original.machine_states(foreign)[0].statement_nodes;
    let StatementNode::LocalData(foreign_local) =
        &original.statement_table.statements(foreign_statements)[0]
    else {
        panic!("foreign local declaration");
    };
    for (name, symbol, complete) in [
        ("exact", exact, true),
        (
            "same spelling in foreign state",
            foreign_local.symbol,
            false,
        ),
        (
            "stale generation",
            psi_symbols::SymbolHandle::from_parts(exact.arena_index(), exact.generation() + 1),
            false,
        ),
    ] {
        let mut program = original.clone();
        let StatementNode::LocalData(local) =
            &mut program.statement_table.statements_mut(statements)[0]
        else {
            panic!("local declaration");
        };
        local.symbol = symbol;
        let caller = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Main::run")
            .expect("caller");
        let state = &program.machine_states(caller)[0];
        let statement = &program.statement_table.statements(statements)[1];
        let resolver = psi_validation::CallFrameResolver::new(&program).expect("resolver");
        let origins = resolver.local_write_origins_before_statement(caller, statement);
        assert_eq!(origins.is_some(), complete, "{name}: stored metadata");
        if let Some(origins) = origins {
            assert_eq!(origins.len(), 1);
            assert_eq!(origins[0].local_symbol, exact);
            assert_eq!(origins[0].source_path, "self.value");
        }
        let frame = resolver.inferred_state_write_frame(caller, state);
        assert_eq!(frame.is_complete(), complete, "{name}: raw frame");
        if complete {
            assert_eq!(
                frame.complete_paths(),
                Some(["self.value".to_owned()].as_slice())
            );
        }
    }
}
