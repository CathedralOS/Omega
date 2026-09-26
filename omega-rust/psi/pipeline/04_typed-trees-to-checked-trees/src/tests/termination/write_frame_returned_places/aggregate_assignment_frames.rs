use crate::tests::front_end::typed_program;

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
