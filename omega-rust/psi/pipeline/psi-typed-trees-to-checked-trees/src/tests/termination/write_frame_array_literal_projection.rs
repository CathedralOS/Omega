use super::*;

#[test]
fn transparent_returned_place_composes_direct_array_literal_index_frames() {
    let source = r#"
    data Main {
        cells: [u64; 2];
        target: u64;
        first: u64;
        second: u64;
        third: u64;
        outer_index: u64;
        inner_index: u64;
        reference_first: [u64; 2];
        reference_second: [u64; 2];
    }

    data Cell { value: u64; }

    machine compute(value: &mut u64) -> u64 {
        value = 1;
        0
    }

    machine compute_index(value: &mut u64) -> u64 [0..=1] {
        value = 1;
        0
    }

    machine recursive_compute(value: &mut u64) -> u64 {
        recursive_compute(value)
    }

    machine return_array(value: &mut [u64; 2]) -> &mut [u64; 2] {
        value
    }

    machine return_after_array_literal_index<'cells, 'target, 'first, 'second>(
        cells: &'cells mut [u64; 2],
        target: &'target mut u64,
        first: &'first mut u64,
        second: &'second mut u64
    ) -> &'cells mut [u64; 2] {
        target = [compute(first) + 1, compute(second)][0];
        cells
    }

    machine return_after_wrapped_array_literal_index<'cells, 'target, 'first, 'second>(
        cells: &'cells mut [u64; 2],
        target: &'target mut u64,
        first: &'first mut u64,
        second: &'second mut u64
    ) -> &'cells mut [u64; 2] {
        target = ~([compute(first), compute(second)][0]);
        cells
    }

    machine return_after_over_budget_array_literal_index<'cells, 'target, 'first>(
        cells: &'cells mut [u64; 2],
        target: &'target mut u64,
        first: &'first mut u64
    ) -> &'cells mut [u64; 2] {
        target = ~([compute(first) + 1, 0][0]);
        cells
    }

    machine return_after_nested_array_literal_index<
        'cells, 'target, 'first, 'second, 'third, 'outer, 'inner
    >(
        cells: &'cells mut [u64; 2],
        target: &'target mut u64,
        first: &'first mut u64,
        second: &'second mut u64,
        third: &'third mut u64,
        outer_index: &'outer mut u64,
        inner_index: &'inner mut u64
    ) -> &'cells mut [u64; 2] {
        target = [
            [compute(first), compute(second)],
            [compute(third), 0]
        ][compute_index(outer_index)][compute_index(inner_index)];
        cells
    }

    machine return_after_three_level_array_literal_index<
        'cells, 'target, 'first
    >(
        cells: &'cells mut [u64; 2],
        target: &'target mut u64,
        first: &'first mut u64
    ) -> &'cells mut [u64; 2] {
        target = [
            [[compute(first), 0], [0, 0]],
            [[0, 0], [0, 0]]
        ][0][0][0];
        cells
    }

    machine return_after_opaque_nested_array_literal_index<
        'cells, 'target, 'first
    >(
        cells: &'cells mut [u64; 2],
        target: &'target mut u64,
        first: &'first mut u64
    ) -> &'cells mut [u64; 2] {
        target = [[recursive_compute(first), 0], [0, 0]][0][0];
        cells
    }

    machine return_after_reference_array_literal_index<
        'cells, 'target, 'first, 'second
    >(
        cells: &'cells mut [u64; 2],
        target: &'target mut u64,
        first: &'first mut [u64; 2],
        second: &'second mut [u64; 2]
    ) -> &'cells mut [u64; 2] {
        target = [return_array(first), return_array(second)][0][0];
        cells
    }

    machine return_after_nominal_array_literal_index<'cells, 'target, 'first>(
        cells: &'cells mut [u64; 2], target: &'target mut u64, first: &'first mut u64
    ) -> &'cells mut [u64; 2] {
        target = [Cell { value: compute(first) }][0].value;
        cells
    }

    machine Main::direct(&mut self) {
        let alias: &mut [u64; 2] = return_after_array_literal_index(
            &mut self.cells,
            &mut self.target,
            &mut self.first,
            &mut self.second
        );
        alias[0] = 3;
    }

    machine Main::wrapped(&mut self) {
        let alias: &mut [u64; 2] = return_after_wrapped_array_literal_index(
            &mut self.cells,
            &mut self.target,
            &mut self.first,
            &mut self.second
        );
        alias[0] = 3;
    }

    machine Main::over_budget(&mut self) {
        let alias: &mut [u64; 2] = return_after_over_budget_array_literal_index(
            &mut self.cells,
            &mut self.target,
            &mut self.first
        );
        alias[0] = 3;
    }

    machine Main::nested(&mut self) {
        let alias: &mut [u64; 2] = return_after_nested_array_literal_index(
            &mut self.cells,
            &mut self.target,
            &mut self.first,
            &mut self.second,
            &mut self.third,
            &mut self.outer_index,
            &mut self.inner_index
        );
        alias[0] = 3;
    }

    machine Main::three_level(&mut self) {
        let alias: &mut [u64; 2] = return_after_three_level_array_literal_index(
            &mut self.cells,
            &mut self.target,
            &mut self.first
        );
        alias[0] = 3;
    }

    machine Main::opaque_nested(&mut self) {
        let alias: &mut [u64; 2] = return_after_opaque_nested_array_literal_index(
            &mut self.cells,
            &mut self.target,
            &mut self.first
        );
        alias[0] = 3;
    }

    machine Main::reference_element(&mut self) {
        let alias: &mut [u64; 2] = return_after_reference_array_literal_index(
            &mut self.cells,
            &mut self.target,
            &mut self.reference_first,
            &mut self.reference_second
        );
        alias[0] = 3;
    }

    machine Main::nominal_element(&mut self) {
        let alias: &mut [u64; 2] = return_after_nominal_array_literal_index(
            &mut self.cells, &mut self.target, &mut self.first
        );
        alias[0] = 3;
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let resolver = psi_validation::CallFrameResolver::new(&typed).expect("valid symbol cache");

    for name in ["Main::direct", "Main::wrapped"] {
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
            Some(
                ["self.cells", "self.first", "self.second", "self.target"]
                    .map(str::to_owned)
                    .as_slice()
            ),
            "{name} must retain the returned place and publish every eagerly evaluated array-element write"
        );
    }

    let nested = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::nested")
        .expect("nested-array machine");
    let nested_entry = typed
        .machine_states(nested)
        .first()
        .expect("nested-array entry state");
    assert_eq!(
        resolver
            .inferred_state_write_frame(nested, nested_entry)
            .complete_paths(),
        Some(
            [
                "self.cells",
                "self.first",
                "self.inner_index",
                "self.outer_index",
                "self.second",
                "self.target",
                "self.third",
            ]
            .map(str::to_owned)
            .as_slice()
        ),
        "nested array literals must retain the returned place and publish every eagerly evaluated element and index write"
    );

    let over_budget = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::over_budget")
        .expect("over-budget machine");
    let entry = typed
        .machine_states(over_budget)
        .first()
        .expect("over-budget entry state");
    assert_eq!(
        resolver
            .inferred_state_write_frame(over_budget, entry)
            .complete_paths(),
        Some(
            ["self.cells", "self.first", "self.target"]
                .map(str::to_owned)
                .as_slice()
        ),
        "outer computations and literal projections retain each evaluated element write"
    );

    let three_level = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::three_level")
        .expect("three-level array machine");
    let entry = typed
        .machine_states(three_level)
        .first()
        .expect("entry state");
    assert_eq!(
        resolver
            .inferred_state_write_frame(three_level, entry)
            .complete_paths(),
        Some(
            ["self.cells", "self.first", "self.target"]
                .map(str::to_owned)
                .as_slice()
        ),
        "finite nested array projections preserve their complete frame"
    );

    for (name, reason) in [
        (
            "Main::opaque_nested",
            "an opaque recursive element call must fence the nested array literal",
        ),
        (
            "Main::reference_element",
            "a reference-valued array element call must remain a returned-place fence",
        ),
        (
            "Main::nominal_element",
            "a projected array cannot invent contextual nominal element type evidence",
        ),
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
            "{reason}"
        );
    }
}
