use super::*;

#[test]
fn transparent_returned_place_accepts_bounded_indexed_target_calls() {
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

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let resolver = psi_validation::CallFrameResolver::new(&typed).expect("valid symbol cache");

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
        "Main::deep_index_target_result",
        "Main::deep_alias_index_target_result",
        "Main::binding_reborrow_index_target_result",
        "Main::recursive_index_target_result",
        "Main::recursive_helper_index_target_result",
        "Main::deep_slice_view_index_target_result",
        "Main::recursive_slice_view_index_target_result",
        "Main::deep_alias_slice_view_index_target_result",
        "Main::recursive_alias_slice_view_index_target_result",
        "Main::deep_member_alias_slice_view_index_target_result",
        "Main::recursive_member_alias_slice_view_index_target_result",
        "Main::deep_helper_slice_view_index_target_result",
        "Main::recursive_helper_slice_view_index_target_result",
        "Main::deep_projected_helper_index_target_result",
        "Main::recursive_projected_helper_index_target_result",
        "Main::deep_projected_helper_slice_view_index_target_result",
        "Main::recursive_projected_helper_slice_view_index_target_result",
        "Main::deep_slice_view_member_after_index_target_result",
        "Main::recursive_slice_view_member_after_index_target_result",
        "Main::deep_member_after_index_target_result",
        "Main::recursive_member_after_index_target_result",
        "Main::deep_projected_repeated_index_target_result",
        "Main::recursive_projected_repeated_index_target_result",
        "Main::deep_slice_view_repeated_index_target_result",
        "Main::recursive_slice_view_repeated_index_target_result",
        "Main::recursive_attached_index_target_result",
        "Main::deep_attached_slice_view_index_target_result",
        "Main::recursive_attached_slice_view_index_target_result",
        "Main::deep_repeated_index_target_result",
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
            "{name} must remain opaque outside the bounded indexed-target rung"
        );
    }
}

#[test]
fn transparent_returned_place_accepts_bounded_value_call_assignments() {
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

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let resolver = psi_validation::CallFrameResolver::new(&typed).expect("valid symbol cache");

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
        "Main::too_deep_value_call_assignment_result",
        "Main::reborrow_sibling_value_call_assignment_result",
        "Main::binding_reborrow_value_call_assignment_result",
        "Main::recursive_value_call_assignment_result",
        "Main::generic_record_value_call_assignment_result",
        "Main::deep_computed_record_field_assignment_result",
        "Main::three_computed_record_field_assignment_result",
        "Main::reference_projected_record_field_assignment_result",
        "Main::generic_case_value_call_assignment_result",
        "Main::deeper_record_value_call_assignment_result",
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
            "{name} must remain opaque outside the bounded value-call assignment rung"
        );
    }
}

#[test]
fn transparent_returned_place_accepts_bounded_direct_scalar_computations() {
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

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let resolver = psi_validation::CallFrameResolver::new(&typed).expect("valid symbol cache");

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
            "{name} must admit the bounded computed value through its primitive mutable-reference target"
        );
    }

    for name in [
        "Main::nineteen_computed_scalar_result",
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
            "{name} must remain opaque outside the bounded primitive scalar rung"
        );
    }
}

#[test]
fn transparent_returned_place_accepts_bounded_fixed_array_assignment_values() {
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

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let resolver = psi_validation::CallFrameResolver::new(&typed).expect("valid symbol cache");

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
            "{name} must publish each bounded fixed-array element call write"
        );
    }

    for name in [
        "Main::four_array_levels_result",
        "Main::three_array_computations_result",
        "Main::five_array_calls_result",
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
            "{name} must remain opaque outside the bounded fixed-array rung"
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

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let resolver = psi_validation::CallFrameResolver::new(&typed).expect("valid symbol cache");

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
            "{name} must remain opaque outside the bounded mixed-aggregate rung"
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

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let resolver = psi_validation::CallFrameResolver::new(&typed).expect("valid symbol cache");

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
        "the member and field computations must share the depth-two budget and publish every field write"
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
        "the literal member must share the aggregate-depth-two and reduced computation budgets while publishing every nested write"
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
        "the literal member must carry its reduced computation budget through the nested fixed array and publish every element and sibling write"
    );

    for name in [
        "Main::wrapped_computed_literal_field_result",
        "Main::third_shell_literal_member_result",
        "Main::third_aggregate_literal_member_result",
        "Main::array_field_two_shells_result",
        "Main::record_array_record_literal_member_result",
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

#[test]
fn transparent_returned_place_composes_bounded_assignment_call_trees() {
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

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let resolver = psi_validation::CallFrameResolver::new(&typed).expect("valid symbol cache");

    for name in [
        "Main::composed_assignment_result",
        "Main::slice_view_composed_assignment_result",
        "Main::deep_value_assignment_result",
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
        "Main::deep_target_assignment_result",
        "Main::reborrow_target_assignment_result",
        "Main::reborrow_value_assignment_result",
        "Main::deep_slice_view_target_assignment_result",
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
