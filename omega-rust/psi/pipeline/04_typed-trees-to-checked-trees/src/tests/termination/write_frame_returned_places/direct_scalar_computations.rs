use crate::tests::front_end::typed_program;

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
