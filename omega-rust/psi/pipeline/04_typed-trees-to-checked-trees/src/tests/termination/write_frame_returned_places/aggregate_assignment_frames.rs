use crate::tests::front_end::typed_program;

#[test]
fn aggregate_assignment_frames_require_exact_array_context() {
    let source = r#"
    data Cell { value: u64; }
    data OtherCell { value: u64; }
    data Main { cells: [Cell; 2]; result: u64; source: u64; }
    machine compute(value: &mut u64) -> u64 { value = 1; 0 }
    machine assign<'cells, 'result, 'source>(
        cells: &'cells mut [Cell; 2], result: &'result mut u64, source: &'source mut u64
    ) -> &'result mut u64 {
        cells = $ELEMENTS;
        result
    }
    machine Main::entry(&mut self) {
        let alias: &mut u64 = assign(&mut self.cells, &mut self.result, &mut self.source);
        alias = 3;
    }
    "#;
    for (elements, complete) in [
        ("[Cell { value: compute(source) }, Cell { value: 0 }]", true),
        ("[Cell { value: compute(source) }]", false),
        (
            "[OtherCell { value: compute(source) }, OtherCell { value: 0 }]",
            false,
        ),
    ] {
        let source = source.replace("$ELEMENTS", elements);
        let typed = typed_program(&source);
        let resolver = validation::CallFrameResolver::new(&typed).expect("valid symbol cache");
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Main::entry")
            .expect("caller machine");
        let state = typed.machine_states(machine).first().expect("entry state");
        let frame = resolver.inferred_state_write_frame(machine, state);
        if complete {
            assert_eq!(
                frame.complete_paths(),
                Some(
                    ["self.cells", "self.result", "self.source"]
                        .map(str::to_owned)
                        .as_slice()
                ),
                "the exact contextual array publishes its writes"
            );
        } else {
            assert!(
                !frame.is_complete(),
                "{elements} cannot satisfy the declared array context"
            );
        }
    }
}

#[test]
fn finite_mixed_assignment_shapes_retain_all_leaf_effects() {
    let mut declarations = "data Layer0 { value: u64; audit: u64; }\n".to_owned();
    let computation = (0..64).fold("compute(first)".to_owned(), |value, _| {
        format!("~({value})")
    });
    let mut literal = format!("Layer0 {{ value: {computation}, audit: compute(second) }}");
    for level in 1..=8 {
        declarations.push_str(&format!(
            "data Layer{level} {{ children: [Layer{}; 1]; audit: u64; }}\n",
            level - 1
        ));
        literal = format!("Layer{level} {{ children: [{literal}], audit: compute(second) }}");
    }
    let source = r#"
    $DECLARATIONS
    data Main { aggregate: Layer8; result: u64; first: u64; second: u64; }
    machine compute(value: &mut u64) -> u64 { value = 1; 0 }
    machine recursive_compute(value: &mut u64) -> u64 { recursive_compute(value) }
    machine assign<'target, 'result, 'first, 'second>(
        target: &'target mut Layer8, result: &'result mut u64,
        first: &'first mut u64, second: &'second mut u64
    ) -> &'result mut u64 {
        target = $LITERAL;
        result
    }
    machine project<'result, 'first, 'second>(
        result: &'result mut u64, first: &'first mut u64, second: &'second mut u64
    ) -> &'result mut u64 {
        result = ($LITERAL).audit;
        result
    }
    machine reborrow<'target, 'result, 'first, 'second>(
        target: &'target mut Layer8, result: &'result mut u64,
        first: &'first mut u64, second: &'second mut u64
    ) -> &'result mut u64 {
        target = $REBIND;
        result
    }
    machine recursive<'target, 'result, 'first, 'second>(
        target: &'target mut Layer8, result: &'result mut u64,
        first: &'first mut u64, second: &'second mut u64
    ) -> &'result mut u64 {
        target = $RECURSIVE;
        result
    }
    machine Main::assigned(&mut self) {
        let alias: &mut u64 = assign(
            &mut self.aggregate, &mut self.result, &mut self.first, &mut self.second
        );
        alias = 3;
    }
    machine Main::projected(&mut self) {
        let alias: &mut u64 = project(&mut self.result, &mut self.first, &mut self.second);
        alias = 3;
    }
    machine Main::reborrowed(&mut self) {
        let alias: &mut u64 = reborrow(
            &mut self.aggregate, &mut self.result, &mut self.first, &mut self.second
        );
        alias = 3;
    }
    machine Main::recursive(&mut self) {
        let alias: &mut u64 = recursive(
            &mut self.aggregate, &mut self.result, &mut self.first, &mut self.second
        );
        alias = 3;
    }
    "#
    .replace("$DECLARATIONS", &declarations)
    .replace("$LITERAL", &literal)
    .replace(
        "$REBIND",
        &literal.replace("compute(first)", "compute(&mut first)"),
    )
    .replace(
        "$RECURSIVE",
        &literal.replace("compute(second)", "recursive_compute(second)"),
    );
    let typed = typed_program(&source);
    let resolver = validation::CallFrameResolver::new(&typed).expect("valid symbol cache");
    for (name, expected) in [
        (
            "Main::assigned",
            Some(vec![
                "self.aggregate",
                "self.first",
                "self.result",
                "self.second",
            ]),
        ),
        (
            "Main::projected",
            Some(vec!["self.first", "self.result", "self.second"]),
        ),
        ("Main::reborrowed", None),
        ("Main::recursive", None),
    ] {
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap_or_else(|| panic!("{name} machine"));
        let state = typed.machine_states(machine).first().expect("entry state");
        let expected =
            expected.map(|paths| paths.into_iter().map(str::to_owned).collect::<Vec<_>>());
        assert_eq!(
            resolver
                .inferred_state_write_frame(machine, state)
                .complete_paths(),
            expected.as_deref(),
            "{name} must retain every eager leaf effect and reject an invalid sibling"
        );
    }
}

#[test]
fn transparent_returned_place_accepts_complete_indexed_target_calls() {
    let source = r#"
    data Bucket {
        cells: [u64; 2];
    }

    data Cell {
        value: u64;
    }

    data CellBucket {
        cells: [Cell; 2];
    }

    data GridBucket {
        rows: [[u64; 2]; 2];
    }

    data Main {
        value: u64;
        other_value: u64;
        result: u64;
        cells: [u64; 2];
        matrix: [[u64; 2]; 2];
        bucket: Bucket;
        cell_bucket: CellBucket;
        grid_bucket: GridBucket;
    }

    machine make_index() -> u64 [0..=1] {
        0
    }

    machine write_index(value: &mut u64) -> u64 [0..=1] {
        value = 1;
        0
    }

    machine identity_index(index: u64 [0..=1]) -> u64 [0..=1] {
        index
    }

    machine recursive_index() -> u64 [0..=1] {
        recursive_index()
    }

    machine return_cells(cells: &mut [u64; 2]) -> &mut [u64; 2] {
        cells
    }

    machine recursive_cells(cells: &mut [u64; 2]) -> &mut [u64; 2] {
        recursive_cells(cells)
    }

    machine return_bucket(bucket: &mut Bucket) -> &mut Bucket {
        bucket
    }

    machine recursive_bucket(bucket: &mut Bucket) -> &mut Bucket {
        recursive_bucket(bucket)
    }

    machine return_cell_bucket(bucket: &mut CellBucket) -> &mut CellBucket {
        bucket
    }

    machine recursive_cell_bucket(bucket: &mut CellBucket) -> &mut CellBucket {
        recursive_cell_bucket(bucket)
    }

    machine return_grid_bucket(bucket: &mut GridBucket) -> &mut GridBucket {
        bucket
    }

    machine recursive_grid_bucket(bucket: &mut GridBucket) -> &mut GridBucket {
        recursive_grid_bucket(bucket)
    }

    machine Main::return_attached_cells(&mut self) -> &mut [u64; 2] {
        &mut self.cells
    }

    machine Main::recursive_attached_cells(&mut self) -> &mut [u64; 2] {
        self.recursive_attached_cells()
    }

    machine return_after_index_target(cells: &mut [u64; 2]) -> &mut [u64; 2] {
        cells[make_index()] = 1;
        cells
    }

    machine return_after_nested_index_target<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        cells[identity_index(write_index(value))] = 1;
        cells
    }

    machine return_after_alias_index_target<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        let alias: &mut [u64; 2] = cells;
        alias[identity_index(write_index(value))] = 1;
        cells
    }

    machine return_after_helper_result_index_target<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        return_cells(cells)[identity_index(write_index(value))] = 1;
        cells
    }

    machine return_after_slice_view_index_target<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        cells.as_mut_slice()[identity_index(write_index(value))] = 1;
        cells
    }

    machine return_after_deep_slice_view_index_target(
        cells: &mut [u64; 2]
    ) -> &mut [u64; 2] {
        cells.as_mut_slice()[
            identity_index(identity_index(make_index()))
        ] = 1;
        cells
    }

    machine return_after_recursive_slice_view_index_target(
        cells: &mut [u64; 2]
    ) -> &mut [u64; 2] {
        cells.as_mut_slice()[recursive_index()] = 1;
        cells
    }

    machine return_after_alias_slice_view_index_target<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        let alias: &mut [u64; 2] = cells;
        alias.as_mut_slice()[identity_index(write_index(value))] = 1;
        cells
    }

    machine return_after_deep_alias_slice_view_index_target(
        cells: &mut [u64; 2]
    ) -> &mut [u64; 2] {
        let alias: &mut [u64; 2] = cells;
        alias.as_mut_slice()[
            identity_index(identity_index(make_index()))
        ] = 1;
        cells
    }

    machine return_after_recursive_alias_slice_view_index_target(
        cells: &mut [u64; 2]
    ) -> &mut [u64; 2] {
        let alias: &mut [u64; 2] = cells;
        alias.as_mut_slice()[recursive_index()] = 1;
        cells
    }

    machine return_after_member_alias_slice_view_index_target<
        'bucket, 'result, 'value
    >(
        bucket: &'bucket mut Bucket,
        result: &'result mut u64,
        value: &'value mut u64
    ) -> &'result mut u64 {
        let alias: &mut [u64; 2] = &mut bucket.cells;
        alias.as_mut_slice()[identity_index(write_index(value))] = 1;
        result
    }

    machine return_after_deep_member_alias_slice_view_index_target<
        'bucket, 'result
    >(
        bucket: &'bucket mut Bucket,
        result: &'result mut u64
    ) -> &'result mut u64 {
        let alias: &mut [u64; 2] = &mut bucket.cells;
        alias.as_mut_slice()[
            identity_index(identity_index(make_index()))
        ] = 1;
        result
    }

    machine return_after_recursive_member_alias_slice_view_index_target<
        'bucket, 'result
    >(
        bucket: &'bucket mut Bucket,
        result: &'result mut u64
    ) -> &'result mut u64 {
        let alias: &mut [u64; 2] = &mut bucket.cells;
        alias.as_mut_slice()[recursive_index()] = 1;
        result
    }

    machine return_after_slice_view_repeated_index_target<
        'matrix, 'first, 'second
    >(
        matrix: &'matrix mut [[u64; 2]; 2],
        first: &'first mut u64,
        second: &'second mut u64
    ) -> &'matrix mut [[u64; 2]; 2] {
        matrix.as_mut_slice()[write_index(first)][write_index(second)] = 1;
        matrix
    }

    machine return_after_deep_slice_view_repeated_index_target(
        matrix: &mut [[u64; 2]; 2]
    ) -> &mut [[u64; 2]; 2] {
        matrix.as_mut_slice()[
            identity_index(identity_index(make_index()))
        ][make_index()] = 1;
        matrix
    }

    machine return_after_recursive_slice_view_repeated_index_target(
        matrix: &mut [[u64; 2]; 2]
    ) -> &mut [[u64; 2]; 2] {
        matrix.as_mut_slice()[recursive_index()][make_index()] = 1;
        matrix
    }

    machine return_after_helper_slice_view_index_target<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        return_cells(cells).as_mut_slice()[
            identity_index(write_index(value))
        ] = 1;
        cells
    }

    machine return_after_deep_helper_slice_view_index_target(
        cells: &mut [u64; 2]
    ) -> &mut [u64; 2] {
        return_cells(cells).as_mut_slice()[
            identity_index(identity_index(make_index()))
        ] = 1;
        cells
    }

    machine return_after_recursive_helper_slice_view_index_target(
        cells: &mut [u64; 2]
    ) -> &mut [u64; 2] {
        recursive_cells(cells).as_mut_slice()[make_index()] = 1;
        cells
    }

    machine return_after_recursive_helper_index_target(
        cells: &mut [u64; 2]
    ) -> &mut [u64; 2] {
        recursive_cells(cells)[make_index()] = 1;
        cells
    }

    machine return_after_projected_helper_index_target<'bucket, 'result, 'value>(
        bucket: &'bucket mut Bucket,
        result: &'result mut u64,
        value: &'value mut u64
    ) -> &'result mut u64 {
        return_bucket(bucket).cells[
            identity_index(write_index(value))
        ] = 1;
        result
    }

    machine return_after_deep_projected_helper_index_target<'bucket, 'result>(
        bucket: &'bucket mut Bucket,
        result: &'result mut u64
    ) -> &'result mut u64 {
        return_bucket(bucket).cells[
            identity_index(identity_index(make_index()))
        ] = 1;
        result
    }

    machine return_after_recursive_projected_helper_index_target<'bucket, 'result>(
        bucket: &'bucket mut Bucket,
        result: &'result mut u64
    ) -> &'result mut u64 {
        recursive_bucket(bucket).cells[make_index()] = 1;
        result
    }

    machine return_after_projected_helper_slice_view_index_target<
        'bucket, 'result, 'value
    >(
        bucket: &'bucket mut Bucket,
        result: &'result mut u64,
        value: &'value mut u64
    ) -> &'result mut u64 {
        return_bucket(bucket).cells.as_mut_slice()[
            identity_index(write_index(value))
        ] = 1;
        result
    }

    machine return_after_deep_projected_helper_slice_view_index_target<
        'bucket, 'result
    >(
        bucket: &'bucket mut Bucket,
        result: &'result mut u64
    ) -> &'result mut u64 {
        return_bucket(bucket).cells.as_mut_slice()[
            identity_index(identity_index(make_index()))
        ] = 1;
        result
    }

    machine return_after_recursive_projected_helper_slice_view_index_target<
        'bucket, 'result
    >(
        bucket: &'bucket mut Bucket,
        result: &'result mut u64
    ) -> &'result mut u64 {
        recursive_bucket(bucket).cells.as_mut_slice()[make_index()] = 1;
        result
    }

    machine return_after_slice_view_member_after_index_target<
        'bucket, 'result, 'value
    >(
        bucket: &'bucket mut CellBucket,
        result: &'result mut u64,
        value: &'value mut u64
    ) -> &'result mut u64 {
        return_cell_bucket(bucket).cells.as_mut_slice()[
            identity_index(write_index(value))
        ].value = 1;
        result
    }

    machine return_after_deep_slice_view_member_after_index_target<
        'bucket, 'result
    >(
        bucket: &'bucket mut CellBucket,
        result: &'result mut u64
    ) -> &'result mut u64 {
        return_cell_bucket(bucket).cells.as_mut_slice()[
            identity_index(identity_index(make_index()))
        ].value = 1;
        result
    }

    machine return_after_recursive_slice_view_member_after_index_target<
        'bucket, 'result
    >(
        bucket: &'bucket mut CellBucket,
        result: &'result mut u64
    ) -> &'result mut u64 {
        recursive_cell_bucket(bucket).cells.as_mut_slice()[make_index()].value = 1;
        result
    }

    machine return_after_member_after_index_target<'bucket, 'result, 'value>(
        bucket: &'bucket mut CellBucket,
        result: &'result mut u64,
        value: &'value mut u64
    ) -> &'result mut u64 {
        return_cell_bucket(bucket).cells[
            identity_index(write_index(value))
        ].value = 1;
        result
    }

    machine return_after_deep_member_after_index_target<'bucket, 'result>(
        bucket: &'bucket mut CellBucket,
        result: &'result mut u64
    ) -> &'result mut u64 {
        return_cell_bucket(bucket).cells[
            identity_index(identity_index(make_index()))
        ].value = 1;
        result
    }

    machine return_after_recursive_member_after_index_target<'bucket, 'result>(
        bucket: &'bucket mut CellBucket,
        result: &'result mut u64
    ) -> &'result mut u64 {
        recursive_cell_bucket(bucket).cells[make_index()].value = 1;
        result
    }

    machine return_after_projected_repeated_index_target<
        'bucket, 'result, 'first, 'second
    >(
        bucket: &'bucket mut GridBucket,
        result: &'result mut u64,
        first: &'first mut u64,
        second: &'second mut u64
    ) -> &'result mut u64 {
        return_grid_bucket(bucket).rows[
            write_index(first)
        ][write_index(second)] = 1;
        result
    }

    machine return_after_deep_projected_repeated_index_target<'bucket, 'result>(
        bucket: &'bucket mut GridBucket,
        result: &'result mut u64
    ) -> &'result mut u64 {
        return_grid_bucket(bucket).rows[
            identity_index(identity_index(make_index()))
        ][make_index()] = 1;
        result
    }

    machine return_after_recursive_projected_repeated_index_target<'bucket, 'result>(
        bucket: &'bucket mut GridBucket,
        result: &'result mut u64
    ) -> &'result mut u64 {
        recursive_grid_bucket(bucket).rows[make_index()][make_index()] = 1;
        result
    }

    machine Main::return_after_attached_helper_index_target(
        &mut self
    ) -> &mut u64 {
        self.return_attached_cells()[
            identity_index(write_index(&mut self.value))
        ] = 1;
        &mut self.result
    }

    machine Main::return_after_recursive_attached_index_target(
        &mut self
    ) -> &mut u64 {
        self.recursive_attached_cells()[make_index()] = 1;
        &mut self.result
    }

    machine Main::return_after_attached_slice_view_index_target(
        &mut self
    ) -> &mut u64 {
        self.return_attached_cells().as_mut_slice()[
            identity_index(write_index(&mut self.value))
        ] = 1;
        &mut self.result
    }

    machine Main::return_after_deep_attached_slice_view_index_target(
        &mut self
    ) -> &mut u64 {
        self.return_attached_cells().as_mut_slice()[
            identity_index(identity_index(make_index()))
        ] = 1;
        &mut self.result
    }

    machine Main::return_after_recursive_attached_slice_view_index_target(
        &mut self
    ) -> &mut u64 {
        self.recursive_attached_cells().as_mut_slice()[make_index()] = 1;
        &mut self.result
    }

    machine return_after_deep_index_target(cells: &mut [u64; 2]) -> &mut [u64; 2] {
        cells[identity_index(identity_index(make_index()))] = 1;
        cells
    }

    machine return_after_deep_alias_index_target(
        cells: &mut [u64; 2]
    ) -> &mut [u64; 2] {
        let alias: &mut [u64; 2] = cells;
        alias[identity_index(identity_index(make_index()))] = 1;
        cells
    }

    machine return_after_binding_reborrow_index_target<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        cells[identity_index(write_index(&mut value))] = 1;
        cells
    }

    machine return_after_recursive_index_target(
        cells: &mut [u64; 2]
    ) -> &mut [u64; 2] {
        cells[recursive_index()] = 1;
        cells
    }

    machine return_after_repeated_index_target<'matrix, 'first, 'second>(
        matrix: &'matrix mut [[u64; 2]; 2],
        first: &'first mut u64,
        second: &'second mut u64
    ) -> &'matrix mut [[u64; 2]; 2] {
        matrix[write_index(first)][write_index(second)] = 1;
        matrix
    }

    machine return_after_deep_repeated_index_target(
        matrix: &mut [[u64; 2]; 2]
    ) -> &mut [[u64; 2]; 2] {
        matrix[identity_index(identity_index(make_index()))][make_index()] = 1;
        matrix
    }

    machine Main::index_target_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_index_target(&mut self.cells);
        alias[0] = 2;
    }

    machine Main::nested_index_target_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_nested_index_target(&mut self.cells, &mut self.value);
        alias[0] = 2;
    }

    machine Main::alias_index_target_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_alias_index_target(&mut self.cells, &mut self.value);
        alias[0] = 2;
    }

    machine Main::helper_result_index_target_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_helper_result_index_target(
            &mut self.cells,
            &mut self.value
        );
        alias[0] = 2;
    }

    machine Main::slice_view_index_target_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_slice_view_index_target(
            &mut self.cells,
            &mut self.value
        );
        alias[0] = 2;
    }

    machine Main::deep_slice_view_index_target_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_deep_slice_view_index_target(&mut self.cells);
        alias[0] = 2;
    }

    machine Main::recursive_slice_view_index_target_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_recursive_slice_view_index_target(&mut self.cells);
        alias[0] = 2;
    }

    machine Main::alias_slice_view_index_target_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_alias_slice_view_index_target(
            &mut self.cells,
            &mut self.value
        );
        alias[0] = 2;
    }

    machine Main::deep_alias_slice_view_index_target_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_deep_alias_slice_view_index_target(&mut self.cells);
        alias[0] = 2;
    }

    machine Main::recursive_alias_slice_view_index_target_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_recursive_alias_slice_view_index_target(&mut self.cells);
        alias[0] = 2;
    }

    machine Main::member_alias_slice_view_index_target_result(&mut self) {
        let alias: &mut u64 = return_after_member_alias_slice_view_index_target(
            &mut self.bucket,
            &mut self.result,
            &mut self.value
        );
        alias = 2;
    }

    machine Main::deep_member_alias_slice_view_index_target_result(&mut self) {
        let alias: &mut u64 = return_after_deep_member_alias_slice_view_index_target(
            &mut self.bucket,
            &mut self.result
        );
        alias = 2;
    }

    machine Main::recursive_member_alias_slice_view_index_target_result(&mut self) {
        let alias: &mut u64 = return_after_recursive_member_alias_slice_view_index_target(
            &mut self.bucket,
            &mut self.result
        );
        alias = 2;
    }

    machine Main::slice_view_repeated_index_target_result(&mut self) {
        let alias: &mut [[u64; 2]; 2] = return_after_slice_view_repeated_index_target(
            &mut self.matrix,
            &mut self.value,
            &mut self.other_value
        );
        alias[0][0] = 2;
    }

    machine Main::deep_slice_view_repeated_index_target_result(&mut self) {
        let alias: &mut [[u64; 2]; 2] =
            return_after_deep_slice_view_repeated_index_target(&mut self.matrix);
        alias[0][0] = 2;
    }

    machine Main::recursive_slice_view_repeated_index_target_result(&mut self) {
        let alias: &mut [[u64; 2]; 2] =
            return_after_recursive_slice_view_repeated_index_target(&mut self.matrix);
        alias[0][0] = 2;
    }

    machine Main::helper_slice_view_index_target_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_helper_slice_view_index_target(
            &mut self.cells,
            &mut self.value
        );
        alias[0] = 2;
    }

    machine Main::deep_helper_slice_view_index_target_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_deep_helper_slice_view_index_target(&mut self.cells);
        alias[0] = 2;
    }

    machine Main::recursive_helper_slice_view_index_target_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_recursive_helper_slice_view_index_target(&mut self.cells);
        alias[0] = 2;
    }

    machine Main::recursive_helper_index_target_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_recursive_helper_index_target(&mut self.cells);
        alias[0] = 2;
    }

    machine Main::projected_helper_index_target_result(&mut self) {
        let alias: &mut u64 = return_after_projected_helper_index_target(
            &mut self.bucket,
            &mut self.result,
            &mut self.value
        );
        alias = 2;
    }

    machine Main::deep_projected_helper_index_target_result(&mut self) {
        let alias: &mut u64 = return_after_deep_projected_helper_index_target(
            &mut self.bucket,
            &mut self.result
        );
        alias = 2;
    }

    machine Main::recursive_projected_helper_index_target_result(&mut self) {
        let alias: &mut u64 = return_after_recursive_projected_helper_index_target(
            &mut self.bucket,
            &mut self.result
        );
        alias = 2;
    }

    machine Main::projected_helper_slice_view_index_target_result(&mut self) {
        let alias: &mut u64 = return_after_projected_helper_slice_view_index_target(
            &mut self.bucket,
            &mut self.result,
            &mut self.value
        );
        alias = 2;
    }

    machine Main::deep_projected_helper_slice_view_index_target_result(&mut self) {
        let alias: &mut u64 = return_after_deep_projected_helper_slice_view_index_target(
            &mut self.bucket,
            &mut self.result
        );
        alias = 2;
    }

    machine Main::recursive_projected_helper_slice_view_index_target_result(&mut self) {
        let alias: &mut u64 = return_after_recursive_projected_helper_slice_view_index_target(
            &mut self.bucket,
            &mut self.result
        );
        alias = 2;
    }

    machine Main::slice_view_member_after_index_target_result(&mut self) {
        let alias: &mut u64 = return_after_slice_view_member_after_index_target(
            &mut self.cell_bucket,
            &mut self.result,
            &mut self.value
        );
        alias = 2;
    }

    machine Main::deep_slice_view_member_after_index_target_result(&mut self) {
        let alias: &mut u64 = return_after_deep_slice_view_member_after_index_target(
            &mut self.cell_bucket,
            &mut self.result
        );
        alias = 2;
    }

    machine Main::recursive_slice_view_member_after_index_target_result(&mut self) {
        let alias: &mut u64 = return_after_recursive_slice_view_member_after_index_target(
            &mut self.cell_bucket,
            &mut self.result
        );
        alias = 2;
    }

    machine Main::member_after_index_target_result(&mut self) {
        let alias: &mut u64 = return_after_member_after_index_target(
            &mut self.cell_bucket,
            &mut self.result,
            &mut self.value
        );
        alias = 2;
    }

    machine Main::deep_member_after_index_target_result(&mut self) {
        let alias: &mut u64 = return_after_deep_member_after_index_target(
            &mut self.cell_bucket,
            &mut self.result
        );
        alias = 2;
    }

    machine Main::recursive_member_after_index_target_result(&mut self) {
        let alias: &mut u64 = return_after_recursive_member_after_index_target(
            &mut self.cell_bucket,
            &mut self.result
        );
        alias = 2;
    }

    machine Main::projected_repeated_index_target_result(&mut self) {
        let alias: &mut u64 = return_after_projected_repeated_index_target(
            &mut self.grid_bucket,
            &mut self.result,
            &mut self.value,
            &mut self.other_value
        );
        alias = 2;
    }

    machine Main::deep_projected_repeated_index_target_result(&mut self) {
        let alias: &mut u64 = return_after_deep_projected_repeated_index_target(
            &mut self.grid_bucket,
            &mut self.result
        );
        alias = 2;
    }

    machine Main::recursive_projected_repeated_index_target_result(&mut self) {
        let alias: &mut u64 = return_after_recursive_projected_repeated_index_target(
            &mut self.grid_bucket,
            &mut self.result
        );
        alias = 2;
    }

    machine Main::attached_helper_index_target_result(&mut self) {
        let alias: &mut u64 = self.return_after_attached_helper_index_target();
        alias = 2;
    }

    machine Main::recursive_attached_index_target_result(&mut self) {
        let alias: &mut u64 =
            self.return_after_recursive_attached_index_target();
        alias = 2;
    }

    machine Main::attached_slice_view_index_target_result(&mut self) {
        let alias: &mut u64 =
            self.return_after_attached_slice_view_index_target();
        alias = 2;
    }

    machine Main::deep_attached_slice_view_index_target_result(&mut self) {
        let alias: &mut u64 =
            self.return_after_deep_attached_slice_view_index_target();
        alias = 2;
    }

    machine Main::recursive_attached_slice_view_index_target_result(&mut self) {
        let alias: &mut u64 =
            self.return_after_recursive_attached_slice_view_index_target();
        alias = 2;
    }

    machine Main::deep_index_target_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_deep_index_target(&mut self.cells);
        alias[0] = 2;
    }

    machine Main::deep_alias_index_target_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_deep_alias_index_target(&mut self.cells);
        alias[0] = 2;
    }

    machine Main::binding_reborrow_index_target_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_binding_reborrow_index_target(
            &mut self.cells,
            &mut self.value
        );
        alias[0] = 2;
    }

    machine Main::recursive_index_target_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_recursive_index_target(&mut self.cells);
        alias[0] = 2;
    }

    machine Main::repeated_index_target_result(&mut self) {
        let alias: &mut [[u64; 2]; 2] = return_after_repeated_index_target(
            &mut self.matrix,
            &mut self.value,
            &mut self.other_value
        );
        alias[0][0] = 2;
    }

    machine Main::deep_repeated_index_target_result(&mut self) {
        let alias: &mut [[u64; 2]; 2] =
            return_after_deep_repeated_index_target(&mut self.matrix);
        alias[0][0] = 2;
    }
    "#;

    let typed = typed_program(source);
    let resolver = validation::CallFrameResolver::new(&typed).expect("valid symbol cache");

    for (name, expected_paths) in [
        ("Main::index_target_result", vec!["self.cells"]),
        (
            "Main::nested_index_target_result",
            vec!["self.cells", "self.value"],
        ),
        (
            "Main::alias_index_target_result",
            vec!["self.cells", "self.value"],
        ),
        (
            "Main::helper_result_index_target_result",
            vec!["self.cells", "self.value"],
        ),
        (
            "Main::slice_view_index_target_result",
            vec!["self.cells", "self.value"],
        ),
        (
            "Main::alias_slice_view_index_target_result",
            vec!["self.cells", "self.value"],
        ),
        (
            "Main::member_alias_slice_view_index_target_result",
            vec!["self.bucket.cells", "self.result", "self.value"],
        ),
        (
            "Main::helper_slice_view_index_target_result",
            vec!["self.cells", "self.value"],
        ),
        (
            "Main::projected_helper_index_target_result",
            vec!["self.bucket.cells", "self.result", "self.value"],
        ),
        (
            "Main::projected_helper_slice_view_index_target_result",
            vec!["self.bucket.cells", "self.result", "self.value"],
        ),
        (
            "Main::slice_view_member_after_index_target_result",
            vec!["self.cell_bucket.cells", "self.result", "self.value"],
        ),
        (
            "Main::member_after_index_target_result",
            vec!["self.cell_bucket.cells", "self.result", "self.value"],
        ),
        (
            "Main::projected_repeated_index_target_result",
            vec![
                "self.grid_bucket.rows",
                "self.other_value",
                "self.result",
                "self.value",
            ],
        ),
        (
            "Main::slice_view_repeated_index_target_result",
            vec!["self.matrix", "self.other_value", "self.value"],
        ),
        (
            "Main::attached_helper_index_target_result",
            vec!["self.cells", "self.result", "self.value"],
        ),
        (
            "Main::attached_slice_view_index_target_result",
            vec!["self.cells", "self.result", "self.value"],
        ),
        (
            "Main::repeated_index_target_result",
            vec!["self.matrix", "self.other_value", "self.value"],
        ),
        ("Main::deep_index_target_result", vec!["self.cells"]),
        ("Main::deep_alias_index_target_result", vec!["self.cells"]),
        (
            "Main::deep_slice_view_index_target_result",
            vec!["self.cells"],
        ),
        (
            "Main::deep_alias_slice_view_index_target_result",
            vec!["self.cells"],
        ),
        (
            "Main::deep_member_alias_slice_view_index_target_result",
            vec!["self.bucket.cells", "self.result"],
        ),
        (
            "Main::deep_helper_slice_view_index_target_result",
            vec!["self.cells"],
        ),
        (
            "Main::deep_projected_helper_index_target_result",
            vec!["self.bucket.cells", "self.result"],
        ),
        (
            "Main::deep_projected_helper_slice_view_index_target_result",
            vec!["self.bucket.cells", "self.result"],
        ),
        (
            "Main::deep_slice_view_member_after_index_target_result",
            vec!["self.cell_bucket.cells", "self.result"],
        ),
        (
            "Main::deep_member_after_index_target_result",
            vec!["self.cell_bucket.cells", "self.result"],
        ),
        (
            "Main::deep_projected_repeated_index_target_result",
            vec!["self.grid_bucket.rows", "self.result"],
        ),
        (
            "Main::deep_slice_view_repeated_index_target_result",
            vec!["self.matrix"],
        ),
        (
            "Main::deep_attached_slice_view_index_target_result",
            vec!["self.cells", "self.result"],
        ),
        (
            "Main::deep_repeated_index_target_result",
            vec!["self.matrix"],
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
            "{name} must preserve the returned collection and publish index-call writes"
        );
    }

    for name in [
        "Main::binding_reborrow_index_target_result",
        "Main::recursive_index_target_result",
        "Main::recursive_helper_index_target_result",
        "Main::recursive_slice_view_index_target_result",
        "Main::recursive_alias_slice_view_index_target_result",
        "Main::recursive_member_alias_slice_view_index_target_result",
        "Main::recursive_helper_slice_view_index_target_result",
        "Main::recursive_projected_helper_index_target_result",
        "Main::recursive_projected_helper_slice_view_index_target_result",
        "Main::recursive_slice_view_member_after_index_target_result",
        "Main::recursive_member_after_index_target_result",
        "Main::recursive_projected_repeated_index_target_result",
        "Main::recursive_slice_view_repeated_index_target_result",
        "Main::recursive_attached_index_target_result",
        "Main::recursive_attached_slice_view_index_target_result",
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
            "{name} must remain opaque without a complete non-rebinding target index"
        );
    }
}

#[test]
fn transparent_returned_place_accepts_finite_value_call_assignments() {
    let source = r#"
    data Pair {
        first: u64;
        second: u64;
    }

    data GenericPair<T> {
        first: T;
        second: u64;
    }

    data PairChoice {
        tag: u64;
        case Values(first: u64, second: u64);
        case Wrapped(pair: Pair);
        case Empty;
    }

    data GenericChoice<T> {
        case Value(value: T);
        case Empty;
    }

    data NestedPair {
        pair: Pair;
        marker: u64;
    }

    data DeepPair {
        nested: NestedPair;
    }

    data DeeperPair {
        deep: DeepPair;
    }

    data ChoiceHolder {
        choice: PairChoice;
    }

    data OuterChoice {
        stamp: u64;
        case Nested(choice: PairChoice);
        case Empty;
    }

    data GenericChoiceHolder {
        choice: GenericChoice<u64>;
    }

    data Main {
        value: u64;
        other: u64;
        pair: Pair;
        source_pair: Pair;
        generic_pair: GenericPair<u64>;
        choice: PairChoice;
        generic_choice: GenericChoice<u64>;
        nested_pair: NestedPair;
        deep_pair: DeepPair;
        deeper_pair: DeeperPair;
        choice_holder: ChoiceHolder;
        outer_choice: OuterChoice;
        generic_choice_holder: GenericChoiceHolder;
        cells: [u64; 2];
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

    machine combine(first: u64, second: u64) -> u64 {
        first + second
    }

    machine make_pair(value: &mut u64) -> Pair {
        value = 1;
        Pair { first: 1, second: 2 }
    }

    machine make_cells(value: &mut u64) -> [u64; 2] {
        value = 1;
        [1, 2]
    }

    machine return_pair(pair: &mut Pair) -> &mut Pair {
        pair
    }

    machine recursive_value() -> u64 {
        recursive_value()
    }

    machine return_after_value_call<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        cells[0] = compute(value);
        cells
    }

    machine return_after_nested_value_call<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        cells[0] = identity(compute(value));
        cells
    }

    machine return_after_sibling_value_calls<'cells, 'first, 'second>(
        cells: &'cells mut [u64; 2],
        first: &'first mut u64,
        second: &'second mut u64
    ) -> &'cells mut [u64; 2] {
        cells[0] = combine(compute(first), compute(second));
        cells
    }

    machine return_after_deep_sibling_value_call<'cells, 'first, 'second>(
        cells: &'cells mut [u64; 2],
        first: &'first mut u64,
        second: &'second mut u64
    ) -> &'cells mut [u64; 2] {
        cells[0] = combine(identity(compute(first)), compute(second));
        cells
    }

    machine return_after_reborrow_sibling_value_call<'cells, 'first, 'second>(
        cells: &'cells mut [u64; 2],
        first: &'first mut u64,
        second: &'second mut u64
    ) -> &'cells mut [u64; 2] {
        cells[0] = combine(compute(first), compute(&mut second));
        cells
    }

    machine return_after_deep_value_call<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        cells[0] = identity(identity(compute(value)));
        cells
    }

    machine return_after_four_level_value_call<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        cells[0] = identity(identity(identity(compute(value))));
        cells
    }

    machine return_after_record_value_calls<'cells, 'pair, 'first, 'second>(
        cells: &'cells mut [u64; 2],
        pair: &'pair mut Pair,
        first: &'first mut u64,
        second: &'second mut u64
    ) -> &'cells mut [u64; 2] {
        pair = Pair {
            first: identity(compute(first)),
            second: identity(identity(compute(second)))
        };
        cells
    }

    machine return_after_generic_record_value_call<'cells, 'pair, 'value>(
        cells: &'cells mut [u64; 2],
        pair: &'pair mut GenericPair<u64>,
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        pair = GenericPair {
            first: 0,
            second: compute(value)
        };
        cells
    }

    machine return_after_computed_record_field<'cells, 'pair, 'value>(
        cells: &'cells mut [u64; 2],
        pair: &'pair mut Pair,
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        pair = Pair {
            first: compute(value) + 1,
            second: 0
        };
        cells
    }

    machine return_after_deep_computed_record_field<'cells, 'pair, 'value>(
        cells: &'cells mut [u64; 2],
        pair: &'pair mut Pair,
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        pair = Pair {
            first: identity(identity(identity(identity(compute(value))))) + 1,
            second: 0
        };
        cells
    }

    machine return_after_cast_record_field<'cells, 'pair, 'value>(
        cells: &'cells mut [u64; 2],
        pair: &'pair mut Pair,
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        pair = Pair {
            first: compute(value) as u64,
            second: 0
        };
        cells
    }

    machine return_after_unary_record_field<'cells, 'pair, 'value>(
        cells: &'cells mut [u64; 2],
        pair: &'pair mut Pair,
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        pair = Pair {
            first: ~compute(value),
            second: 0
        };
        cells
    }

    machine return_after_nested_computed_record_field<'cells, 'pair, 'value>(
        cells: &'cells mut [u64; 2],
        pair: &'pair mut Pair,
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        pair = Pair {
            first: (compute(value) as u64) + 1,
            second: 0
        };
        cells
    }

    machine return_after_three_computed_record_field<'cells, 'pair, 'value>(
        cells: &'cells mut [u64; 2],
        pair: &'pair mut Pair,
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        pair = Pair {
            first: (~compute(value) as u64) + 1,
            second: 0
        };
        cells
    }

    machine return_after_projected_record_field<'cells, 'pair, 'value>(
        cells: &'cells mut [u64; 2],
        pair: &'pair mut Pair,
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        pair = Pair {
            first: make_pair(value).first,
            second: 0
        };
        cells
    }

    machine return_after_indexed_record_field<'cells, 'pair, 'value>(
        cells: &'cells mut [u64; 2],
        pair: &'pair mut Pair,
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        pair = Pair {
            first: make_cells(value)[0],
            second: 0
        };
        cells
    }

    machine return_after_reference_projected_record_field<'cells, 'pair, 'source>(
        cells: &'cells mut [u64; 2],
        pair: &'pair mut Pair,
        source: &'source mut Pair
    ) -> &'cells mut [u64; 2] {
        pair = Pair {
            first: return_pair(source).first,
            second: 0
        };
        cells
    }

    machine return_after_case_value_calls<'cells, 'choice, 'first, 'second>(
        cells: &'cells mut [u64; 2],
        choice: &'choice mut PairChoice,
        first: &'first mut u64,
        second: &'second mut u64
    ) -> &'cells mut [u64; 2] {
        choice = PairChoice::Values {
            tag: identity(compute(first)),
            first: 0,
            second: identity(identity(compute(second)))
        };
        cells
    }

    machine return_after_generic_case_value_call<'cells, 'choice, 'value>(
        cells: &'cells mut [u64; 2],
        choice: &'choice mut GenericChoice<u64>,
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        choice = GenericChoice::Value { value: compute(value) };
        cells
    }

    machine return_after_computed_case_field<'cells, 'choice, 'value>(
        cells: &'cells mut [u64; 2],
        choice: &'choice mut PairChoice,
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        choice = PairChoice::Values {
            tag: 0,
            first: compute(value) + 1,
            second: 0
        };
        cells
    }

    machine return_after_nested_record_value_calls<'cells, 'nested, 'first, 'second>(
        cells: &'cells mut [u64; 2],
        nested: &'nested mut NestedPair,
        first: &'first mut u64,
        second: &'second mut u64
    ) -> &'cells mut [u64; 2] {
        nested = NestedPair {
            pair: Pair {
                first: identity(compute(first)),
                second: identity(identity(identity(compute(second))))
            },
            marker: 0
        };
        cells
    }

    machine return_after_case_nested_record_value_calls<'cells, 'choice, 'first, 'second>(
        cells: &'cells mut [u64; 2],
        choice: &'choice mut PairChoice,
        first: &'first mut u64,
        second: &'second mut u64
    ) -> &'cells mut [u64; 2] {
        choice = PairChoice::Wrapped {
            tag: 0,
            pair: Pair {
                first: identity(compute(first)),
                second: identity(identity(identity(compute(second))))
            }
        };
        cells
    }

    machine return_after_deep_record_value_call<'cells, 'deep, 'value>(
        cells: &'cells mut [u64; 2],
        deep: &'deep mut DeepPair,
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        deep = DeepPair {
            nested: NestedPair {
                pair: Pair {
                    first: compute(value),
                    second: 0
                },
                marker: 0
            }
        };
        cells
    }

    machine return_after_deeper_record_value_call<'cells, 'deeper, 'value>(
        cells: &'cells mut [u64; 2],
        deeper: &'deeper mut DeeperPair,
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        deeper = DeeperPair {
            deep: DeepPair {
                nested: NestedPair {
                    pair: Pair {
                        first: compute(value),
                        second: 0
                    },
                    marker: 0
                }
            }
        };
        cells
    }

    machine return_after_nested_case_value_call<'cells, 'holder, 'value>(
        cells: &'cells mut [u64; 2],
        holder: &'holder mut ChoiceHolder,
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        holder = ChoiceHolder {
            choice: PairChoice::Values {
                tag: 0,
                first: compute(value),
                second: 0
            }
        };
        cells
    }

    machine return_after_case_nested_case_value_calls<'cells, 'outer, 'first, 'second>(
        cells: &'cells mut [u64; 2],
        outer: &'outer mut OuterChoice,
        first: &'first mut u64,
        second: &'second mut u64
    ) -> &'cells mut [u64; 2] {
        outer = OuterChoice::Nested {
            stamp: 0,
            choice: PairChoice::Values {
                tag: identity(compute(first)),
                first: 0,
                second: identity(identity(identity(compute(second))))
            }
        };
        cells
    }

    machine return_after_generic_nested_case_value_call<'cells, 'holder, 'value>(
        cells: &'cells mut [u64; 2],
        holder: &'holder mut GenericChoiceHolder,
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        holder = GenericChoiceHolder {
            choice: GenericChoice::Value { value: compute(value) }
        };
        cells
    }

    machine return_after_nested_computed_case_field<'cells, 'holder, 'value>(
        cells: &'cells mut [u64; 2],
        holder: &'holder mut ChoiceHolder,
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        holder = ChoiceHolder {
            choice: PairChoice::Values {
                tag: 0,
                first: compute(value) + 1,
                second: 0
            }
        };
        cells
    }

    machine return_after_too_deep_value_call<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        cells[0] = identity(identity(identity(identity(compute(value)))));
        cells
    }

    machine return_after_binding_reborrow_value_call<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        cells[0] = identity(compute(&mut value));
        cells
    }

    machine return_after_recursive_value_call(
        cells: &mut [u64; 2]
    ) -> &mut [u64; 2] {
        cells[0] = recursive_value();
        cells
    }

    machine Main::value_call_assignment_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_value_call(&mut self.cells, &mut self.value);
        alias[0] = 2;
    }

    machine Main::nested_value_call_assignment_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_nested_value_call(&mut self.cells, &mut self.value);
        alias[0] = 2;
    }

    machine Main::sibling_value_call_assignment_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_sibling_value_calls(
            &mut self.cells,
            &mut self.value,
            &mut self.other
        );
        alias[0] = 2;
    }

    machine Main::deep_sibling_value_call_assignment_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_deep_sibling_value_call(
            &mut self.cells,
            &mut self.value,
            &mut self.other
        );
        alias[0] = 2;
    }

    machine Main::reborrow_sibling_value_call_assignment_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_reborrow_sibling_value_call(
            &mut self.cells,
            &mut self.value,
            &mut self.other
        );
        alias[0] = 2;
    }

    machine Main::deep_value_call_assignment_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_deep_value_call(&mut self.cells, &mut self.value);
        alias[0] = 2;
    }

    machine Main::four_level_value_call_assignment_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_four_level_value_call(&mut self.cells, &mut self.value);
        alias[0] = 2;
    }

    machine Main::record_value_call_assignment_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_record_value_calls(
            &mut self.cells,
            &mut self.pair,
            &mut self.value,
            &mut self.other
        );
        alias[0] = 2;
    }

    machine Main::generic_record_value_call_assignment_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_generic_record_value_call(
            &mut self.cells,
            &mut self.generic_pair,
            &mut self.value
        );
        alias[0] = 2;
    }

    machine Main::computed_record_field_assignment_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_computed_record_field(
            &mut self.cells,
            &mut self.pair,
            &mut self.value
        );
        alias[0] = 2;
    }

    machine Main::deep_computed_record_field_assignment_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_deep_computed_record_field(
            &mut self.cells,
            &mut self.pair,
            &mut self.value
        );
        alias[0] = 2;
    }

    machine Main::cast_record_field_assignment_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_cast_record_field(
            &mut self.cells,
            &mut self.pair,
            &mut self.value
        );
        alias[0] = 2;
    }

    machine Main::unary_record_field_assignment_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_unary_record_field(
            &mut self.cells,
            &mut self.pair,
            &mut self.value
        );
        alias[0] = 2;
    }

    machine Main::nested_computed_record_field_assignment_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_nested_computed_record_field(
            &mut self.cells,
            &mut self.pair,
            &mut self.value
        );
        alias[0] = 2;
    }

    machine Main::three_computed_record_field_assignment_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_three_computed_record_field(
            &mut self.cells,
            &mut self.pair,
            &mut self.value
        );
        alias[0] = 2;
    }

    machine Main::projected_record_field_assignment_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_projected_record_field(
            &mut self.cells,
            &mut self.pair,
            &mut self.value
        );
        alias[0] = 2;
    }

    machine Main::indexed_record_field_assignment_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_indexed_record_field(
            &mut self.cells,
            &mut self.pair,
            &mut self.value
        );
        alias[0] = 2;
    }

    machine Main::reference_projected_record_field_assignment_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_reference_projected_record_field(
            &mut self.cells,
            &mut self.pair,
            &mut self.source_pair
        );
        alias[0] = 2;
    }

    machine Main::case_value_call_assignment_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_case_value_calls(
            &mut self.cells,
            &mut self.choice,
            &mut self.value,
            &mut self.other
        );
        alias[0] = 2;
    }

    machine Main::generic_case_value_call_assignment_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_generic_case_value_call(
            &mut self.cells,
            &mut self.generic_choice,
            &mut self.value
        );
        alias[0] = 2;
    }

    machine Main::computed_case_field_assignment_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_computed_case_field(
            &mut self.cells,
            &mut self.choice,
            &mut self.value
        );
        alias[0] = 2;
    }

    machine Main::nested_record_value_call_assignment_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_nested_record_value_calls(
            &mut self.cells,
            &mut self.nested_pair,
            &mut self.value,
            &mut self.other
        );
        alias[0] = 2;
    }

    machine Main::case_nested_record_value_call_assignment_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_case_nested_record_value_calls(
            &mut self.cells,
            &mut self.choice,
            &mut self.value,
            &mut self.other
        );
        alias[0] = 2;
    }

    machine Main::deep_record_value_call_assignment_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_deep_record_value_call(
            &mut self.cells,
            &mut self.deep_pair,
            &mut self.value
        );
        alias[0] = 2;
    }

    machine Main::deeper_record_value_call_assignment_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_deeper_record_value_call(
            &mut self.cells,
            &mut self.deeper_pair,
            &mut self.value
        );
        alias[0] = 2;
    }

    machine Main::nested_case_value_call_assignment_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_nested_case_value_call(
            &mut self.cells,
            &mut self.choice_holder,
            &mut self.value
        );
        alias[0] = 2;
    }

    machine Main::case_nested_case_value_call_assignment_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_case_nested_case_value_calls(
            &mut self.cells,
            &mut self.outer_choice,
            &mut self.value,
            &mut self.other
        );
        alias[0] = 2;
    }

    machine Main::generic_nested_case_value_call_assignment_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_generic_nested_case_value_call(
            &mut self.cells,
            &mut self.generic_choice_holder,
            &mut self.value
        );
        alias[0] = 2;
    }

    machine Main::nested_computed_case_field_assignment_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_nested_computed_case_field(
            &mut self.cells,
            &mut self.choice_holder,
            &mut self.value
        );
        alias[0] = 2;
    }

    machine Main::too_deep_value_call_assignment_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_too_deep_value_call(&mut self.cells, &mut self.value);
        alias[0] = 2;
    }

    machine Main::binding_reborrow_value_call_assignment_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_binding_reborrow_value_call(
            &mut self.cells,
            &mut self.value
        );
        alias[0] = 2;
    }

    machine Main::recursive_value_call_assignment_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_recursive_value_call(&mut self.cells);
        alias[0] = 2;
    }

    "#;

    let typed = typed_program(source);
    let resolver = validation::CallFrameResolver::new(&typed).expect("valid symbol cache");

    for (name, expected_paths) in [
        (
            "Main::value_call_assignment_result",
            vec!["self.cells", "self.value"],
        ),
        (
            "Main::nested_value_call_assignment_result",
            vec!["self.cells", "self.value"],
        ),
        (
            "Main::sibling_value_call_assignment_result",
            vec!["self.cells", "self.other", "self.value"],
        ),
        (
            "Main::deep_value_call_assignment_result",
            vec!["self.cells", "self.value"],
        ),
        (
            "Main::too_deep_value_call_assignment_result",
            vec!["self.cells", "self.value"],
        ),
        (
            "Main::deep_sibling_value_call_assignment_result",
            vec!["self.cells", "self.other", "self.value"],
        ),
        (
            "Main::four_level_value_call_assignment_result",
            vec!["self.cells", "self.value"],
        ),
        (
            "Main::record_value_call_assignment_result",
            vec!["self.cells", "self.other", "self.pair", "self.value"],
        ),
        (
            "Main::computed_record_field_assignment_result",
            vec!["self.cells", "self.pair", "self.value"],
        ),
        (
            "Main::deep_computed_record_field_assignment_result",
            vec!["self.cells", "self.pair", "self.value"],
        ),
        (
            "Main::three_computed_record_field_assignment_result",
            vec!["self.cells", "self.pair", "self.value"],
        ),
        (
            "Main::deeper_record_value_call_assignment_result",
            vec!["self.cells", "self.deeper_pair", "self.value"],
        ),
        (
            "Main::cast_record_field_assignment_result",
            vec!["self.cells", "self.pair", "self.value"],
        ),
        (
            "Main::unary_record_field_assignment_result",
            vec!["self.cells", "self.pair", "self.value"],
        ),
        (
            "Main::nested_computed_record_field_assignment_result",
            vec!["self.cells", "self.pair", "self.value"],
        ),
        (
            "Main::projected_record_field_assignment_result",
            vec!["self.cells", "self.pair", "self.value"],
        ),
        (
            "Main::indexed_record_field_assignment_result",
            vec!["self.cells", "self.pair", "self.value"],
        ),
        (
            "Main::case_value_call_assignment_result",
            vec!["self.cells", "self.choice", "self.other", "self.value"],
        ),
        (
            "Main::computed_case_field_assignment_result",
            vec!["self.cells", "self.choice", "self.value"],
        ),
        (
            "Main::nested_record_value_call_assignment_result",
            vec!["self.cells", "self.nested_pair", "self.other", "self.value"],
        ),
        (
            "Main::case_nested_record_value_call_assignment_result",
            vec!["self.cells", "self.choice", "self.other", "self.value"],
        ),
        (
            "Main::deep_record_value_call_assignment_result",
            vec!["self.cells", "self.deep_pair", "self.value"],
        ),
        (
            "Main::nested_case_value_call_assignment_result",
            vec!["self.cells", "self.choice_holder", "self.value"],
        ),
        (
            "Main::nested_computed_case_field_assignment_result",
            vec!["self.cells", "self.choice_holder", "self.value"],
        ),
        (
            "Main::case_nested_case_value_call_assignment_result",
            vec![
                "self.cells",
                "self.other",
                "self.outer_choice",
                "self.value",
            ],
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
            "{name} must distinguish value-call writes from reference rebinding"
        );
    }

    for name in [
        "Main::reborrow_sibling_value_call_assignment_result",
        "Main::binding_reborrow_value_call_assignment_result",
        "Main::recursive_value_call_assignment_result",
        "Main::generic_record_value_call_assignment_result",
        "Main::reference_projected_record_field_assignment_result",
        "Main::generic_case_value_call_assignment_result",
        "Main::generic_nested_case_value_call_assignment_result",
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
            "{name} must remain opaque for unsupported assignment value shapes"
        );
    }
}
