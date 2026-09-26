use crate::tests::front_end::typed_program;

fn stored_aggregate_program(body: &str) -> typed_trees::TypedTrees {
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
    typed_program(&source)
}

fn storage_place_label(
    program: &typed_trees::TypedTrees,
    place: &crate::flow::CanonicalPlace,
) -> String {
    let facts::PlaceRoot::Symbol(root) = place.root else {
        panic!("expected declared storage root: {place:?}");
    };
    let mut label = program.symbols.name(root).to_owned();
    for segment in &place.segments {
        match segment {
            facts::PlaceSegment::Case { variant } => {
                label.push_str("::");
                label.push_str(program.symbols.name(*variant));
            }
            facts::PlaceSegment::Field { symbol } => {
                label.push('.');
                label.push_str(program.symbols.name(*symbol));
            }
            facts::PlaceSegment::FixedIndex { index } => label.push_str(&format!("[{index}]")),
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
            "replaced_whole_carrier",
            "let mut view: View = View { body: &mut self.value, tag: 0 }; view = View { body: &mut self.other, tag: 0 }; view.body = 1;",
            vec!["self.other", "view.body"],
        ),
        (
            "reference_suffix",
            "let mut view: CellView = CellView { body: &mut self.cell }; view.body.value = 1;",
            vec!["self.cell.value", "view.body.value"],
        ),
        (
            "fixed_source",
            "let mut view: CellView = CellView { body: &mut self.cells[0] }; view.body.value = 1;",
            vec!["self.cells[0].value", "view.body.value"],
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
            "dynamic_stored_array_origins",
            "let mut first: [CellView; 2] = [CellView { body: &mut self.cell }, CellView { body: &mut self.cells[0] }]; let mut second: View = View { body: &mut first[index].body.value, tag: 0 }; second.body = 1;",
            vec![
                "first[0].body.value",
                "first[1].body.value",
                "second.body",
                "self.cell.value",
                "self.cells[0].value",
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
            ::validation::CallFrameResolver::new(&program).as_ref(),
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
fn stored_aggregate_metadata_requires_exact_live_local_identity() {
    use typed_trees::statement::StatementNode;
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
    let original = typed_program(source);
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
            symbols::SymbolHandle::from_parts(exact.arena_index(), exact.generation() + 1),
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
        let resolver = validation::CallFrameResolver::new(&program).expect("resolver");
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
