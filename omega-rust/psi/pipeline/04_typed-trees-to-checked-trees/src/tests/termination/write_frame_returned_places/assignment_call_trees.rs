use crate::tests::front_end::typed_program;

#[test]
fn transparent_returned_place_composes_finite_assignment_call_trees() {
    let source = r#"
    data Main {
        target_value: u64;
        source_value: u64;
        cells: [u64; 2];
    }

    machine write_index(value: &mut u64) -> u64 [0..=1] {
        value = 1;
        0
    }

    machine identity_index(index: u64 [0..=1]) -> u64 [0..=1] {
        index
    }

    machine compute(value: &mut u64) -> u64 {
        value = 2;
        0
    }

    machine identity(value: u64) -> u64 {
        value
    }

    machine return_after_composed_assignment<'cells, 'target, 'source>(
        cells: &'cells mut [u64; 2],
        target_value: &'target mut u64,
        source_value: &'source mut u64
    ) -> &'cells mut [u64; 2] {
        cells[identity_index(write_index(target_value))] =
            identity(compute(source_value));
        cells
    }

    machine return_after_slice_view_composed_assignment<'cells, 'target, 'source>(
        cells: &'cells mut [u64; 2],
        target_value: &'target mut u64,
        source_value: &'source mut u64
    ) -> &'cells mut [u64; 2] {
        cells.as_mut_slice()[identity_index(write_index(target_value))] =
            identity(compute(source_value));
        cells
    }

    machine return_after_deep_slice_view_target_assignment<'cells, 'target, 'source>(
        cells: &'cells mut [u64; 2],
        target_value: &'target mut u64,
        source_value: &'source mut u64
    ) -> &'cells mut [u64; 2] {
        cells.as_mut_slice()[
            identity_index(identity_index(write_index(target_value)))
        ] = identity(compute(source_value));
        cells
    }

    machine return_after_recursive_slice_view_value_assignment<'cells, 'target>(
        cells: &'cells mut [u64; 2],
        target_value: &'target mut u64
    ) -> &'cells mut [u64; 2] {
        cells.as_mut_slice()[identity_index(write_index(target_value))] =
            recursive_value();
        cells
    }

    machine return_after_deep_target_assignment<'cells, 'target, 'source>(
        cells: &'cells mut [u64; 2],
        target_value: &'target mut u64,
        source_value: &'source mut u64
    ) -> &'cells mut [u64; 2] {
        cells[identity_index(identity_index(write_index(target_value)))] =
            identity(compute(source_value));
        cells
    }

    machine return_after_deep_value_assignment<'cells, 'target, 'source>(
        cells: &'cells mut [u64; 2],
        target_value: &'target mut u64,
        source_value: &'source mut u64
    ) -> &'cells mut [u64; 2] {
        cells[identity_index(write_index(target_value))] =
            identity(identity(identity(compute(source_value))));
        cells
    }

    machine return_after_reborrow_target_assignment<'cells, 'target, 'source>(
        cells: &'cells mut [u64; 2],
        target_value: &'target mut u64,
        source_value: &'source mut u64
    ) -> &'cells mut [u64; 2] {
        cells[identity_index(write_index(&mut target_value))] =
            identity(compute(source_value));
        cells
    }

    machine return_after_reborrow_value_assignment<'cells, 'target, 'source>(
        cells: &'cells mut [u64; 2],
        target_value: &'target mut u64,
        source_value: &'source mut u64
    ) -> &'cells mut [u64; 2] {
        cells[identity_index(write_index(target_value))] =
            identity(compute(&mut source_value));
        cells
    }

    machine Main::composed_assignment_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_composed_assignment(
            &mut self.cells,
            &mut self.target_value,
            &mut self.source_value
        );
        alias[0] = 3;
    }

    machine Main::slice_view_composed_assignment_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_slice_view_composed_assignment(
            &mut self.cells,
            &mut self.target_value,
            &mut self.source_value
        );
        alias[0] = 3;
    }

    machine Main::deep_slice_view_target_assignment_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_deep_slice_view_target_assignment(
            &mut self.cells,
            &mut self.target_value,
            &mut self.source_value
        );
        alias[0] = 3;
    }

    machine Main::recursive_slice_view_value_assignment_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_recursive_slice_view_value_assignment(
            &mut self.cells,
            &mut self.target_value
        );
        alias[0] = 3;
    }

    machine Main::deep_target_assignment_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_deep_target_assignment(
            &mut self.cells,
            &mut self.target_value,
            &mut self.source_value
        );
        alias[0] = 3;
    }

    machine Main::deep_value_assignment_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_deep_value_assignment(
            &mut self.cells,
            &mut self.target_value,
            &mut self.source_value
        );
        alias[0] = 3;
    }

    machine Main::reborrow_target_assignment_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_reborrow_target_assignment(
            &mut self.cells,
            &mut self.target_value,
            &mut self.source_value
        );
        alias[0] = 3;
    }

    machine Main::reborrow_value_assignment_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_reborrow_value_assignment(
            &mut self.cells,
            &mut self.target_value,
            &mut self.source_value
        );
        alias[0] = 3;
    }
    "#;

    let typed = typed_program(source);
    let resolver = validation::CallFrameResolver::new(&typed).expect("valid symbol cache");

    for name in [
        "Main::composed_assignment_result",
        "Main::slice_view_composed_assignment_result",
        "Main::deep_value_assignment_result",
        "Main::deep_target_assignment_result",
        "Main::deep_slice_view_target_assignment_result",
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
            Some(
                ["self.cells", "self.source_value", "self.target_value"]
                    .map(str::to_owned)
                    .as_slice()
            ),
            "{name} target and value call trees must independently publish their writes"
        );
    }

    for name in [
        "Main::reborrow_target_assignment_result",
        "Main::reborrow_value_assignment_result",
        "Main::recursive_slice_view_value_assignment_result",
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
            "{name} must remain opaque when either assignment side exceeds its rail"
        );
    }
}
