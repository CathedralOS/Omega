use crate::tests::front_end::typed_program;

#[test]
fn transparent_returned_place_accepts_finite_direct_scalar_computations() {
    let source = r#"
    data Pair {
        first: u64;
        second: u64;
    }

    data Main {
        cells: [u64; 2];
        value: u64;
        other: u64;
        pair: Pair;
    }

    machine compute(value: &mut u64) -> u64 {
        value = 1;
        0
    }

    machine recursive_value() -> u64 {
        recursive_value()
    }

    machine return_pair(pair: &mut Pair) -> &mut Pair {
        pair
    }

    machine make_pair(value: &mut u64) -> Pair {
        value = 1;
        Pair { first: 1, second: 2 }
    }

    machine make_cells(value: &mut u64) -> [u64; 2] {
        value = 1;
        [1, 2]
    }

    machine return_after_computed_scalar<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        cells[0] = compute(value) + 1;
        cells
    }

    machine return_after_nested_computed_scalar<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        cells[0] = ~(compute(value) + 1);
        cells
    }

    machine return_after_parameter_computed_scalar<'cells, 'target, 'source>(
        cells: &'cells mut [u64; 2],
        target: &'target mut u64,
        source: &'source mut u64
    ) -> &'cells mut [u64; 2] {
        target = ~(compute(source) + 1);
        cells
    }

    machine return_after_parameter_three_computed_scalar<'cells, 'target, 'source>(
        cells: &'cells mut [u64; 2],
        target: &'target mut u64,
        source: &'source mut u64
    ) -> &'cells mut [u64; 2] {
        target = ~~~compute(source);
        cells
    }

    machine return_after_projected_computed_scalar<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        cells[0] = make_pair(value).first + 1;
        cells
    }

    machine return_after_indexed_computed_scalar<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        cells[0] = make_cells(value)[0] + 1;
        cells
    }

    machine return_after_three_computed_scalar<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        cells[0] = ~~~compute(value);
        cells
    }

    machine return_after_four_computed_scalar<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        cells[0] = ~~~~compute(value);
        cells
    }

    machine return_after_five_computed_scalar<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        cells[0] = ~~~~~compute(value);
        cells
    }

    machine return_after_six_computed_scalar<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        cells[0] = ~~~~~~compute(value);
        cells
    }

    machine return_after_seven_computed_scalar<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        cells[0] = ~~~~~~~compute(value);
        cells
    }

    machine return_after_eight_computed_scalar<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        cells[0] = ~~~~~~~~compute(value);
        cells
    }

    machine return_after_nine_computed_scalar<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        cells[0] = ~~~~~~~~~compute(value);
        cells
    }

    machine return_after_ten_computed_scalar<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        cells[0] = ~~~~~~~~~~compute(value);
        cells
    }

    machine return_after_eleven_computed_scalar<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        cells[0] = ~~~~~~~~~~~compute(value);
        cells
    }

    machine return_after_twelve_computed_scalar<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        cells[0] = ~~~~~~~~~~~~compute(value);
        cells
    }

    machine return_after_thirteen_computed_scalar<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        cells[0] = ~~~~~~~~~~~~~compute(value);
        cells
    }

    machine return_after_fourteen_computed_scalar<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        cells[0] = ~~~~~~~~~~~~~~compute(value);
        cells
    }

    machine return_after_fifteen_computed_scalar<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        cells[0] = ~~~~~~~~~~~~~~~compute(value);
        cells
    }

    machine return_after_sixteen_computed_scalar<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        cells[0] = ~~~~~~~~~~~~~~~~compute(value);
        cells
    }

    machine return_after_seventeen_computed_scalar<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        cells[0] = ~~~~~~~~~~~~~~~~~compute(value);
        cells
    }

    machine return_after_eighteen_computed_scalar<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        cells[0] = ~~~~~~~~~~~~~~~~~~compute(value);
        cells
    }

    machine return_after_nineteen_computed_scalar<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        cells[0] = ~~~~~~~~~~~~~~~~~~~compute(value);
        cells
    }

    machine return_after_twenty_computed_scalar<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        cells[0] = ~~~~~~~~~~~~~~~~~~~~compute(value);
        cells
    }

    machine return_after_twenty_one_computed_scalar<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        cells[0] = ~~~~~~~~~~~~~~~~~~~~~compute(value);
        cells
    }

    machine return_after_twenty_two_computed_scalar<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        cells[0] = ~~~~~~~~~~~~~~~~~~~~~~compute(value);
        cells
    }

    machine return_after_twenty_three_computed_scalar<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        cells[0] = ~~~~~~~~~~~~~~~~~~~~~~~compute(value);
        cells
    }

    machine return_after_twenty_four_computed_scalar<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        cells[0] = ~~~~~~~~~~~~~~~~~~~~~~~~compute(value);
        cells
    }

    machine return_after_twenty_five_computed_scalar<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        cells[0] = ~~~~~~~~~~~~~~~~~~~~~~~~~compute(value);
        cells
    }

    machine return_after_twenty_six_computed_scalar<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        cells[0] = ~~~~~~~~~~~~~~~~~~~~~~~~~~compute(value);
        cells
    }

    machine return_after_twenty_seven_computed_scalar<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        cells[0] = ~~~~~~~~~~~~~~~~~~~~~~~~~~~compute(value);
        cells
    }

    machine return_after_twenty_eight_computed_scalar<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        cells[0] = ~~~~~~~~~~~~~~~~~~~~~~~~~~~~compute(value);
        cells
    }

    machine return_after_twenty_nine_computed_scalar<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        cells[0] = ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~compute(value);
        cells
    }

    machine return_after_thirty_computed_scalar<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        cells[0] = ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~compute(value);
        cells
    }

    machine return_after_thirty_one_computed_scalar<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        cells[0] = ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~compute(value);
        cells
    }

    machine return_after_thirty_two_computed_scalar<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        cells[0] = ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~compute(value);
        cells
    }

    machine return_after_thirty_three_computed_scalar<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        cells[0] = ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~compute(value);
        cells
    }

    machine return_after_thirty_four_computed_scalar<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        cells[0] = ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~compute(value);
        cells
    }

    machine return_after_three_projected_computed_scalar<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        cells[0] = ~(make_pair(value).first + 1);
        cells
    }

    machine return_after_binding_reborrow_computed_scalar<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        cells[0] = compute(&mut value) + 1;
        cells
    }

    machine return_after_recursive_computed_scalar(
        cells: &mut [u64; 2]
    ) -> &mut [u64; 2] {
        cells[0] = recursive_value() + 1;
        cells
    }

    machine return_after_reference_projection_computed_scalar<'cells, 'pair>(
        cells: &'cells mut [u64; 2],
        pair: &'pair mut Pair
    ) -> &'cells mut [u64; 2] {
        cells[0] = return_pair(pair).first + 1;
        cells
    }

    machine Main::computed_scalar_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_computed_scalar(&mut self.cells, &mut self.value);
        alias[0] = 2;
    }

    machine Main::nested_computed_scalar_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_nested_computed_scalar(&mut self.cells, &mut self.value);
        alias[0] = 2;
    }

    machine Main::parameter_computed_scalar_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_parameter_computed_scalar(
            &mut self.cells,
            &mut self.value,
            &mut self.other
        );
        alias[0] = 2;
    }

    machine Main::parameter_three_computed_scalar_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_parameter_three_computed_scalar(
            &mut self.cells,
            &mut self.value,
            &mut self.other
        );
        alias[0] = 2;
    }

    machine Main::projected_computed_scalar_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_projected_computed_scalar(&mut self.cells, &mut self.value);
        alias[0] = 2;
    }

    machine Main::indexed_computed_scalar_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_indexed_computed_scalar(&mut self.cells, &mut self.value);
        alias[0] = 2;
    }

    machine Main::three_computed_scalar_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_three_computed_scalar(&mut self.cells, &mut self.value);
        alias[0] = 2;
    }

    machine Main::four_computed_scalar_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_four_computed_scalar(&mut self.cells, &mut self.value);
        alias[0] = 2;
    }

    machine Main::five_computed_scalar_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_five_computed_scalar(&mut self.cells, &mut self.value);
        alias[0] = 2;
    }

    machine Main::six_computed_scalar_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_six_computed_scalar(&mut self.cells, &mut self.value);
        alias[0] = 2;
    }

    machine Main::seven_computed_scalar_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_seven_computed_scalar(&mut self.cells, &mut self.value);
        alias[0] = 2;
    }

    machine Main::eight_computed_scalar_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_eight_computed_scalar(&mut self.cells, &mut self.value);
        alias[0] = 2;
    }

    machine Main::nine_computed_scalar_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_nine_computed_scalar(&mut self.cells, &mut self.value);
        alias[0] = 2;
    }

    machine Main::ten_computed_scalar_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_ten_computed_scalar(&mut self.cells, &mut self.value);
        alias[0] = 2;
    }

    machine Main::eleven_computed_scalar_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_eleven_computed_scalar(&mut self.cells, &mut self.value);
        alias[0] = 2;
    }

    machine Main::twelve_computed_scalar_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_twelve_computed_scalar(&mut self.cells, &mut self.value);
        alias[0] = 2;
    }

    machine Main::thirteen_computed_scalar_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_thirteen_computed_scalar(&mut self.cells, &mut self.value);
        alias[0] = 2;
    }

    machine Main::fourteen_computed_scalar_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_fourteen_computed_scalar(&mut self.cells, &mut self.value);
        alias[0] = 2;
    }

    machine Main::fifteen_computed_scalar_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_fifteen_computed_scalar(&mut self.cells, &mut self.value);
        alias[0] = 2;
    }

    machine Main::sixteen_computed_scalar_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_sixteen_computed_scalar(&mut self.cells, &mut self.value);
        alias[0] = 2;
    }

    machine Main::seventeen_computed_scalar_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_seventeen_computed_scalar(&mut self.cells, &mut self.value);
        alias[0] = 2;
    }

    machine Main::eighteen_computed_scalar_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_eighteen_computed_scalar(&mut self.cells, &mut self.value);
        alias[0] = 2;
    }

    machine Main::nineteen_computed_scalar_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_nineteen_computed_scalar(&mut self.cells, &mut self.value);
        alias[0] = 2;
    }

    machine Main::twenty_computed_scalar_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_twenty_computed_scalar(&mut self.cells, &mut self.value);
        alias[0] = 2;
    }

    machine Main::twenty_one_computed_scalar_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_twenty_one_computed_scalar(&mut self.cells, &mut self.value);
        alias[0] = 2;
    }

    machine Main::twenty_two_computed_scalar_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_twenty_two_computed_scalar(&mut self.cells, &mut self.value);
        alias[0] = 2;
    }

    machine Main::twenty_three_computed_scalar_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_twenty_three_computed_scalar(&mut self.cells, &mut self.value);
        alias[0] = 2;
    }

    machine Main::twenty_four_computed_scalar_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_twenty_four_computed_scalar(&mut self.cells, &mut self.value);
        alias[0] = 2;
    }

    machine Main::twenty_five_computed_scalar_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_twenty_five_computed_scalar(&mut self.cells, &mut self.value);
        alias[0] = 2;
    }

    machine Main::twenty_six_computed_scalar_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_twenty_six_computed_scalar(&mut self.cells, &mut self.value);
        alias[0] = 2;
    }

    machine Main::twenty_seven_computed_scalar_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_twenty_seven_computed_scalar(&mut self.cells, &mut self.value);
        alias[0] = 2;
    }

    machine Main::twenty_eight_computed_scalar_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_twenty_eight_computed_scalar(&mut self.cells, &mut self.value);
        alias[0] = 2;
    }

    machine Main::twenty_nine_computed_scalar_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_twenty_nine_computed_scalar(&mut self.cells, &mut self.value);
        alias[0] = 2;
    }

    machine Main::thirty_computed_scalar_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_thirty_computed_scalar(&mut self.cells, &mut self.value);
        alias[0] = 2;
    }

    machine Main::thirty_one_computed_scalar_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_thirty_one_computed_scalar(&mut self.cells, &mut self.value);
        alias[0] = 2;
    }

    machine Main::thirty_two_computed_scalar_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_thirty_two_computed_scalar(&mut self.cells, &mut self.value);
        alias[0] = 2;
    }

    machine Main::thirty_three_computed_scalar_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_thirty_three_computed_scalar(&mut self.cells, &mut self.value);
        alias[0] = 2;
    }

    machine Main::thirty_four_computed_scalar_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_thirty_four_computed_scalar(&mut self.cells, &mut self.value);
        alias[0] = 2;
    }

    machine Main::three_projected_computed_scalar_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_three_projected_computed_scalar(
            &mut self.cells,
            &mut self.value
        );
        alias[0] = 2;
    }

    machine Main::binding_reborrow_computed_scalar_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_binding_reborrow_computed_scalar(
            &mut self.cells,
            &mut self.value
        );
        alias[0] = 2;
    }

    machine Main::recursive_computed_scalar_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_recursive_computed_scalar(&mut self.cells);
        alias[0] = 2;
    }

    machine Main::reference_projection_computed_scalar_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_reference_projection_computed_scalar(
            &mut self.cells,
            &mut self.pair
        );
        alias[0] = 2;
    }
    "#;

    let typed = typed_program(source);
    let resolver = validation::CallFrameResolver::new(&typed).expect("valid symbol cache");

    for name in [
        "Main::computed_scalar_result",
        "Main::nested_computed_scalar_result",
        "Main::projected_computed_scalar_result",
        "Main::indexed_computed_scalar_result",
        "Main::three_computed_scalar_result",
        "Main::four_computed_scalar_result",
        "Main::five_computed_scalar_result",
        "Main::six_computed_scalar_result",
        "Main::seven_computed_scalar_result",
        "Main::eight_computed_scalar_result",
        "Main::nine_computed_scalar_result",
        "Main::ten_computed_scalar_result",
        "Main::eleven_computed_scalar_result",
        "Main::twelve_computed_scalar_result",
        "Main::thirteen_computed_scalar_result",
        "Main::fourteen_computed_scalar_result",
        "Main::fifteen_computed_scalar_result",
        "Main::sixteen_computed_scalar_result",
        "Main::seventeen_computed_scalar_result",
        "Main::eighteen_computed_scalar_result",
        "Main::nineteen_computed_scalar_result",
        "Main::twenty_computed_scalar_result",
        "Main::twenty_one_computed_scalar_result",
        "Main::twenty_two_computed_scalar_result",
        "Main::twenty_three_computed_scalar_result",
        "Main::twenty_four_computed_scalar_result",
        "Main::twenty_five_computed_scalar_result",
        "Main::twenty_six_computed_scalar_result",
        "Main::twenty_seven_computed_scalar_result",
        "Main::twenty_eight_computed_scalar_result",
        "Main::twenty_nine_computed_scalar_result",
        "Main::thirty_computed_scalar_result",
        "Main::thirty_one_computed_scalar_result",
        "Main::thirty_two_computed_scalar_result",
        "Main::thirty_three_computed_scalar_result",
        "Main::thirty_four_computed_scalar_result",
        "Main::three_projected_computed_scalar_result",
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
            Some(["self.cells".to_owned(), "self.value".to_owned()].as_slice()),
            "{name} must publish the computed call write without losing the returned place"
        );
    }

    for name in [
        "Main::parameter_computed_scalar_result",
        "Main::parameter_three_computed_scalar_result",
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
                ["self.cells", "self.other", "self.value"]
                    .map(str::to_owned)
                    .as_slice()
            ),
            "{name} must admit the computed value through its primitive mutable-reference target"
        );
    }

    for name in [
        "Main::binding_reborrow_computed_scalar_result",
        "Main::recursive_computed_scalar_result",
        "Main::reference_projection_computed_scalar_result",
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
            "{name} must reject invalid call-result provenance or incomplete frames"
        );
    }
}

#[test]
fn transparent_returned_place_accepts_finite_fixed_array_assignment_values() {
    let source = r#"
    data Pair {
        first: u64;
        second: u64;
    }

    data BorrowCell<'source> {
        value: &'source mut u64;
    }

    data Main {
        cells: [u64; 2];
        values: [u64; 2];
        matrix: [[u64; 2]; 2];
        cube: [[[u64; 1]; 1]; 1];
        hypercube: [[[[u64; 1]; 1]; 1]; 1];
        first: u64;
        second: u64;
    }

    data ReferenceMain<'storage> {
        cells: [u64; 2];
        target: [BorrowCell<'storage>; 1];
        source: &'storage mut u64;
    }

    machine compute(value: &mut u64) -> u64 {
        value = 1;
        0
    }

    machine identity(value: u64) -> u64 {
        value
    }

    machine recursive_value() -> u64 {
        recursive_value()
    }

    machine return_reference<'value>(value: &'value mut u64) -> &'value mut u64 {
        value
    }

    machine make_pair(value: &mut u64) -> Pair {
        value = 2;
        Pair { first: 0, second: 1 }
    }

    machine return_after_array_values<'cells, 'target, 'first, 'second>(
        cells: &'cells mut [u64; 2],
        target: &'target mut [u64; 2],
        first: &'first mut u64,
        second: &'second mut u64
    ) -> &'cells mut [u64; 2] {
        target = [
            identity(identity(identity(compute(first)))),
            compute(second)
        ];
        cells
    }

    machine return_after_nested_array_values<'cells, 'target, 'first, 'second>(
        cells: &'cells mut [u64; 2],
        target: &'target mut [[u64; 2]; 2],
        first: &'first mut u64,
        second: &'second mut u64
    ) -> &'cells mut [u64; 2] {
        target = [
            [~(compute(first) + 1), 0],
            [make_pair(second).first, 1]
        ];
        cells
    }

    machine return_after_three_array_levels<'cells, 'target, 'first>(
        cells: &'cells mut [u64; 2],
        target: &'target mut [[[u64; 1]; 1]; 1],
        first: &'first mut u64
    ) -> &'cells mut [u64; 2] {
        target = [[[compute(first)]]];
        cells
    }

    machine return_after_four_array_levels<'cells, 'target, 'first>(
        cells: &'cells mut [u64; 2],
        target: &'target mut [[[[u64; 1]; 1]; 1]; 1],
        first: &'first mut u64
    ) -> &'cells mut [u64; 2] {
        target = [[[[compute(first)]]]];
        cells
    }

    machine return_after_three_array_computations<'cells, 'target, 'first>(
        cells: &'cells mut [u64; 2],
        target: &'target mut [u64; 2],
        first: &'first mut u64
    ) -> &'cells mut [u64; 2] {
        target = [(~compute(first) as u64) + 1, 0];
        cells
    }

    machine return_after_five_array_calls<'cells, 'target, 'first>(
        cells: &'cells mut [u64; 2],
        target: &'target mut [u64; 2],
        first: &'first mut u64
    ) -> &'cells mut [u64; 2] {
        target = [
            identity(identity(identity(identity(compute(first))))),
            0
        ];
        cells
    }

    machine return_after_array_binding_reborrow<'cells, 'target, 'first>(
        cells: &'cells mut [u64; 2],
        target: &'target mut [u64; 2],
        first: &'first mut u64
    ) -> &'cells mut [u64; 2] {
        target = [compute(&mut first), 0];
        cells
    }

    machine return_after_recursive_array_value<'cells, 'target>(
        cells: &'cells mut [u64; 2],
        target: &'target mut [u64; 2]
    ) -> &'cells mut [u64; 2] {
        target = [recursive_value(), 0];
        cells
    }

    machine return_after_reference_array<'cells, 'target, 'value>(
        cells: &'cells mut [u64; 2],
        target: &'target mut [BorrowCell<'value>; 1],
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        target = [BorrowCell { value: return_reference(value) }];
        cells
    }

    machine Main::array_value_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_array_values(
            &mut self.cells,
            &mut self.values,
            &mut self.first,
            &mut self.second
        );
        alias[0] = 3;
    }

    machine Main::nested_array_value_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_nested_array_values(
            &mut self.cells,
            &mut self.matrix,
            &mut self.first,
            &mut self.second
        );
        alias[0] = 3;
    }

    machine Main::three_array_levels_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_three_array_levels(
            &mut self.cells,
            &mut self.cube,
            &mut self.first
        );
        alias[0] = 3;
    }

    machine Main::four_array_levels_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_four_array_levels(
            &mut self.cells,
            &mut self.hypercube,
            &mut self.first
        );
        alias[0] = 3;
    }

    machine Main::three_array_computations_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_three_array_computations(
            &mut self.cells,
            &mut self.values,
            &mut self.first
        );
        alias[0] = 3;
    }

    machine Main::five_array_calls_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_five_array_calls(
            &mut self.cells,
            &mut self.values,
            &mut self.first
        );
        alias[0] = 3;
    }

    machine Main::array_binding_reborrow_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_array_binding_reborrow(
            &mut self.cells,
            &mut self.values,
            &mut self.first
        );
        alias[0] = 3;
    }

    machine Main::recursive_array_value_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_recursive_array_value(
            &mut self.cells,
            &mut self.values
        );
        alias[0] = 3;
    }

    machine ReferenceMain::reference_array_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_reference_array(
            &mut self.cells,
            &mut self.target,
            self.source
        );
        alias[0] = 3;
    }
    "#;

    let typed = typed_program(source);
    let resolver = validation::CallFrameResolver::new(&typed).expect("valid symbol cache");

    for (name, expected_paths) in [
        (
            "Main::array_value_result",
            vec!["self.cells", "self.first", "self.second", "self.values"],
        ),
        (
            "Main::nested_array_value_result",
            vec!["self.cells", "self.first", "self.matrix", "self.second"],
        ),
        (
            "Main::three_array_levels_result",
            vec!["self.cells", "self.cube", "self.first"],
        ),
        (
            "Main::five_array_calls_result",
            vec!["self.cells", "self.first", "self.values"],
        ),
        (
            "Main::four_array_levels_result",
            vec!["self.cells", "self.first", "self.hypercube"],
        ),
        (
            "Main::three_array_computations_result",
            vec!["self.cells", "self.first", "self.values"],
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
        assert_eq!(
            resolver
                .inferred_state_write_frame(machine, entry)
                .complete_paths(),
            Some(
                expected_paths
                    .into_iter()
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
                    .as_slice()
            ),
            "{name} must publish every fixed-array element call write"
        );
    }

    for name in [
        "Main::array_binding_reborrow_result",
        "Main::recursive_array_value_result",
        "ReferenceMain::reference_array_result",
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
            "{name} must reject reference-bearing elements, binding reborrows, or recursive calls"
        );
    }
}

#[test]
fn transparent_returned_place_composes_mixed_aggregate_assignment_values() {
    let source = r#"
    data Pair {
        first: u64;
        second: u64;
    }

    data GenericPair<T> {
        first: T;
        second: u64;
    }

    data Choice {
        tag: u64;
        case Values(first: u64, second: u64);
        case Empty;
    }

    data RecordWithArray {
        values: [u64; 2];
        marker: u64;
    }

    data ChoiceWithArray {
        tag: u64;
        case Values(values: [u64; 2], marker: u64);
        case Empty;
    }

    data RecordWithPairs {
        pairs: [Pair; 1];
        marker: u64;
    }

    data BorrowCell<'source> {
        value: &'source mut u64;
    }

    data ReferenceHolder<'source> {
        values: [BorrowCell<'source>; 1];
    }

    data Main {
        cells: [u64; 2];
        pairs: [Pair; 2];
        choices: [Choice; 1];
        record: RecordWithArray;
        choice_record: ChoiceWithArray;
        record_array: [RecordWithArray; 1];
        pair_record: RecordWithPairs;
        generic_pairs: [GenericPair<u64>; 1];
        first: u64;
        second: u64;
        marker: u64;
    }

    data ReferenceMain<'storage> {
        cells: [u64; 2];
        holder: ReferenceHolder<'storage>;
        source: &'storage mut u64;
    }

    machine compute(value: &mut u64) -> u64 {
        value = 1;
        0
    }

    machine identity(value: u64) -> u64 {
        value
    }

    machine make_pair(value: &mut u64) -> Pair {
        value = 2;
        Pair { first: 0, second: 1 }
    }

    machine recursive_value() -> u64 {
        recursive_value()
    }

    machine return_reference<'value>(value: &'value mut u64) -> &'value mut u64 {
        value
    }

    machine return_after_array_records<'cells, 'target, 'first, 'second>(
        cells: &'cells mut [u64; 2],
        target: &'target mut [Pair; 2],
        first: &'first mut u64,
        second: &'second mut u64
    ) -> &'cells mut [u64; 2] {
        target = [
            Pair {
                first: identity(identity(identity(compute(first)))),
                second: ~(compute(second) + 1)
            },
            Pair { first: 0, second: 1 }
        ];
        cells
    }

    machine return_after_array_cases<'cells, 'target, 'first, 'second>(
        cells: &'cells mut [u64; 2],
        target: &'target mut [Choice; 1],
        first: &'first mut u64,
        second: &'second mut u64
    ) -> &'cells mut [u64; 2] {
        target = [Choice::Values {
            tag: 0,
            first: compute(first),
            second: make_pair(second).first
        }];
        cells
    }

    machine return_after_record_array<'cells, 'target, 'first, 'second, 'marker>(
        cells: &'cells mut [u64; 2],
        target: &'target mut RecordWithArray,
        first: &'first mut u64,
        second: &'second mut u64,
        marker: &'marker mut u64
    ) -> &'cells mut [u64; 2] {
        target = RecordWithArray {
            values: [
                identity(identity(identity(compute(first)))),
                ~(compute(second) + 1)
            ],
            marker: compute(marker)
        };
        cells
    }

    machine return_after_case_array<'cells, 'target, 'first, 'second, 'marker>(
        cells: &'cells mut [u64; 2],
        target: &'target mut ChoiceWithArray,
        first: &'first mut u64,
        second: &'second mut u64,
        marker: &'marker mut u64
    ) -> &'cells mut [u64; 2] {
        target = ChoiceWithArray::Values {
            tag: 0,
            values: [compute(first), make_pair(second).first],
            marker: compute(marker)
        };
        cells
    }

    machine return_after_array_record_array<'cells, 'target, 'value>(
        cells: &'cells mut [u64; 2],
        target: &'target mut [RecordWithArray; 1],
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        target = [RecordWithArray {
            values: [compute(value), 0],
            marker: 0
        }];
        cells
    }

    machine return_after_record_array_record<'cells, 'target, 'value>(
        cells: &'cells mut [u64; 2],
        target: &'target mut RecordWithPairs,
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        target = RecordWithPairs {
            pairs: [Pair { first: compute(value), second: 0 }],
            marker: 0
        };
        cells
    }

    machine return_after_generic_array_record<'cells, 'target, 'value>(
        cells: &'cells mut [u64; 2],
        target: &'target mut [GenericPair<u64>; 1],
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        target = [GenericPair { first: compute(value), second: 0 }];
        cells
    }

    machine return_after_mixed_reborrow<'cells, 'target, 'value>(
        cells: &'cells mut [u64; 2],
        target: &'target mut RecordWithArray,
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        target = RecordWithArray {
            values: [compute(&mut value), 0],
            marker: 0
        };
        cells
    }

    machine return_after_mixed_recursion<'cells, 'target>(
        cells: &'cells mut [u64; 2],
        target: &'target mut [Pair; 2]
    ) -> &'cells mut [u64; 2] {
        target = [
            Pair { first: recursive_value(), second: 0 },
            Pair { first: 0, second: 1 }
        ];
        cells
    }

    machine return_after_reference_record_array<'cells, 'target, 'value>(
        cells: &'cells mut [u64; 2],
        target: &'target mut ReferenceHolder<'value>,
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        target = ReferenceHolder {
            values: [BorrowCell { value: return_reference(value) }]
        };
        cells
    }

    machine Main::array_records_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_array_records(
            &mut self.cells,
            &mut self.pairs,
            &mut self.first,
            &mut self.second
        );
        alias[0] = 3;
    }

    machine Main::array_cases_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_array_cases(
            &mut self.cells,
            &mut self.choices,
            &mut self.first,
            &mut self.second
        );
        alias[0] = 3;
    }

    machine Main::record_array_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_record_array(
            &mut self.cells,
            &mut self.record,
            &mut self.first,
            &mut self.second,
            &mut self.marker
        );
        alias[0] = 3;
    }

    machine Main::case_array_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_case_array(
            &mut self.cells,
            &mut self.choice_record,
            &mut self.first,
            &mut self.second,
            &mut self.marker
        );
        alias[0] = 3;
    }

    machine Main::array_record_array_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_array_record_array(
            &mut self.cells,
            &mut self.record_array,
            &mut self.first
        );
        alias[0] = 3;
    }

    machine Main::record_array_record_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_record_array_record(
            &mut self.cells,
            &mut self.pair_record,
            &mut self.first
        );
        alias[0] = 3;
    }

    machine Main::generic_array_record_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_generic_array_record(
            &mut self.cells,
            &mut self.generic_pairs,
            &mut self.first
        );
        alias[0] = 3;
    }

    machine Main::mixed_reborrow_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_mixed_reborrow(
            &mut self.cells,
            &mut self.record,
            &mut self.first
        );
        alias[0] = 3;
    }

    machine Main::mixed_recursion_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_mixed_recursion(
            &mut self.cells,
            &mut self.pairs
        );
        alias[0] = 3;
    }

    machine ReferenceMain::reference_record_array_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_reference_record_array(
            &mut self.cells,
            &mut self.holder,
            self.source
        );
        alias[0] = 3;
    }
    "#;

    let typed = typed_program(source);
    let resolver = validation::CallFrameResolver::new(&typed).expect("valid symbol cache");

    for (name, expected_paths) in [
        (
            "Main::array_records_result",
            vec!["self.cells", "self.first", "self.pairs", "self.second"],
        ),
        (
            "Main::array_cases_result",
            vec!["self.cells", "self.choices", "self.first", "self.second"],
        ),
        (
            "Main::record_array_result",
            vec![
                "self.cells",
                "self.first",
                "self.marker",
                "self.record",
                "self.second",
            ],
        ),
        (
            "Main::case_array_result",
            vec![
                "self.cells",
                "self.choice_record",
                "self.first",
                "self.marker",
                "self.second",
            ],
        ),
        (
            "Main::array_record_array_result",
            vec!["self.cells", "self.first", "self.record_array"],
        ),
        (
            "Main::record_array_record_result",
            vec!["self.cells", "self.first", "self.pair_record"],
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
        assert_eq!(
            resolver
                .inferred_state_write_frame(machine, entry)
                .complete_paths(),
            Some(
                expected_paths
                    .into_iter()
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
                    .as_slice()
            ),
            "{name} must preserve the mixed aggregate relation and every nested call write"
        );
    }

    for name in [
        "Main::generic_array_record_result",
        "Main::mixed_reborrow_result",
        "Main::mixed_recursion_result",
        "ReferenceMain::reference_record_array_result",
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
            "{name} must reject unproven aggregate types, binding reborrows, or recursive calls"
        );
    }
}

#[test]
fn transparent_returned_place_accepts_direct_concrete_literal_member_values() {
    let source = r#"
    data Pair {
        first: u64;
        second: u64;
    }

    data NestedPair {
        pair: Pair;
        marker: u64;
    }

    data DeepPair {
        nested: NestedPair;
        marker: u64;
    }

    data RecordWithArray {
        values: [u64; 2];
        marker: u64;
    }

    data RecordWithPairs {
        values: [Pair; 1];
        marker: u64;
    }

    data GenericPair<T> {
        first: T;
        second: u64;
    }

    data Choice {
        tag: u64;
        case Values(first: u64, second: u64);
        case Empty;
    }

    data BorrowHolder<'source> {
        value: &'source mut u64;
        marker: u64;
    }

    data Main {
        cells: [u64; 2];
        target: u64;
        first: u64;
        second: u64;
        third: u64;
    }

    data ReferenceMain<'storage> {
        cells: [u64; 2];
        target: u64;
        source: &'storage mut u64;
        other: u64;
    }

    machine compute(value: &mut u64) -> u64 {
        value = 1;
        0
    }

    machine identity(value: u64) -> u64 {
        value
    }

    machine recursive_value() -> u64 {
        recursive_value()
    }

    machine return_reference<'value>(value: &'value mut u64) -> &'value mut u64 {
        value
    }

    machine return_after_record_literal_member<'cells, 'target, 'first, 'second>(
        cells: &'cells mut [u64; 2],
        target: &'target mut u64,
        first: &'first mut u64,
        second: &'second mut u64
    ) -> &'cells mut [u64; 2] {
        target = (Pair {
            first: identity(identity(identity(compute(first)))),
            second: compute(second)
        }).first;
        cells
    }

    machine return_after_case_literal_member<'cells, 'target, 'first, 'second>(
        cells: &'cells mut [u64; 2],
        target: &'target mut u64,
        first: &'first mut u64,
        second: &'second mut u64
    ) -> &'cells mut [u64; 2] {
        target = (Choice::Values {
            tag: identity(compute(first)),
            first: compute(second),
            second: 0
        }).tag;
        cells
    }

    machine return_after_wrapped_literal_member<'cells, 'target, 'value>(
        cells: &'cells mut [u64; 2],
        target: &'target mut u64,
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        target = ~(Pair { first: compute(value), second: 0 }).first;
        cells
    }

    machine return_after_third_shell_literal_member<'cells, 'target, 'value>(
        cells: &'cells mut [u64; 2],
        target: &'target mut u64,
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        target = ~(~(Pair { first: compute(value), second: 0 }).first);
        cells
    }

    machine return_after_computed_literal_field<'cells, 'target, 'value, 'other>(
        cells: &'cells mut [u64; 2],
        target: &'target mut u64,
        value: &'value mut u64,
        other: &'other mut u64
    ) -> &'cells mut [u64; 2] {
        target = (Pair {
            first: identity(identity(identity(compute(value)))) + 1,
            second: compute(other) + 2
        }).first;
        cells
    }

    machine return_after_wrapped_computed_literal_field<'cells, 'target, 'value>(
        cells: &'cells mut [u64; 2],
        target: &'target mut u64,
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        target = ~((Pair { first: compute(value) + 1, second: 0 }).first);
        cells
    }

    machine return_after_nested_literal_member<'cells, 'target, 'value, 'other>(
        cells: &'cells mut [u64; 2],
        target: &'target mut u64,
        value: &'value mut u64,
        other: &'other mut u64
    ) -> &'cells mut [u64; 2] {
        target = (NestedPair {
            pair: Pair {
                first: identity(identity(identity(compute(value)))) + 1,
                second: 0
            },
            marker: compute(other) + 2
        }).marker;
        cells
    }

    machine return_after_third_aggregate_literal_member<'cells, 'target, 'value>(
        cells: &'cells mut [u64; 2],
        target: &'target mut u64,
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        target = (DeepPair {
            nested: NestedPair {
                pair: Pair { first: compute(value), second: 0 },
                marker: 0
            },
            marker: 0
        }).marker;
        cells
    }

    machine return_after_array_field_literal_member<'cells, 'target, 'first, 'second, 'third>(
        cells: &'cells mut [u64; 2],
        target: &'target mut u64,
        first: &'first mut u64,
        second: &'second mut u64,
        third: &'third mut u64
    ) -> &'cells mut [u64; 2] {
        target = (RecordWithArray {
            values: [
                compute(first) + 1,
                identity(identity(identity(compute(second))))
            ],
            marker: ~compute(third)
        }).marker;
        cells
    }

    machine return_after_array_field_two_shells<'cells, 'target, 'value>(
        cells: &'cells mut [u64; 2],
        target: &'target mut u64,
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        target = (RecordWithArray {
            values: [~(compute(value) + 1), 0],
            marker: 0
        }).marker;
        cells
    }

    machine return_after_record_array_record_literal_member<'cells, 'target, 'value>(
        cells: &'cells mut [u64; 2],
        target: &'target mut u64,
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        target = (RecordWithPairs {
            values: [Pair { first: compute(value), second: 0 }],
            marker: 0
        }).marker;
        cells
    }

    machine return_after_generic_literal_member<'cells, 'target, 'value>(
        cells: &'cells mut [u64; 2],
        target: &'target mut u64,
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        target = (GenericPair { first: compute(value), second: 0 }).second;
        cells
    }

    machine return_after_reborrow_literal_member<'cells, 'target, 'value>(
        cells: &'cells mut [u64; 2],
        target: &'target mut u64,
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        target = (Pair { first: compute(&mut value), second: 0 }).first;
        cells
    }

    machine return_after_recursive_literal_member<'cells, 'target>(
        cells: &'cells mut [u64; 2],
        target: &'target mut u64
    ) -> &'cells mut [u64; 2] {
        target = (Pair { first: recursive_value(), second: 0 }).first;
        cells
    }

    machine return_after_reference_literal_member<'cells, 'target, 'source, 'other>(
        cells: &'cells mut [u64; 2],
        target: &'target mut u64,
        source: &'source mut u64,
        other: &'other mut u64
    ) -> &'cells mut [u64; 2] {
        target = (BorrowHolder {
            value: return_reference(source),
            marker: compute(other)
        }).marker;
        cells
    }

    machine Main::record_literal_member_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_record_literal_member(
            &mut self.cells,
            &mut self.target,
            &mut self.first,
            &mut self.second
        );
        alias[0] = 3;
    }

    machine Main::case_literal_member_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_case_literal_member(
            &mut self.cells,
            &mut self.target,
            &mut self.first,
            &mut self.second
        );
        alias[0] = 3;
    }

    machine Main::wrapped_literal_member_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_wrapped_literal_member(
            &mut self.cells,
            &mut self.target,
            &mut self.first
        );
        alias[0] = 3;
    }

    machine Main::computed_literal_field_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_computed_literal_field(
            &mut self.cells,
            &mut self.target,
            &mut self.first,
            &mut self.second
        );
        alias[0] = 3;
    }

    machine Main::wrapped_computed_literal_field_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_wrapped_computed_literal_field(
            &mut self.cells,
            &mut self.target,
            &mut self.first
        );
        alias[0] = 3;
    }

    machine Main::third_shell_literal_member_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_third_shell_literal_member(
            &mut self.cells,
            &mut self.target,
            &mut self.first
        );
        alias[0] = 3;
    }

    machine Main::nested_literal_member_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_nested_literal_member(
            &mut self.cells,
            &mut self.target,
            &mut self.first,
            &mut self.second
        );
        alias[0] = 3;
    }

    machine Main::third_aggregate_literal_member_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_third_aggregate_literal_member(
            &mut self.cells,
            &mut self.target,
            &mut self.first
        );
        alias[0] = 3;
    }

    machine Main::array_field_literal_member_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_array_field_literal_member(
            &mut self.cells,
            &mut self.target,
            &mut self.first,
            &mut self.second,
            &mut self.third
        );
        alias[0] = 3;
    }

    machine Main::array_field_two_shells_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_array_field_two_shells(
            &mut self.cells,
            &mut self.target,
            &mut self.first
        );
        alias[0] = 3;
    }

    machine Main::record_array_record_literal_member_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_record_array_record_literal_member(
            &mut self.cells,
            &mut self.target,
            &mut self.first
        );
        alias[0] = 3;
    }

    machine Main::generic_literal_member_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_generic_literal_member(
            &mut self.cells,
            &mut self.target,
            &mut self.first
        );
        alias[0] = 3;
    }

    machine Main::reborrow_literal_member_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_reborrow_literal_member(
            &mut self.cells,
            &mut self.target,
            &mut self.first
        );
        alias[0] = 3;
    }

    machine Main::recursive_literal_member_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_recursive_literal_member(
            &mut self.cells,
            &mut self.target
        );
        alias[0] = 3;
    }

    machine ReferenceMain::reference_literal_member_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_reference_literal_member(
            &mut self.cells,
            &mut self.target,
            self.source,
            &mut self.other
        );
        alias[0] = 3;
    }
    "#;

    let typed = typed_program(source);
    let resolver = validation::CallFrameResolver::new(&typed).expect("valid symbol cache");

    for name in [
        "Main::record_literal_member_result",
        "Main::case_literal_member_result",
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
                ["self.cells", "self.first", "self.second", "self.target"]
                    .map(str::to_owned)
                    .as_slice()
            ),
            "{name} must retain the returned place and publish every literal-field call write"
        );
    }

    let wrapped = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::wrapped_literal_member_result")
        .expect("wrapped literal member machine");
    let wrapped_entry = typed
        .machine_states(wrapped)
        .first()
        .expect("wrapped literal member entry state");
    assert_eq!(
        resolver
            .inferred_state_write_frame(wrapped, wrapped_entry)
            .complete_paths(),
        Some(
            ["self.cells", "self.first", "self.target"]
                .map(str::to_owned)
                .as_slice()
        ),
        "one outer computation shell must retain the returned place and literal-field call write"
    );

    let computed = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::computed_literal_field_result")
        .expect("computed literal field machine");
    let computed_entry = typed
        .machine_states(computed)
        .first()
        .expect("computed literal field entry state");
    assert_eq!(
        resolver
            .inferred_state_write_frame(computed, computed_entry)
            .complete_paths(),
        Some(
            ["self.cells", "self.first", "self.second", "self.target"]
                .map(str::to_owned)
                .as_slice()
        ),
        "the member and field computations must publish every eagerly evaluated field write"
    );

    let nested = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::nested_literal_member_result")
        .expect("nested literal member machine");
    let nested_entry = typed
        .machine_states(nested)
        .first()
        .expect("nested literal member entry state");
    assert_eq!(
        resolver
            .inferred_state_write_frame(nested, nested_entry)
            .complete_paths(),
        Some(
            ["self.cells", "self.first", "self.second", "self.target"]
                .map(str::to_owned)
                .as_slice()
        ),
        "the literal member must retain every nested aggregate and computation write"
    );

    let array_field = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::array_field_literal_member_result")
        .expect("array-field literal member machine");
    let array_field_entry = typed
        .machine_states(array_field)
        .first()
        .expect("array-field literal member entry state");
    assert_eq!(
        resolver
            .inferred_state_write_frame(array_field, array_field_entry)
            .complete_paths(),
        Some(
            [
                "self.cells",
                "self.first",
                "self.second",
                "self.target",
                "self.third",
            ]
            .map(str::to_owned)
            .as_slice()
        ),
        "the literal member must publish every nested array element and sibling write"
    );

    for name in [
        "Main::wrapped_computed_literal_field_result",
        "Main::third_shell_literal_member_result",
        "Main::third_aggregate_literal_member_result",
        "Main::array_field_two_shells_result",
        "Main::record_array_record_literal_member_result",
    ] {
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap_or_else(|| panic!("{name} machine"));
        let entry = typed.machine_states(machine).first().expect("entry state");
        assert_eq!(
            resolver
                .inferred_state_write_frame(machine, entry)
                .complete_paths(),
            Some(
                ["self.cells", "self.first", "self.target"]
                    .map(str::to_owned)
                    .as_slice()
            ),
            "{name} must publish writes through finite aggregate and computation nesting"
        );
    }

    for name in [
        "Main::generic_literal_member_result",
        "Main::reborrow_literal_member_result",
        "Main::recursive_literal_member_result",
        "ReferenceMain::reference_literal_member_result",
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
            "{name} must remain opaque outside the direct concrete-literal member cohort"
        );
    }
}
