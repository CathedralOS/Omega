use super::*;

#[test]
fn write_frame_stays_opaque_for_non_bijective_exclusive_cycle() {
    // This deliberately duplicates one exclusive parameter on a backedge.
    // Query the typed-tree frame resolver directly: later borrow validation is
    // allowed to reject the source independently, while R5 must still fail
    // closed if it is asked to summarize the malformed cycle.
    let source = r#"
    machine duplicate_cycle(first: &mut u64, second: &mut u64) {
        transition { _ -> cycle(first, second) }
        state cycle(left: &mut u64, right: &mut u64) {
            left = 1;
            transition { _ -> cycle(left, left) }
        }
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "duplicate_cycle")
        .expect("duplicate-cycle machine");
    let entry = typed
        .machine_states(machine)
        .first()
        .expect("duplicate-cycle entry state");
    let resolver = validation::CallFrameResolver::new(&typed).expect("valid symbol cache");

    for query in 0..2 {
        assert!(
            !resolver
                .inferred_state_write_frame(machine, entry)
                .is_complete(),
            "duplicating one exclusive root must remain opaque on query {query}"
        );
    }
}

#[test]
fn write_frame_composes_transparent_helpers_in_exclusive_cycles() {
    let source = r#"
    machine identity(value: &mut u64) -> &mut u64 {
        value
    }

    machine write(value: &mut u64) {
        value = 2;
    }

    machine write_then_identity(value: &mut u64) -> &mut u64 {
        write(value);
        value
    }

    machine transparent_cycle(value: &mut u64) {
        transition { _ -> cycle(value) }
        state cycle(item: &mut u64) {
            item = 1;
            transition { _ -> cycle(identity(item)) }
        }
    }

    machine write_through_helper_cycle(value: &mut u64) {
        transition { _ -> cycle(value) }
        state cycle(item: &mut u64) {
            item = 1;
            transition { _ -> cycle(write_then_identity(item)) }
        }
    }

    machine duplicate_transparent_cycle(first: &mut u64, second: &mut u64) {
        transition { _ -> cycle(first, second) }
        state cycle(left: &mut u64, right: &mut u64) {
            left = 1;
            transition { _ -> cycle(identity(left), identity(left)) }
        }
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let resolver = validation::CallFrameResolver::new(&typed).expect("valid symbol cache");
    let frame = |name: &str| {
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap_or_else(|| panic!("{name} machine"));
        let entry = typed
            .machine_states(machine)
            .first()
            .unwrap_or_else(|| panic!("{name} entry state"));
        resolver.inferred_state_write_frame(machine, entry)
    };

    assert_eq!(
        frame("transparent_cycle").complete_paths(),
        Some(["$P0".to_owned()].as_slice()),
        "a transparent identity helper preserves the cycle's exact root permutation"
    );
    assert_eq!(
        frame("write_through_helper_cycle").complete_paths(),
        Some(["$P0".to_owned()].as_slice()),
        "a write-through helper publishes its write without obscuring the cycle's root permutation"
    );
    assert!(
        !frame("duplicate_transparent_cycle").is_complete(),
        "duplicate_transparent_cycle must remain opaque without an exact bijection"
    );
}

#[test]
fn write_frame_substitutes_stable_local_exclusive_alias_origins() {
    let source = r#"
    data Cell { value: u64; }
    data BorrowCell<'source> { value: &'source mut u64; }
    data Group { cells: [Cell; 2]; }
    data Main {
        value: u64;
        cell: Cell;
        cells: [Cell; 2];
        values: [u64; 2];
        group: Group;
        groups: [Group; 2];
    }

    machine Main::local_alias_acyclic(&mut self) {
        let alias: &mut u64 = &mut self.value;
        alias = 1;
    }

    machine Main::local_alias_member(&mut self) {
        let alias: &mut Cell = &mut self.cell;
        alias.value = 2;
    }

    machine write_local_alias(value: &mut u64) {
        value = 3;
    }

    machine Main::local_alias_call(&mut self) {
        let alias: &mut u64 = &mut self.value;
        write_local_alias(alias);
    }

    machine alias_parameter(value: &mut u64) {
        let alias: &mut u64 = &mut value;
        alias = 4;
    }

    machine Main::call_alias_parameter(&mut self) {
        alias_parameter(&mut self.value);
    }

    machine Main::local_alias_self_loop(&mut self) {
        let alias: &mut u64 = &mut self.value;
        alias = 5;
        transition { _ -> self }
    }

    machine Main::named_alias_acyclic(&mut self) {
        let alias: &mut u64 = &mut self.value;
        transition { _ -> finish(alias) }
        state finish(&mut self, value: &mut u64) {
            value = 6;
        }
    }

    machine Main::named_alias_multihop(&mut self) {
        let alias: &mut u64 = &mut self.value;
        transition { _ -> forward(alias) }
        state forward(&mut self, value: &mut u64) {
            transition { _ -> finish(value) }
        }
        state finish(&mut self, value: &mut u64) {
            value = 7;
        }
    }

    machine Main::named_alias_member(&mut self) {
        let alias: &mut Cell = &mut self.cell;
        transition { _ -> finish(alias) }
        state finish(&mut self, value: &mut Cell) {
            value.value = 8;
        }
    }

    machine alias_parameter_named(value: &mut u64) {
        let alias: &mut u64 = &mut value;
        transition { _ -> finish(alias) }
        state finish(value: &mut u64) {
            value = 9;
        }
    }

    machine Main::call_alias_parameter_named(&mut self) {
        alias_parameter_named(&mut self.value);
    }

    machine Main::local_alias_chain(&mut self) {
        let first: &mut u64 = &mut self.value;
        let second: &mut u64 = &mut first;
        let third: &mut u64 = &mut second;
        third = 10;
    }

    machine Main::local_alias_chain_member_write(&mut self) {
        let first: &mut Cell = &mut self.cell;
        let second: &mut Cell = &mut first;
        second.value = 11;
    }

    machine Main::local_alias_projected_reborrow(&mut self) {
        let first: &mut Cell = &mut self.cell;
        let second: &mut u64 = &mut first.value;
        second = 11;
    }

    machine Main::local_alias_chain_call(&mut self) {
        let first: &mut u64 = &mut self.value;
        let second: &mut u64 = &mut first;
        write_local_alias(second);
    }

    machine alias_parameter_chain(value: &mut u64) {
        let first: &mut u64 = &mut value;
        let second: &mut u64 = &mut first;
        second = 12;
    }

    machine alias_parameter_projection(cell: &mut Cell) {
        let root: &mut Cell = &mut cell;
        let value: &mut u64 = &mut root.value;
        value = 12;
    }

    machine Main::call_alias_parameter_chain(&mut self) {
        alias_parameter_chain(&mut self.value);
    }

    machine Main::call_alias_parameter_projection(&mut self) {
        alias_parameter_projection(&mut self.cell);
    }

    machine Main::local_alias_chain_self_loop(&mut self) {
        let first: &mut u64 = &mut self.value;
        let second: &mut u64 = &mut first;
        second = 13;
        transition { _ -> self }
    }

    machine Main::named_alias_chain(&mut self) {
        let first: &mut u64 = &mut self.value;
        let second: &mut u64 = &mut first;
        transition { _ -> finish(second) }
        state finish(&mut self, value: &mut u64) {
            value = 14;
        }
    }

    machine write_cell(cell: &mut Cell) {
        cell.value = 15;
    }

    machine Main::indexed_alias_fixed(&mut self) {
        let alias: &mut u64 = &mut self.values[0];
        alias = 16;
    }

    machine Main::indexed_alias_dynamic(&mut self, index: u64)
    requires
        index < 2
    {
        let alias: &mut u64 = &mut self.values[index];
        alias = 17;
    }

    machine Main::indexed_alias_member_write(&mut self) {
        let alias: &mut Cell = &mut self.cells[0];
        alias.value = 18;
    }

    machine Main::indexed_alias_call(&mut self) {
        let alias: &mut Cell = &mut self.cells[0];
        write_cell(alias);
    }

    machine indexed_alias_parameter(cells: &mut [Cell; 2]) {
        let alias: &mut Cell = &mut cells[0];
        alias.value = 19;
    }

    machine Main::call_indexed_alias_parameter(&mut self) {
        indexed_alias_parameter(&mut self.cells);
    }

    machine Main::indexed_alias_chain(&mut self) {
        let root: &mut Cell = &mut self.cells[0];
        let alias: &mut Cell = &mut root;
        alias.value = 20;
    }

    machine Main::indexed_alias_projected_reborrow(&mut self) {
        let root: &mut [u64; 2] = &mut self.values;
        let alias: &mut u64 = &mut root[0];
        alias = 20;
    }

    machine Main::coarse_alias_projected_reborrow(&mut self) {
        let root: &mut Cell = &mut self.cells[0];
        let alias: &mut u64 = &mut root.value;
        alias = 20;
    }

    machine Main::member_indexed_alias_projected_reborrow(&mut self) {
        let group: &mut Group = &mut self.group;
        let alias: &mut Cell = &mut group.cells[0];
        alias.value = 20;
    }

    machine Main::direct_member_after_index_alias(&mut self) {
        let alias: &mut u64 = &mut self.cells[0].value;
        alias = 20;
    }

    machine Main::indexed_alias_self_loop(&mut self) {
        let alias: &mut Cell = &mut self.cells[0];
        alias.value = 21;
        transition { _ -> self }
    }

    machine Main::indexed_alias_named(&mut self) {
        let alias: &mut Cell = &mut self.cells[0];
        transition { _ -> finish(alias) }
        state finish(&mut self, cell: &mut Cell) {
            cell.value = 22;
        }
    }

    machine Main::direct_indexed_call(&mut self) {
        write_cell(&mut self.cells[0]);
    }

    machine Main::direct_indexed_transition(&mut self) {
        transition { _ -> finish(&mut self.cells[0]) }
        state finish(&mut self, cell: &mut Cell) {
            cell.value = 23;
        }
    }

    machine mutate_group(group: &mut Group) {
        let alias: &mut Cell = &mut group.cells[0];
        alias.value = 24;
    }

    machine Main::call_indexed_group(&mut self) {
        mutate_group(&mut self.groups[0]);
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let resolver = validation::CallFrameResolver::new(&typed).expect("valid symbol cache");

    let expected = [
        ("Main::local_alias_acyclic", "self.value"),
        ("Main::local_alias_member", "self.cell.value"),
        ("Main::local_alias_call", "self.value"),
        ("alias_parameter", "$P0"),
        ("Main::call_alias_parameter", "self.value"),
        ("Main::local_alias_self_loop", "self.value"),
        ("Main::named_alias_acyclic", "self.value"),
        ("Main::named_alias_multihop", "self.value"),
        ("Main::named_alias_member", "self.cell.value"),
        ("alias_parameter_named", "$P0"),
        ("Main::call_alias_parameter_named", "self.value"),
        ("Main::local_alias_chain", "self.value"),
        ("Main::local_alias_chain_member_write", "self.cell.value"),
        ("Main::local_alias_projected_reborrow", "self.cell.value"),
        ("Main::local_alias_chain_call", "self.value"),
        ("alias_parameter_chain", "$P0"),
        ("alias_parameter_projection", "$P0.value"),
        ("Main::call_alias_parameter_chain", "self.value"),
        ("Main::call_alias_parameter_projection", "self.cell.value"),
        ("Main::local_alias_chain_self_loop", "self.value"),
        ("Main::named_alias_chain", "self.value"),
        ("Main::indexed_alias_fixed", "self.values"),
        ("Main::indexed_alias_dynamic", "self.values"),
        ("Main::indexed_alias_member_write", "self.cells"),
        ("Main::indexed_alias_call", "self.cells"),
        ("indexed_alias_parameter", "$P0"),
        ("Main::call_indexed_alias_parameter", "self.cells"),
        ("Main::indexed_alias_chain", "self.cells"),
        ("Main::indexed_alias_projected_reborrow", "self.values"),
        ("Main::coarse_alias_projected_reborrow", "self.cells"),
        (
            "Main::member_indexed_alias_projected_reborrow",
            "self.group.cells",
        ),
        ("Main::direct_member_after_index_alias", "self.cells"),
        ("Main::indexed_alias_self_loop", "self.cells"),
        ("Main::indexed_alias_named", "self.cells"),
        ("Main::direct_indexed_call", "self.cells"),
        ("Main::direct_indexed_transition", "self.cells"),
        ("mutate_group", "$P0.cells"),
        ("Main::call_indexed_group", "self.groups"),
    ];
    for (name, expected_path) in expected {
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap_or_else(|| panic!("{name} machine"));
        let entry = typed
            .machine_states(machine)
            .first()
            .unwrap_or_else(|| panic!("{name} entry state"));
        assert_eq!(
            resolver
                .inferred_state_write_frame(machine, entry)
                .complete_paths(),
            Some([expected_path.to_owned()].as_slice()),
            "{name} must substitute the local alias back to its visible origin"
        );
    }
}

#[test]
fn write_frame_distinguishes_isolated_and_unrepresentable_local_aliases() {
    let source = r#"
    data Cell { value: u64; }
    data BorrowCell<'source> { value: &'source mut u64; }
    data Main {
        value: u64;
        other: u64;
        cell: Cell;
        cells: [u64; 2];
        cell_items: [Cell; 2];
    }

    machine Main::rebound_alias(&mut self) {
        let alias: &mut u64 = &mut self.value;
        alias = &mut self.other;
        alias = 1;
    }

    machine Main::alias_chain_upstream_rebind(&mut self) {
        let first: &mut u64 = &mut self.value;
        let second: &mut u64 = &mut first;
        first = &mut self.other;
        second = 2;
    }

    machine Main::alias_chain_leaf_rebind(&mut self) {
        let first: &mut u64 = &mut self.value;
        let second: &mut u64 = &mut first;
        second = &mut self.other;
        second = 2;
    }

    machine Main::alias_chain_rebind_from_alias(&mut self) {
        let first: &mut u64 = &mut self.value;
        let second: &mut u64 = &mut self.other;
        second = &mut first;
        second = 2;
    }

    machine Main::local_origin(&mut self) {
        let local: u64 = 0;
        let alias: &mut u64 = &mut local;
        alias = 2;
    }

    machine Main::indexed_local_origin(&mut self) {
        let local: [u64; 2] = [0, 1];
        let alias: &mut u64 = &mut local[0];
        alias = 3;
    }

    machine Main::constrained_local_origin(&mut self) {
        let local: u64 [0..=3] = 0;
        let alias: &mut u64 = &mut local;
        alias = 2;
    }

    machine Main::indexed_constrained_local_origin(&mut self) {
        let local: [u64 [0..=3]; 2] = [0, 1];
        let alias: &mut u64 = &mut local[0];
        alias = 2;
    }

    machine Main::indexed_local_member_after_index(&mut self) {
        let local: [Cell; 2] = [Cell { value: 0 }, Cell { value: 1 }];
        let alias: &mut u64 = &mut local[0].value;
        alias = 3;
    }

    machine reference_bearing_named_local_origin<'source>(source: &'source mut u64) {
        let local: BorrowCell<'source> = BorrowCell { value: source };
        let alias: &mut u64 = &mut local.value;
        alias = 3;
    }

    machine Main::indexed_alias_rebind(&mut self) {
        let alias: &mut u64 = &mut self.cells[0];
        alias = &mut self.other;
        alias = 3;
    }

    machine overwrite_alias_binding(value: &mut u64) {
        value = 5;
    }

    machine return_alias(value: &mut u64) -> &mut u64 {
        value
    }

    machine write_argument(value: &mut u64) {
        value = 8;
    }

    machine return_local_alias(value: &mut u64) -> &mut u64 {
        let alias: &mut u64 = &mut value;
        alias
    }

    machine return_projected_local_alias(cell: &mut Cell) -> &mut u64 {
        let alias: &mut Cell = &mut cell;
        &mut alias.value
    }

    machine return_call_initialized_alias(value: &mut u64) -> &mut u64 {
        let alias: &mut u64 = return_alias(value);
        alias
    }

    machine return_call_initialized_projection(cell: &mut Cell) -> &mut u64 {
        let alias: &mut u64 = project_value(cell);
        alias
    }

    machine return_cells(cells: &mut [u64; 2]) -> &mut [u64; 2] {
        cells
    }

    machine write_then_return_cells(cells: &mut [u64; 2]) -> &mut [u64; 2] {
        cells[0] = 4;
        cells
    }

    machine return_cell_items(cells: &mut [Cell; 2]) -> &mut [Cell; 2] {
        cells
    }

    machine project_cell_value(cells: &mut [Cell; 2]) -> &mut u64 {
        &mut cells[0].value
    }

    machine project_value(cell: &mut Cell) -> &mut u64 {
        &mut cell.value
    }

    machine Main::return_attached_alias(&self, value: &mut u64) -> &mut u64 {
        value
    }

    machine Main::project_attached_value(&self, cell: &mut Cell) -> &mut u64 {
        &mut cell.value
    }

    machine Main::return_attached_receiver(&mut self) -> &mut u64 {
        &mut self.value
    }

    machine Main::return_attached_receiver_via_local_alias(&mut self) -> &mut u64 {
        let alias: &mut u64 = &mut self.value;
        alias
    }

    machine Main::write_then_return_attached_receiver(&mut self) -> &mut u64 {
        self.other = 4;
        &mut self.value
    }

    machine write_then_return(value: &mut u64) -> &mut u64 {
        value = 4;
        value
    }

    machine return_effectful_call_initialized_alias(value: &mut u64) -> &mut u64 {
        let alias: &mut u64 = write_then_return(value);
        alias
    }

    machine return_recursive_alias(value: &mut u64) -> &mut u64 {
        let alias: &mut u64 = return_recursive_alias(value);
        alias
    }

    machine return_with_isolated_scratch(value: &mut u64) -> &mut u64 {
        let mut scratch: u64 = 0;
        scratch = 1;
        value
    }

    machine return_with_reference_scratch<'source>(
        value: &'source mut u64,
        other: &'source mut u64
    ) -> &'source mut u64 {
        let scratch: BorrowCell<'source> = BorrowCell { value: other };
        value
    }

    machine make_scratch() -> u64 {
        0
    }

    machine return_with_call_scratch(value: &mut u64) -> &mut u64 {
        let scratch: u64 = make_scratch();
        value
    }

    machine impure_scratch(value: &mut u64) -> u64 {
        value = 1;
        0
    }

    machine mixed_scratch(first: &mut u64, second: &mut u64) -> u64 {
        first = 1;
        second = 2;
        0
    }

    machine scratch_from(value: u64) -> u64 {
        value
    }

    machine return_with_impure_call_scratch(value: &mut u64) -> &mut u64 {
        let scratch: u64 = impure_scratch(value);
        value
    }

    machine return_with_isolated_write_call_scratch(value: &mut u64) -> &mut u64 {
        let mut prior: u64 = 0;
        let scratch: u64 = impure_scratch(&mut prior);
        value
    }

    machine return_with_mixed_write_call_scratch(value: &mut u64) -> &mut u64 {
        let mut prior: u64 = 0;
        let scratch: u64 = mixed_scratch(&mut prior, value);
        value
    }

    machine return_after_isolated_write_statement_call(value: &mut u64) -> &mut u64 {
        let mut scratch: u64 = 0;
        impure_scratch(&mut scratch);
        value
    }

    machine return_after_mixed_write_statement_call(value: &mut u64) -> &mut u64 {
        let mut scratch: u64 = 0;
        mixed_scratch(&mut scratch, value);
        value
    }

    machine return_with_nested_call_scratch(value: &mut u64) -> &mut u64 {
        let scratch: u64 = scratch_from(make_scratch());
        value
    }

    machine return_after_pure_expression(value: &mut u64) -> &mut u64 {
        value == value;
        value
    }

    machine return_after_recast_write(value: &mut u64) -> &mut u64 {
        let view: &mut f64 = &mut value as &mut f64;
        view = 4.0;
        value
    }

    machine return_after_effectful_recast_write(
        cells: &mut [u64; 2]
    ) -> &mut [u64; 2] {
        let view: &mut f64 = &mut cells[make_index()] as &mut f64;
        view = 4.0;
        cells
    }

    machine return_after_discarded_call(value: &mut u64) -> &mut u64 {
        _ = make_scratch();
        value
    }

    machine return_after_discarded_write<'returned, 'side>(
        returned: &'returned mut u64,
        side: &'side mut u64
    ) -> &'returned mut u64 {
        _ = impure_scratch(side);
        returned
    }

    machine return_after_discarded_reference_call(value: &mut u64) -> &mut u64 {
        _ = return_alias(value);
        value
    }

    machine make_cell() -> Cell {
        Cell { value: 0 }
    }

    machine return_after_discarded_aggregate_call(value: &mut u64) -> &mut u64 {
        _ = make_cell();
        value
    }

    machine recursive_scratch() -> u64 {
        recursive_scratch()
    }

    machine return_after_discarded_recursive_call(value: &mut u64) -> &mut u64 {
        _ = recursive_scratch();
        value
    }

    machine return_after_transparent_call_target_write(value: &mut u64) -> &mut u64 {
        write_then_return(value) = 4;
        value
    }

    machine return_after_opaque_call_target_write(value: &mut u64) -> &mut u64 {
        call_then_return(value) = 4;
        value
    }

    machine make_index() -> u64 [0..=1] {
        0
    }

    machine return_after_hidden_index_call_target_write(
        cells: &mut [u64; 2]
    ) -> &mut [u64; 2] {
        cells[make_index()] = 4;
        cells
    }

    machine return_mutable_local_alias(value: &mut u64) -> &mut u64 {
        let mut alias: &mut u64 = &mut value;
        alias
    }

    machine return_rebound_mutable_local_alias<'first, 'second>(
        first: &'first mut u64,
        second: &'second mut u64
    ) -> &'second mut u64 {
        let mut alias: &mut u64 = &mut first;
        alias = &mut second;
        alias
    }

    machine return_call_rebound_mutable_local_alias<'first, 'second>(
        first: &'first mut u64,
        second: &'second mut u64
    ) -> &'second mut u64 {
        let mut alias: &mut u64 = &mut first;
        alias = call_then_return(second);
        alias
    }

    machine return_rebound_alias<'first, 'second>(
        first: &'first mut u64,
        second: &'second mut u64
    ) -> &'second mut u64 {
        let alias: &mut u64 = &mut first;
        alias = &mut second;
        alias
    }

    machine return_pre_rebind_alias<'first, 'second>(
        first: &'first mut u64,
        second: &'second mut u64
    ) -> &'first mut u64 {
        let alias: &mut u64 = &mut first;
        let prior: &mut u64 = &mut alias;
        alias = &mut second;
        prior
    }

    machine return_call_rebound_alias<'first, 'second>(
        first: &'first mut u64,
        second: &'second mut u64
    ) -> &'second mut u64 {
        let alias: &mut u64 = &mut first;
        alias = return_alias(second);
        alias
    }

    machine call_then_return(value: &mut u64) -> &mut u64 {
        overwrite_alias_binding(&mut value);
        value
    }

    machine opaque_choose<'first, 'second>(
        first: &'first mut u64,
        second: &'second mut u64
    ) -> &'second mut u64 {
        overwrite_alias_binding(&mut first);
        second
    }

    machine return_escaping_call_rebound_alias<'first, 'second>(
        first: &'first mut u64,
        second: &'second mut u64
    ) -> &'second mut u64 {
        let alias: &mut u64 = &mut first;
        alias = call_then_return(second);
        alias
    }

    machine Main::call_rebound_alias(&mut self) {
        let alias: &mut u64 = &mut self.value;
        overwrite_alias_binding(&mut alias);
    }

    machine Main::call_escaped_alias_chain(&mut self) {
        let first: &mut u64 = &mut self.value;
        let second: &mut u64 = &mut first;
        overwrite_alias_binding(&mut second);
    }

    machine Main::call_escaped_indexed_alias(&mut self) {
        let alias: &mut u64 = &mut self.cells[0];
        overwrite_alias_binding(&mut alias);
    }

    machine Main::call_produced_alias_chain(&mut self) {
        let first: &mut u64 = return_alias(&mut self.value);
        let second: &mut u64 = &mut first;
        second = 3;
    }

    machine Main::recast_local_origin(&mut self) {
        let view: &mut f64 = &mut self.value as &mut f64;
        view = 3.0;
    }

    machine Main::effectful_index_recast_origin(&mut self) {
        let view: &mut f64 = &mut self.cells[make_index()] as &mut f64;
        view = 3.0;
    }

    machine Main::transparent_result_statement_argument(&mut self) {
        write_argument(return_alias(&mut self.value));
    }

    machine Main::opaque_result_statement_argument(&mut self) {
        write_argument(opaque_choose(&mut self.value, &mut self.other));
    }

    machine opaque_parameter_result(first: &mut u64, second: &mut u64) {
        write_argument(opaque_choose(first, second));
    }

    machine Main::effectful_index_statement_argument(&mut self) {
        write_argument(&mut self.cells[make_index()]);
    }

    machine Main::nested_call_produced_alias_chain(&mut self) {
        let first: &mut u64 = return_alias(return_alias(&mut self.value));
        let second: &mut u64 = &mut first;
        second = 3;
    }

    machine Main::local_alias_helper_result(&mut self) {
        let alias: &mut u64 = return_local_alias(&mut self.value);
        alias = 3;
    }

    machine Main::projected_local_alias_helper_result(&mut self) {
        let alias: &mut u64 = return_projected_local_alias(&mut self.cell);
        alias = 3;
    }

    machine Main::call_initialized_local_alias_helper_result(&mut self) {
        let alias: &mut u64 = return_call_initialized_alias(&mut self.value);
        alias = 3;
    }

    machine Main::call_initialized_projected_alias_helper_result(&mut self) {
        let alias: &mut u64 = return_call_initialized_projection(&mut self.cell);
        alias = 3;
    }

    machine Main::effectful_call_initialized_alias_helper_result(&mut self) {
        let alias: &mut u64 = return_effectful_call_initialized_alias(&mut self.value);
        alias = 3;
    }

    machine Main::recursive_alias_helper_result(&mut self) {
        let alias: &mut u64 = return_recursive_alias(&mut self.value);
        alias = 3;
    }

    machine Main::isolated_scratch_helper_result(&mut self) {
        let alias: &mut u64 = return_with_isolated_scratch(&mut self.value);
        alias = 3;
    }

    machine Main::reference_scratch_helper_result(&mut self) {
        let alias: &mut u64 =
            return_with_reference_scratch(&mut self.value, &mut self.other);
        alias = 3;
    }

    machine Main::call_scratch_helper_result(&mut self) {
        let alias: &mut u64 = return_with_call_scratch(&mut self.value);
        alias = 3;
    }

    machine Main::impure_call_scratch_helper_result(&mut self) {
        let alias: &mut u64 = return_with_impure_call_scratch(&mut self.value);
        alias = 3;
    }

    machine Main::isolated_write_call_scratch_helper_result(&mut self) {
        let alias: &mut u64 =
            return_with_isolated_write_call_scratch(&mut self.value);
        alias = 3;
    }

    machine Main::mixed_write_call_scratch_helper_result(&mut self) {
        let alias: &mut u64 = return_with_mixed_write_call_scratch(&mut self.value);
        alias = 3;
    }

    machine Main::isolated_write_statement_call_helper_result(&mut self) {
        let alias: &mut u64 =
            return_after_isolated_write_statement_call(&mut self.value);
        alias = 3;
    }

    machine Main::mixed_write_statement_call_helper_result(&mut self) {
        let alias: &mut u64 = return_after_mixed_write_statement_call(&mut self.value);
        alias = 3;
    }

    machine Main::nested_call_scratch_helper_result(&mut self) {
        let alias: &mut u64 = return_with_nested_call_scratch(&mut self.value);
        alias = 3;
    }

    machine Main::pure_expression_helper_result(&mut self) {
        let alias: &mut u64 = return_after_pure_expression(&mut self.value);
        alias = 3;
    }

    machine Main::recast_write_helper_result(&mut self) {
        let alias: &mut u64 = return_after_recast_write(&mut self.value);
        alias = 3;
    }

    machine Main::effectful_recast_write_helper_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_effectful_recast_write(&mut self.cells);
        alias[0] = 3;
    }

    machine Main::discarded_call_helper_result(&mut self) {
        let alias: &mut u64 = return_after_discarded_call(&mut self.value);
        alias = 3;
    }

    machine Main::discarded_write_call_helper_result(&mut self) {
        let alias: &mut u64 =
            return_after_discarded_write(&mut self.value, &mut self.other);
        alias = 3;
    }

    machine Main::discarded_reference_call_helper_result(&mut self) {
        let alias: &mut u64 =
            return_after_discarded_reference_call(&mut self.value);
        alias = 3;
    }

    machine Main::discarded_aggregate_call_helper_result(&mut self) {
        let alias: &mut u64 =
            return_after_discarded_aggregate_call(&mut self.value);
        alias = 3;
    }

    machine Main::discarded_recursive_call_helper_result(&mut self) {
        let alias: &mut u64 =
            return_after_discarded_recursive_call(&mut self.value);
        alias = 3;
    }

    machine Main::transparent_call_target_write_helper_result(&mut self) {
        let alias: &mut u64 =
            return_after_transparent_call_target_write(&mut self.value);
        alias = 3;
    }

    machine Main::opaque_call_target_write_helper_result(&mut self) {
        let alias: &mut u64 = return_after_opaque_call_target_write(&mut self.value);
        alias = 3;
    }

    machine Main::hidden_index_call_target_write_helper_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_hidden_index_call_target_write(&mut self.cells);
        alias[0] = 3;
    }

    machine Main::mutable_local_alias_helper_result(&mut self) {
        let alias: &mut u64 = return_mutable_local_alias(&mut self.value);
        alias = 3;
    }

    machine Main::rebound_mutable_local_alias_helper_result(&mut self) {
        let alias: &mut u64 =
            return_rebound_mutable_local_alias(&mut self.value, &mut self.other);
        alias = 3;
    }

    machine Main::call_rebound_mutable_local_alias_helper_result(&mut self) {
        let alias: &mut u64 =
            return_call_rebound_mutable_local_alias(&mut self.value, &mut self.other);
        alias = 3;
    }

    machine Main::rebound_helper_result(&mut self) {
        let alias: &mut u64 = return_rebound_alias(&mut self.value, &mut self.other);
        alias = 3;
    }

    machine Main::pre_rebind_helper_result(&mut self) {
        let alias: &mut u64 = return_pre_rebind_alias(&mut self.value, &mut self.other);
        alias = 3;
    }

    machine Main::call_rebound_helper_result(&mut self) {
        let alias: &mut u64 = return_call_rebound_alias(&mut self.value, &mut self.other);
        alias = 3;
    }

    machine Main::escaping_call_rebound_helper_result(&mut self) {
        let alias: &mut u64 =
            return_escaping_call_rebound_alias(&mut self.value, &mut self.other);
        alias = 3;
    }

    machine Main::escaping_call_then_result_alias(&mut self) {
        let alias: &mut u64 = call_then_return(&mut self.value);
        alias = 3;
    }

    machine Main::call_produced_indexed_alias(&mut self) {
        let cells: &mut [u64; 2] = return_cells(&mut self.cells);
        let alias: &mut u64 = &mut cells[0];
        alias = 3;
    }

    machine Main::call_produced_member_after_index_alias(&mut self) {
        let alias: &mut u64 = &mut return_cell_items(&mut self.cell_items)[0].value;
        alias = 3;
    }

    machine Main::projected_call_result_alias(&mut self) {
        let alias: &mut u64 = project_cell_value(&mut self.cell_items);
        alias = 3;
    }

    machine Main::exact_projected_call_result_alias(&mut self) {
        let alias: &mut u64 = project_value(&mut self.cell);
        alias = 3;
    }

    machine Main::attached_call_produced_alias(&mut self) {
        let alias: &mut u64 = self.return_attached_alias(&mut self.value);
        alias = 3;
    }

    machine Main::attached_projected_call_result_alias(&mut self) {
        let alias: &mut u64 = self.project_attached_value(&mut self.cell);
        alias = 3;
    }

    machine Main::attached_receiver_result_alias(&mut self) {
        let alias: &mut u64 = self.return_attached_receiver();
        alias = 3;
    }

    machine Main::attached_receiver_local_alias_result(&mut self) {
        let alias: &mut u64 = self.return_attached_receiver_via_local_alias();
        alias = 3;
    }

    machine Main::nontrivial_attached_receiver_result_alias(&mut self) {
        let alias: &mut u64 = self.write_then_return_attached_receiver();
        alias = 3;
    }

    machine Main::nontrivial_call_result_alias(&mut self) {
        let alias: &mut u64 = write_then_return(&mut self.value);
        alias = 3;
    }

    machine Main::nontrivial_call_rebound_alias(&mut self) {
        let alias: &mut u64 = &mut self.value;
        alias = write_then_return(&mut self.other);
        alias = 3;
    }

    machine Main::computed_local_collection_origin(&mut self) {
        let local: [u64; 2] = [0, 1];
        let values: &mut [u64; 2] = return_cells(&mut local);
        let alias: &mut u64 = &mut values[0];
        alias = 3;
    }

    machine Main::effectful_computed_local_collection_origin(&mut self) {
        let local: [u64; 2] = [0, 1];
        let values: &mut [u64; 2] = write_then_return_cells(&mut local);
        let alias: &mut u64 = &mut values[0];
        alias = 3;
    }

    machine Main::named_alias_cycle(&mut self) {
        transition { _ -> cycle() }
        state cycle(&mut self) {
            let first: &mut u64 = &mut self.value;
            let second: &mut u64 = &mut first;
            second = 4;
            transition { _ -> cycle() }
        }
    }

    machine Main::named_indexed_alias_cycle(&mut self) {
        transition { _ -> cycle() }
        state cycle(&mut self) {
            let alias: &mut u64 = &mut self.cells[0];
            alias = 4;
            transition { _ -> cycle() }
        }
    }

    machine Main::named_alias_multistate_cycle(&mut self) {
        let root: &mut u64 = &mut self.value;
        let alias: &mut u64 = &mut root;
        transition { _ -> first(alias) }
        state first(&mut self, value: &mut u64) {
            transition { _ -> second(value) }
        }
        state second(&mut self, value: &mut u64) {
            value = 5;
            transition { _ -> first(value) }
        }
    }

    machine Main::named_alias_downstream_cycle(&mut self) {
        let alias: &mut u64 = &mut self.value;
        transition { _ -> prefix(alias) }
        state prefix(&mut self, value: &mut u64) {
            transition { _ -> cycle(value) }
        }
        state cycle(&mut self, value: &mut u64) {
            value = 6;
            transition { _ -> cycle(value) }
        }
    }

    machine Main::named_stable_rebound_alias_cycle(&mut self) {
        transition { _ -> cycle() }
        state cycle(&mut self) {
            let alias: &mut u64 = &mut self.value;
            alias = &mut self.other;
            alias = 7;
            transition { _ -> cycle() }
        }
    }

    machine parameter_alias_cycle(value: &mut u64) {
        transition { _ -> cycle(value) }
        state cycle(value: &mut u64) {
            let alias: &mut u64 = &mut value;
            alias = 7;
            transition { _ -> cycle(alias) }
        }
    }

    machine duplicate_parameter_alias_cycle(first: &mut u64, second: &mut u64) {
        transition { _ -> cycle(first, second) }
        state cycle(left: &mut u64, right: &mut u64) {
            let alias: &mut u64 = &mut left;
            alias = 7;
            transition { _ -> cycle(alias, alias) }
        }
    }

    machine Main::named_alias_cross_state_local(&mut self) {
        let alias: &mut u64 = &mut self.value;
        transition { _ -> finish() }
        state finish(&mut self) {
            alias = 7;
        }
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let resolver = validation::CallFrameResolver::new(&typed).expect("valid symbol cache");

    for name in [
        "Main::local_origin",
        "Main::indexed_local_origin",
        "Main::constrained_local_origin",
        "Main::indexed_constrained_local_origin",
        "Main::indexed_local_member_after_index",
        "Main::computed_local_collection_origin",
        "Main::effectful_computed_local_collection_origin",
    ] {
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap_or_else(|| panic!("{name} machine"));
        let entry = typed
            .machine_states(machine)
            .first()
            .unwrap_or_else(|| panic!("{name} entry state"));
        assert_eq!(
            resolver
                .inferred_state_write_frame(machine, entry)
                .complete_paths(),
            Some([].as_slice()),
            "{name} writes only through a caller-isolated local origin"
        );
    }

    for (name, expected_path) in [
        ("Main::rebound_alias", "self.other"),
        ("Main::alias_chain_upstream_rebind", "self.value"),
        ("Main::alias_chain_leaf_rebind", "self.other"),
        ("Main::alias_chain_rebind_from_alias", "self.value"),
        ("Main::indexed_alias_rebind", "self.other"),
        ("Main::call_produced_alias_chain", "self.value"),
        ("Main::recast_local_origin", "self.value"),
        ("Main::transparent_result_statement_argument", "self.value"),
        ("Main::nested_call_produced_alias_chain", "self.value"),
        (
            "Main::effectful_call_initialized_alias_helper_result",
            "self.value",
        ),
        ("Main::nontrivial_call_result_alias", "self.value"),
        ("Main::nontrivial_call_rebound_alias", "self.other"),
        ("Main::isolated_scratch_helper_result", "self.value"),
        ("Main::call_scratch_helper_result", "self.value"),
        (
            "Main::isolated_write_call_scratch_helper_result",
            "self.value",
        ),
        ("Main::nested_call_scratch_helper_result", "self.value"),
        (
            "Main::isolated_write_statement_call_helper_result",
            "self.value",
        ),
        ("Main::discarded_call_helper_result", "self.value"),
        (
            "Main::mixed_write_statement_call_helper_result",
            "self.value",
        ),
        ("Main::pure_expression_helper_result", "self.value"),
        ("Main::recast_write_helper_result", "self.value"),
        (
            "Main::transparent_call_target_write_helper_result",
            "self.value",
        ),
        (
            "Main::hidden_index_call_target_write_helper_result",
            "self.cells",
        ),
        ("Main::mutable_local_alias_helper_result", "self.value"),
        (
            "Main::rebound_mutable_local_alias_helper_result",
            "self.other",
        ),
        ("Main::rebound_helper_result", "self.other"),
        ("Main::pre_rebind_helper_result", "self.value"),
        ("Main::call_rebound_helper_result", "self.other"),
        ("Main::local_alias_helper_result", "self.value"),
        (
            "Main::call_initialized_local_alias_helper_result",
            "self.value",
        ),
        (
            "Main::call_initialized_projected_alias_helper_result",
            "self.cell.value",
        ),
        (
            "Main::projected_local_alias_helper_result",
            "self.cell.value",
        ),
        ("Main::call_produced_indexed_alias", "self.cells"),
        (
            "Main::call_produced_member_after_index_alias",
            "self.cell_items",
        ),
        ("Main::projected_call_result_alias", "self.cell_items"),
        ("Main::exact_projected_call_result_alias", "self.cell.value"),
        ("Main::attached_call_produced_alias", "self.value"),
        (
            "Main::attached_projected_call_result_alias",
            "self.cell.value",
        ),
        ("Main::attached_receiver_result_alias", "self.value"),
        ("Main::attached_receiver_local_alias_result", "self.value"),
        ("Main::named_alias_cycle", "self.value"),
        ("Main::named_indexed_alias_cycle", "self.cells"),
        ("Main::named_alias_multistate_cycle", "self.value"),
        ("Main::named_alias_downstream_cycle", "self.value"),
        ("Main::named_stable_rebound_alias_cycle", "self.other"),
        ("parameter_alias_cycle", "$P0"),
    ] {
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap_or_else(|| panic!("{name} machine"));
        let entry = typed
            .machine_states(machine)
            .first()
            .unwrap_or_else(|| panic!("{name} entry state"));
        assert_eq!(
            resolver
                .inferred_state_write_frame(machine, entry)
                .complete_paths(),
            Some([expected_path.to_owned()].as_slice()),
            "{name} must substitute the transparent call result back to its argument origin"
        );
    }

    for name in [
        "Main::opaque_result_statement_argument",
        "opaque_parameter_result",
    ] {
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .expect("opaque nested-result caller");
        let entry = typed.machine_states(machine).first().expect("entry");
        assert!(
            !resolver
                .inferred_state_write_frame(machine, entry)
                .is_complete(),
            "{name}: producer writes do not prove the returned reference's reach; a whole-self fallback can mask untracked parameter origins"
        );
    }

    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::effectful_index_statement_argument")
        .expect("effectful nested index argument caller");
    let entry = typed
        .machine_states(machine)
        .first()
        .expect("effectful nested index argument caller entry state");
    assert_eq!(
        resolver
            .inferred_state_write_frame(machine, entry)
            .complete_paths(),
        Some(["self.cells".to_owned()].as_slice()),
        "a bounded complete index call must coarsen the written argument to its collection"
    );

    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::nontrivial_attached_receiver_result_alias")
        .expect("attached value-write helper caller");
    let entry = typed
        .machine_states(machine)
        .first()
        .expect("attached value-write helper caller entry state");
    assert_eq!(
        resolver
            .inferred_state_write_frame(machine, entry)
            .complete_paths(),
        Some(["self.other".to_owned(), "self.value".to_owned()].as_slice()),
        "the attached helper's own write and returned-alias write must both remain exact"
    );

    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::discarded_write_call_helper_result")
        .expect("discarded primitive write caller");
    let entry = typed
        .machine_states(machine)
        .first()
        .expect("discarded primitive write caller entry state");
    assert_eq!(
        resolver
            .inferred_state_write_frame(machine, entry)
            .complete_paths(),
        Some(["self.other".to_owned(), "self.value".to_owned()].as_slice()),
        "the discarded primitive result must preserve both the call's side write and the returned-place write"
    );

    for name in [
        "reference_bearing_named_local_origin",
        "Main::call_rebound_alias",
        "Main::call_escaped_alias_chain",
        "Main::call_escaped_indexed_alias",
        "Main::effectful_index_recast_origin",
        "Main::recursive_alias_helper_result",
        "Main::reference_scratch_helper_result",
        "Main::impure_call_scratch_helper_result",
        "Main::mixed_write_call_scratch_helper_result",
        "Main::discarded_reference_call_helper_result",
        "Main::discarded_aggregate_call_helper_result",
        "Main::discarded_recursive_call_helper_result",
        "Main::effectful_recast_write_helper_result",
        "Main::opaque_call_target_write_helper_result",
        "Main::call_rebound_mutable_local_alias_helper_result",
        "Main::escaping_call_rebound_helper_result",
        "Main::escaping_call_then_result_alias",
        "duplicate_parameter_alias_cycle",
        "Main::named_alias_cross_state_local",
    ] {
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap_or_else(|| panic!("{name} machine"));
        let entry = typed
            .machine_states(machine)
            .first()
            .unwrap_or_else(|| panic!("{name} entry state"));
        assert!(
            !resolver
                .inferred_state_write_frame(machine, entry)
                .is_complete(),
            "{name} must remain opaque without one stable representable local origin"
        );
    }
}
