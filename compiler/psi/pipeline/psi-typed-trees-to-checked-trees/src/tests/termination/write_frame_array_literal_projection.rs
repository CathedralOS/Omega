use super::*;

#[test]
fn transparent_returned_place_composes_direct_array_literal_index_frames() {
    let source = r#"
    data Main {
        cells: [u64; 2];
        target: u64;
        first: u64;
        second: u64;
    }

    machine compute(value: &mut u64) -> u64 {
        value = 1;
        0
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

    let over_budget = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::over_budget")
        .expect("over-budget machine");
    let entry = typed
        .machine_states(over_budget)
        .first()
        .expect("over-budget entry state");
    assert!(
        !resolver
            .inferred_state_write_frame(over_budget, entry)
            .is_complete(),
        "an outer unary shell plus index projection plus computed element must remain beyond the shared depth-two budget"
    );
}
