use super::*;

#[test]
fn transparent_returned_place_accepts_bounded_indexed_statement_arguments() {
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
        result: u64;
        index_write: u64;
        second_index_write: u64;
        cells: [u64; 2];
        bucket: Bucket;
        cell_bucket: CellBucket;
        grid_bucket: GridBucket;
    }

    machine write_argument(value: &mut u64) {
        value = 1;
    }

    machine make_index() -> u64 [0..=1] {
        0
    }

    machine write_index(value: &mut u64) -> u64 [0..=1] {
        value = 2;
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

    machine recursive_cell_bucket(
        bucket: &mut CellBucket
    ) -> &mut CellBucket {
        recursive_cell_bucket(bucket)
    }

    machine return_grid_bucket(bucket: &mut GridBucket) -> &mut GridBucket {
        bucket
    }

    machine Main::return_attached_cells(&mut self) -> &mut [u64; 2] {
        &mut self.cells
    }

    machine Main::recursive_attached_cells(&mut self) -> &mut [u64; 2] {
        self.recursive_attached_cells()
    }

    machine Main::return_attached_bucket(&mut self) -> &mut Bucket {
        &mut self.bucket
    }

    machine Main::recursive_attached_bucket(&mut self) -> &mut Bucket {
        self.recursive_attached_bucket()
    }

    machine return_after_indexed_statement<'cells, 'result>(
        cells: &'cells mut [u64; 2],
        result: &'result mut u64
    ) -> &'result mut u64 {
        write_argument(&mut cells[make_index()]);
        result
    }

    machine return_after_nested_indexed_statement<'cells, 'result, 'write>(
        cells: &'cells mut [u64; 2],
        result: &'result mut u64,
        index_write: &'write mut u64
    ) -> &'result mut u64 {
        write_argument(&mut cells[identity_index(write_index(index_write))]);
        result
    }

    machine return_after_slice_view_indexed_statement<'cells, 'result, 'write>(
        cells: &'cells mut [u64; 2],
        result: &'result mut u64,
        index_write: &'write mut u64
    ) -> &'result mut u64 {
        write_argument(
            &mut cells.as_mut_slice()[
                identity_index(write_index(index_write))
            ]
        );
        result
    }

    machine return_after_deep_slice_view_indexed_statement<'cells, 'result>(
        cells: &'cells mut [u64; 2],
        result: &'result mut u64
    ) -> &'result mut u64 {
        write_argument(
            &mut cells.as_mut_slice()[
                identity_index(identity_index(make_index()))
            ]
        );
        result
    }

    machine return_after_recursive_slice_view_indexed_statement<'cells, 'result>(
        cells: &'cells mut [u64; 2],
        result: &'result mut u64
    ) -> &'result mut u64 {
        write_argument(&mut cells.as_mut_slice()[recursive_index()]);
        result
    }

    machine return_after_alias_slice_view_indexed_statement<
        'cells, 'result, 'write
    >(
        cells: &'cells mut [u64; 2],
        result: &'result mut u64,
        index_write: &'write mut u64
    ) -> &'result mut u64 {
        let alias: &mut [u64; 2] = cells;
        write_argument(
            &mut alias.as_mut_slice()[
                identity_index(write_index(index_write))
            ]
        );
        result
    }

    machine return_after_deep_alias_slice_view_indexed_statement<
        'cells, 'result
    >(
        cells: &'cells mut [u64; 2],
        result: &'result mut u64
    ) -> &'result mut u64 {
        let alias: &mut [u64; 2] = cells;
        write_argument(
            &mut alias.as_mut_slice()[
                identity_index(identity_index(make_index()))
            ]
        );
        result
    }

    machine return_after_recursive_alias_slice_view_indexed_statement<
        'cells, 'result
    >(
        cells: &'cells mut [u64; 2],
        result: &'result mut u64
    ) -> &'result mut u64 {
        let alias: &mut [u64; 2] = cells;
        write_argument(&mut alias.as_mut_slice()[recursive_index()]);
        result
    }

    machine return_after_member_alias_slice_view_indexed_statement<
        'bucket, 'result, 'write
    >(
        bucket: &'bucket mut Bucket,
        result: &'result mut u64,
        index_write: &'write mut u64
    ) -> &'result mut u64 {
        let alias: &mut [u64; 2] = &mut bucket.cells;
        write_argument(
            &mut alias.as_mut_slice()[
                identity_index(write_index(index_write))
            ]
        );
        result
    }

    machine return_after_deep_member_alias_slice_view_indexed_statement<
        'bucket, 'result
    >(
        bucket: &'bucket mut Bucket,
        result: &'result mut u64
    ) -> &'result mut u64 {
        let alias: &mut [u64; 2] = &mut bucket.cells;
        write_argument(
            &mut alias.as_mut_slice()[
                identity_index(identity_index(make_index()))
            ]
        );
        result
    }

    machine return_after_recursive_member_alias_slice_view_indexed_statement<
        'bucket, 'result
    >(
        bucket: &'bucket mut Bucket,
        result: &'result mut u64
    ) -> &'result mut u64 {
        let alias: &mut [u64; 2] = &mut bucket.cells;
        write_argument(&mut alias.as_mut_slice()[recursive_index()]);
        result
    }

    machine return_after_helper_slice_view_indexed_statement<
        'cells, 'result, 'write
    >(
        cells: &'cells mut [u64; 2],
        result: &'result mut u64,
        index_write: &'write mut u64
    ) -> &'result mut u64 {
        write_argument(
            &mut return_cells(cells).as_mut_slice()[
                identity_index(write_index(index_write))
            ]
        );
        result
    }

    machine return_after_deep_helper_slice_view_indexed_statement<
        'cells, 'result
    >(
        cells: &'cells mut [u64; 2],
        result: &'result mut u64
    ) -> &'result mut u64 {
        write_argument(
            &mut return_cells(cells).as_mut_slice()[
                identity_index(identity_index(make_index()))
            ]
        );
        result
    }

    machine return_after_recursive_helper_slice_view_indexed_statement<
        'cells, 'result
    >(
        cells: &'cells mut [u64; 2],
        result: &'result mut u64
    ) -> &'result mut u64 {
        write_argument(
            &mut recursive_cells(cells).as_mut_slice()[make_index()]
        );
        result
    }

    machine return_after_alias_indexed_statement<'cells, 'result, 'write>(
        cells: &'cells mut [u64; 2],
        result: &'result mut u64,
        index_write: &'write mut u64
    ) -> &'result mut u64 {
        let alias: &mut [u64; 2] = cells;
        write_argument(
            &mut alias[identity_index(write_index(index_write))]
        );
        result
    }

    machine return_after_helper_result_indexed_statement<'cells, 'result, 'write>(
        cells: &'cells mut [u64; 2],
        result: &'result mut u64,
        index_write: &'write mut u64
    ) -> &'result mut u64 {
        write_argument(
            &mut return_cells(cells)[identity_index(write_index(index_write))]
        );
        result
    }

    machine return_after_recursive_helper_indexed_statement<'cells, 'result>(
        cells: &'cells mut [u64; 2],
        result: &'result mut u64
    ) -> &'result mut u64 {
        write_argument(&mut recursive_cells(cells)[make_index()]);
        result
    }

    machine return_after_projected_helper_indexed_statement<'bucket, 'result, 'write>(
        bucket: &'bucket mut Bucket,
        result: &'result mut u64,
        index_write: &'write mut u64
    ) -> &'result mut u64 {
        write_argument(
            &mut return_bucket(bucket).cells[
                identity_index(write_index(index_write))
            ]
        );
        result
    }

    machine return_after_projected_helper_slice_view_indexed_statement<
        'bucket, 'result, 'write
    >(
        bucket: &'bucket mut Bucket,
        result: &'result mut u64,
        index_write: &'write mut u64
    ) -> &'result mut u64 {
        write_argument(
            &mut return_bucket(bucket).cells.as_mut_slice()[
                identity_index(write_index(index_write))
            ]
        );
        result
    }

    machine return_after_deep_projected_helper_slice_view_indexed_statement<
        'bucket, 'result
    >(
        bucket: &'bucket mut Bucket,
        result: &'result mut u64
    ) -> &'result mut u64 {
        write_argument(
            &mut return_bucket(bucket).cells.as_mut_slice()[
                identity_index(identity_index(make_index()))
            ]
        );
        result
    }

    machine return_after_recursive_projected_helper_slice_view_indexed_statement<
        'bucket, 'result
    >(
        bucket: &'bucket mut Bucket,
        result: &'result mut u64
    ) -> &'result mut u64 {
        write_argument(
            &mut recursive_bucket(bucket).cells.as_mut_slice()[make_index()]
        );
        result
    }

    machine return_after_slice_view_member_after_index_statement<
        'bucket, 'result, 'write
    >(
        bucket: &'bucket mut CellBucket,
        result: &'result mut u64,
        index_write: &'write mut u64
    ) -> &'result mut u64 {
        write_argument(
            &mut return_cell_bucket(bucket).cells.as_mut_slice()[
                identity_index(write_index(index_write))
            ].value
        );
        result
    }

    machine return_after_deep_slice_view_member_after_index_statement<
        'bucket, 'result
    >(
        bucket: &'bucket mut CellBucket,
        result: &'result mut u64
    ) -> &'result mut u64 {
        write_argument(
            &mut return_cell_bucket(bucket).cells.as_mut_slice()[
                identity_index(identity_index(make_index()))
            ].value
        );
        result
    }

    machine return_after_recursive_slice_view_member_after_index_statement<
        'bucket, 'result
    >(
        bucket: &'bucket mut CellBucket,
        result: &'result mut u64
    ) -> &'result mut u64 {
        write_argument(
            &mut recursive_cell_bucket(bucket).cells.as_mut_slice()[make_index()].value
        );
        result
    }

    machine return_after_recursive_projected_helper_statement<'bucket, 'result>(
        bucket: &'bucket mut Bucket,
        result: &'result mut u64
    ) -> &'result mut u64 {
        write_argument(&mut recursive_bucket(bucket).cells[make_index()]);
        result
    }

    machine return_after_member_after_index_statement<'bucket, 'result, 'write>(
        bucket: &'bucket mut CellBucket,
        result: &'result mut u64,
        index_write: &'write mut u64
    ) -> &'result mut u64 {
        write_argument(
            &mut return_cell_bucket(bucket).cells[
                identity_index(write_index(index_write))
            ].value
        );
        result
    }

    machine return_after_recursive_member_after_index_statement<'bucket, 'result>(
        bucket: &'bucket mut CellBucket,
        result: &'result mut u64
    ) -> &'result mut u64 {
        write_argument(
            &mut recursive_cell_bucket(bucket).cells[make_index()].value
        );
        result
    }

    machine return_after_repeated_index_statement<'bucket, 'result, 'first, 'second>(
        bucket: &'bucket mut GridBucket,
        result: &'result mut u64,
        first: &'first mut u64,
        second: &'second mut u64
    ) -> &'result mut u64 {
        write_argument(
            &mut return_grid_bucket(bucket).rows[
                identity_index(write_index(first))
            ][identity_index(write_index(second))]
        );
        result
    }

    machine return_after_slice_view_repeated_index_statement<
        'bucket, 'result, 'first, 'second
    >(
        bucket: &'bucket mut GridBucket,
        result: &'result mut u64,
        first: &'first mut u64,
        second: &'second mut u64
    ) -> &'result mut u64 {
        write_argument(
            &mut return_grid_bucket(bucket).rows.as_mut_slice()[
                identity_index(write_index(first))
            ][identity_index(write_index(second))]
        );
        result
    }

    machine return_after_deep_slice_view_repeated_index_statement<
        'bucket, 'result
    >(
        bucket: &'bucket mut GridBucket,
        result: &'result mut u64
    ) -> &'result mut u64 {
        write_argument(
            &mut return_grid_bucket(bucket).rows.as_mut_slice()[
                identity_index(identity_index(make_index()))
            ][make_index()]
        );
        result
    }

    machine return_after_recursive_slice_view_repeated_index_statement<
        'bucket, 'result
    >(
        bucket: &'bucket mut GridBucket,
        result: &'result mut u64
    ) -> &'result mut u64 {
        write_argument(
            &mut recursive_grid_bucket(bucket).rows.as_mut_slice()[
                make_index()
            ][make_index()]
        );
        result
    }

    machine return_after_deep_repeated_index_statement<'bucket, 'result, 'first>(
        bucket: &'bucket mut GridBucket,
        result: &'result mut u64,
        first: &'first mut u64
    ) -> &'result mut u64 {
        write_argument(
            &mut return_grid_bucket(bucket).rows[
                identity_index(identity_index(write_index(first)))
            ][make_index()]
        );
        result
    }

    machine Main::return_after_attached_result_indexed_statement(
        &mut self
    ) -> &mut u64 {
        write_argument(
            &mut self.return_attached_cells()[
                identity_index(write_index(&mut self.index_write))
            ]
        );
        &mut self.result
    }

    machine Main::return_after_recursive_attached_indexed_statement(
        &mut self
    ) -> &mut u64 {
        write_argument(&mut self.recursive_attached_cells()[make_index()]);
        &mut self.result
    }

    machine Main::return_after_attached_slice_view_indexed_statement(
        &mut self
    ) -> &mut u64 {
        write_argument(
            &mut self.return_attached_cells().as_mut_slice()[
                identity_index(write_index(&mut self.index_write))
            ]
        );
        &mut self.result
    }

    machine Main::return_after_deep_attached_slice_view_indexed_statement(
        &mut self
    ) -> &mut u64 {
        write_argument(
            &mut self.return_attached_cells().as_mut_slice()[
                identity_index(identity_index(make_index()))
            ]
        );
        &mut self.result
    }

    machine Main::return_after_recursive_attached_slice_view_indexed_statement(
        &mut self
    ) -> &mut u64 {
        write_argument(
            &mut self.recursive_attached_cells().as_mut_slice()[make_index()]
        );
        &mut self.result
    }

    machine Main::return_after_attached_projected_slice_view_indexed_statement(
        &mut self
    ) -> &mut u64 {
        write_argument(
            &mut self.return_attached_bucket().cells.as_mut_slice()[
                identity_index(write_index(&mut self.index_write))
            ]
        );
        &mut self.result
    }

    machine Main::return_after_deep_attached_projected_slice_view_indexed_statement(
        &mut self
    ) -> &mut u64 {
        write_argument(
            &mut self.return_attached_bucket().cells.as_mut_slice()[
                identity_index(identity_index(make_index()))
            ]
        );
        &mut self.result
    }

    machine Main::return_after_recursive_attached_projected_slice_view_indexed_statement(
        &mut self
    ) -> &mut u64 {
        write_argument(
            &mut self.recursive_attached_bucket().cells.as_mut_slice()[make_index()]
        );
        &mut self.result
    }

    machine return_after_deep_alias_indexed_statement<'cells, 'result>(
        cells: &'cells mut [u64; 2],
        result: &'result mut u64
    ) -> &'result mut u64 {
        let alias: &mut [u64; 2] = cells;
        write_argument(
            &mut alias[identity_index(identity_index(make_index()))]
        );
        result
    }

    machine return_after_deep_indexed_statement<'cells, 'result>(
        cells: &'cells mut [u64; 2],
        result: &'result mut u64
    ) -> &'result mut u64 {
        write_argument(
            &mut cells[identity_index(identity_index(make_index()))]
        );
        result
    }

    machine return_after_reborrow_indexed_statement<'cells, 'result, 'write>(
        cells: &'cells mut [u64; 2],
        result: &'result mut u64,
        index_write: &'write mut u64
    ) -> &'result mut u64 {
        write_argument(
            &mut cells[identity_index(write_index(&mut index_write))]
        );
        result
    }

    machine return_after_recursive_indexed_statement<'cells, 'result>(
        cells: &'cells mut [u64; 2],
        result: &'result mut u64
    ) -> &'result mut u64 {
        write_argument(&mut cells[recursive_index()]);
        result
    }

    machine Main::indexed_statement_result(&mut self) {
        let alias: &mut u64 = return_after_indexed_statement(
            &mut self.cells,
            &mut self.result
        );
        alias = 3;
    }

    machine Main::nested_indexed_statement_result(&mut self) {
        let alias: &mut u64 = return_after_nested_indexed_statement(
            &mut self.cells,
            &mut self.result,
            &mut self.index_write
        );
        alias = 3;
    }

    machine Main::slice_view_indexed_statement_result(&mut self) {
        let alias: &mut u64 = return_after_slice_view_indexed_statement(
            &mut self.cells,
            &mut self.result,
            &mut self.index_write
        );
        alias = 3;
    }

    machine Main::deep_slice_view_indexed_statement_result(&mut self) {
        let alias: &mut u64 = return_after_deep_slice_view_indexed_statement(
            &mut self.cells,
            &mut self.result
        );
        alias = 3;
    }

    machine Main::recursive_slice_view_indexed_statement_result(&mut self) {
        let alias: &mut u64 = return_after_recursive_slice_view_indexed_statement(
            &mut self.cells,
            &mut self.result
        );
        alias = 3;
    }

    machine Main::alias_slice_view_indexed_statement_result(&mut self) {
        let alias: &mut u64 = return_after_alias_slice_view_indexed_statement(
            &mut self.cells,
            &mut self.result,
            &mut self.index_write
        );
        alias = 3;
    }

    machine Main::deep_alias_slice_view_indexed_statement_result(&mut self) {
        let alias: &mut u64 = return_after_deep_alias_slice_view_indexed_statement(
            &mut self.cells,
            &mut self.result
        );
        alias = 3;
    }

    machine Main::recursive_alias_slice_view_indexed_statement_result(&mut self) {
        let alias: &mut u64 = return_after_recursive_alias_slice_view_indexed_statement(
            &mut self.cells,
            &mut self.result
        );
        alias = 3;
    }

    machine Main::member_alias_slice_view_indexed_statement_result(&mut self) {
        let alias: &mut u64 = return_after_member_alias_slice_view_indexed_statement(
            &mut self.bucket,
            &mut self.result,
            &mut self.index_write
        );
        alias = 3;
    }

    machine Main::deep_member_alias_slice_view_indexed_statement_result(&mut self) {
        let alias: &mut u64 = return_after_deep_member_alias_slice_view_indexed_statement(
            &mut self.bucket,
            &mut self.result
        );
        alias = 3;
    }

    machine Main::recursive_member_alias_slice_view_indexed_statement_result(&mut self) {
        let alias: &mut u64 = return_after_recursive_member_alias_slice_view_indexed_statement(
            &mut self.bucket,
            &mut self.result
        );
        alias = 3;
    }

    machine Main::helper_slice_view_indexed_statement_result(&mut self) {
        let alias: &mut u64 = return_after_helper_slice_view_indexed_statement(
            &mut self.cells,
            &mut self.result,
            &mut self.index_write
        );
        alias = 3;
    }

    machine Main::deep_helper_slice_view_indexed_statement_result(&mut self) {
        let alias: &mut u64 = return_after_deep_helper_slice_view_indexed_statement(
            &mut self.cells,
            &mut self.result
        );
        alias = 3;
    }

    machine Main::recursive_helper_slice_view_indexed_statement_result(&mut self) {
        let alias: &mut u64 = return_after_recursive_helper_slice_view_indexed_statement(
            &mut self.cells,
            &mut self.result
        );
        alias = 3;
    }

    machine Main::alias_indexed_statement_result(&mut self) {
        let alias: &mut u64 = return_after_alias_indexed_statement(
            &mut self.cells,
            &mut self.result,
            &mut self.index_write
        );
        alias = 3;
    }

    machine Main::helper_result_indexed_statement_result(&mut self) {
        let alias: &mut u64 = return_after_helper_result_indexed_statement(
            &mut self.cells,
            &mut self.result,
            &mut self.index_write
        );
        alias = 3;
    }

    machine Main::recursive_helper_indexed_statement_result(&mut self) {
        let alias: &mut u64 = return_after_recursive_helper_indexed_statement(
            &mut self.cells,
            &mut self.result
        );
        alias = 3;
    }

    machine Main::projected_helper_indexed_statement_result(&mut self) {
        let alias: &mut u64 = return_after_projected_helper_indexed_statement(
            &mut self.bucket,
            &mut self.result,
            &mut self.index_write
        );
        alias = 3;
    }

    machine Main::projected_helper_slice_view_indexed_statement_result(&mut self) {
        let alias: &mut u64 = return_after_projected_helper_slice_view_indexed_statement(
            &mut self.bucket,
            &mut self.result,
            &mut self.index_write
        );
        alias = 3;
    }

    machine Main::deep_projected_helper_slice_view_indexed_statement_result(&mut self) {
        let alias: &mut u64 = return_after_deep_projected_helper_slice_view_indexed_statement(
            &mut self.bucket,
            &mut self.result
        );
        alias = 3;
    }

    machine Main::recursive_projected_helper_slice_view_indexed_statement_result(
        &mut self
    ) {
        let alias: &mut u64 =
            return_after_recursive_projected_helper_slice_view_indexed_statement(
                &mut self.bucket,
                &mut self.result
            );
        alias = 3;
    }

    machine Main::slice_view_member_after_index_statement_result(&mut self) {
        let alias: &mut u64 = return_after_slice_view_member_after_index_statement(
            &mut self.cell_bucket,
            &mut self.result,
            &mut self.index_write
        );
        alias = 3;
    }

    machine Main::deep_slice_view_member_after_index_statement_result(&mut self) {
        let alias: &mut u64 = return_after_deep_slice_view_member_after_index_statement(
            &mut self.cell_bucket,
            &mut self.result
        );
        alias = 3;
    }

    machine Main::recursive_slice_view_member_after_index_statement_result(&mut self) {
        let alias: &mut u64 = return_after_recursive_slice_view_member_after_index_statement(
            &mut self.cell_bucket,
            &mut self.result
        );
        alias = 3;
    }

    machine Main::recursive_projected_helper_statement_result(&mut self) {
        let alias: &mut u64 = return_after_recursive_projected_helper_statement(
            &mut self.bucket,
            &mut self.result
        );
        alias = 3;
    }

    machine Main::member_after_index_statement_result(&mut self) {
        let alias: &mut u64 = return_after_member_after_index_statement(
            &mut self.cell_bucket,
            &mut self.result,
            &mut self.index_write
        );
        alias = 3;
    }

    machine Main::recursive_member_after_index_statement_result(&mut self) {
        let alias: &mut u64 = return_after_recursive_member_after_index_statement(
            &mut self.cell_bucket,
            &mut self.result
        );
        alias = 3;
    }

    machine Main::repeated_index_statement_result(&mut self) {
        let alias: &mut u64 = return_after_repeated_index_statement(
            &mut self.grid_bucket,
            &mut self.result,
            &mut self.index_write,
            &mut self.second_index_write
        );
        alias = 3;
    }

    machine Main::slice_view_repeated_index_statement_result(&mut self) {
        let alias: &mut u64 = return_after_slice_view_repeated_index_statement(
            &mut self.grid_bucket,
            &mut self.result,
            &mut self.index_write,
            &mut self.second_index_write
        );
        alias = 3;
    }

    machine Main::deep_slice_view_repeated_index_statement_result(&mut self) {
        let alias: &mut u64 = return_after_deep_slice_view_repeated_index_statement(
            &mut self.grid_bucket,
            &mut self.result
        );
        alias = 3;
    }

    machine Main::recursive_slice_view_repeated_index_statement_result(&mut self) {
        let alias: &mut u64 = return_after_recursive_slice_view_repeated_index_statement(
            &mut self.grid_bucket,
            &mut self.result
        );
        alias = 3;
    }

    machine Main::deep_repeated_index_statement_result(&mut self) {
        let alias: &mut u64 = return_after_deep_repeated_index_statement(
            &mut self.grid_bucket,
            &mut self.result,
            &mut self.index_write
        );
        alias = 3;
    }

    machine Main::attached_result_indexed_statement_result(&mut self) {
        let alias: &mut u64 = self.return_after_attached_result_indexed_statement();
        alias = 3;
    }

    machine Main::recursive_attached_indexed_statement_result(&mut self) {
        let alias: &mut u64 =
            self.return_after_recursive_attached_indexed_statement();
        alias = 3;
    }

    machine Main::attached_slice_view_indexed_statement_result(&mut self) {
        let alias: &mut u64 =
            self.return_after_attached_slice_view_indexed_statement();
        alias = 3;
    }

    machine Main::deep_attached_slice_view_indexed_statement_result(&mut self) {
        let alias: &mut u64 =
            self.return_after_deep_attached_slice_view_indexed_statement();
        alias = 3;
    }

    machine Main::recursive_attached_slice_view_indexed_statement_result(&mut self) {
        let alias: &mut u64 =
            self.return_after_recursive_attached_slice_view_indexed_statement();
        alias = 3;
    }

    machine Main::attached_projected_slice_view_indexed_statement_result(&mut self) {
        let alias: &mut u64 =
            self.return_after_attached_projected_slice_view_indexed_statement();
        alias = 3;
    }

    machine Main::deep_attached_projected_slice_view_indexed_statement_result(&mut self) {
        let alias: &mut u64 =
            self.return_after_deep_attached_projected_slice_view_indexed_statement();
        alias = 3;
    }

    machine Main::recursive_attached_projected_slice_view_indexed_statement_result(
        &mut self
    ) {
        let alias: &mut u64 =
            self.return_after_recursive_attached_projected_slice_view_indexed_statement();
        alias = 3;
    }

    machine Main::deep_alias_indexed_statement_result(&mut self) {
        let alias: &mut u64 = return_after_deep_alias_indexed_statement(
            &mut self.cells,
            &mut self.result
        );
        alias = 3;
    }

    machine Main::deep_indexed_statement_result(&mut self) {
        let alias: &mut u64 = return_after_deep_indexed_statement(
            &mut self.cells,
            &mut self.result
        );
        alias = 3;
    }

    machine Main::reborrow_indexed_statement_result(&mut self) {
        let alias: &mut u64 = return_after_reborrow_indexed_statement(
            &mut self.cells,
            &mut self.result,
            &mut self.index_write
        );
        alias = 3;
    }

    machine Main::recursive_indexed_statement_result(&mut self) {
        let alias: &mut u64 = return_after_recursive_indexed_statement(
            &mut self.cells,
            &mut self.result
        );
        alias = 3;
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
            "Main::indexed_statement_result",
            vec!["self.cells", "self.result"],
        ),
        (
            "Main::nested_indexed_statement_result",
            vec!["self.cells", "self.index_write", "self.result"],
        ),
        (
            "Main::slice_view_indexed_statement_result",
            vec!["self.cells", "self.index_write", "self.result"],
        ),
        (
            "Main::alias_slice_view_indexed_statement_result",
            vec!["self.cells", "self.index_write", "self.result"],
        ),
        (
            "Main::member_alias_slice_view_indexed_statement_result",
            vec!["self.bucket.cells", "self.index_write", "self.result"],
        ),
        (
            "Main::helper_slice_view_indexed_statement_result",
            vec!["self.cells", "self.index_write", "self.result"],
        ),
        (
            "Main::alias_indexed_statement_result",
            vec!["self.cells", "self.index_write", "self.result"],
        ),
        (
            "Main::helper_result_indexed_statement_result",
            vec!["self.cells", "self.index_write", "self.result"],
        ),
        (
            "Main::attached_result_indexed_statement_result",
            vec!["self.cells", "self.index_write", "self.result"],
        ),
        (
            "Main::attached_slice_view_indexed_statement_result",
            vec!["self.cells", "self.index_write", "self.result"],
        ),
        (
            "Main::attached_projected_slice_view_indexed_statement_result",
            vec!["self.bucket.cells", "self.index_write", "self.result"],
        ),
        (
            "Main::projected_helper_indexed_statement_result",
            vec!["self.bucket.cells", "self.index_write", "self.result"],
        ),
        (
            "Main::projected_helper_slice_view_indexed_statement_result",
            vec!["self.bucket.cells", "self.index_write", "self.result"],
        ),
        (
            "Main::member_after_index_statement_result",
            vec!["self.cell_bucket.cells", "self.index_write", "self.result"],
        ),
        (
            "Main::slice_view_member_after_index_statement_result",
            vec!["self.cell_bucket.cells", "self.index_write", "self.result"],
        ),
        (
            "Main::repeated_index_statement_result",
            vec![
                "self.grid_bucket.rows",
                "self.index_write",
                "self.result",
                "self.second_index_write",
            ],
        ),
        (
            "Main::slice_view_repeated_index_statement_result",
            vec![
                "self.grid_bucket.rows",
                "self.index_write",
                "self.result",
                "self.second_index_write",
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
            "{name} must publish the coarse argument, index writes, and returned-place write"
        );
    }

    for name in [
        "Main::deep_indexed_statement_result",
        "Main::deep_alias_indexed_statement_result",
        "Main::reborrow_indexed_statement_result",
        "Main::recursive_indexed_statement_result",
        "Main::deep_slice_view_indexed_statement_result",
        "Main::recursive_slice_view_indexed_statement_result",
        "Main::deep_alias_slice_view_indexed_statement_result",
        "Main::recursive_alias_slice_view_indexed_statement_result",
        "Main::deep_member_alias_slice_view_indexed_statement_result",
        "Main::recursive_member_alias_slice_view_indexed_statement_result",
        "Main::deep_helper_slice_view_indexed_statement_result",
        "Main::recursive_helper_slice_view_indexed_statement_result",
        "Main::recursive_helper_indexed_statement_result",
        "Main::recursive_attached_indexed_statement_result",
        "Main::deep_attached_slice_view_indexed_statement_result",
        "Main::recursive_attached_slice_view_indexed_statement_result",
        "Main::deep_attached_projected_slice_view_indexed_statement_result",
        "Main::recursive_attached_projected_slice_view_indexed_statement_result",
        "Main::recursive_projected_helper_statement_result",
        "Main::deep_projected_helper_slice_view_indexed_statement_result",
        "Main::recursive_projected_helper_slice_view_indexed_statement_result",
        "Main::deep_slice_view_member_after_index_statement_result",
        "Main::recursive_slice_view_member_after_index_statement_result",
        "Main::recursive_member_after_index_statement_result",
        "Main::deep_repeated_index_statement_result",
        "Main::deep_slice_view_repeated_index_statement_result",
        "Main::recursive_slice_view_repeated_index_statement_result",
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
            "{name} must remain opaque outside the bounded indexed-argument rung"
        );
    }
}

#[test]
fn transparent_returned_place_accepts_bounded_isolated_scratch_initializers() {
    let source = r#"
    data Main {
        value: u64;
    }

    machine make_scratch() -> u64 {
        0
    }

    machine scratch_from(value: u64) -> u64 {
        value
    }

    machine write_scratch(value: &mut u64) -> u64 {
        value = 1;
        0
    }

    machine mixed_scratch(first: &mut u64, second: &mut u64) -> u64 {
        first = 1;
        second = 2;
        0
    }

    machine recursive_scratch() -> u64 {
        recursive_scratch()
    }

    machine return_with_nested_scratch(value: &mut u64) -> &mut u64 {
        let scratch: u64 = scratch_from(make_scratch());
        value
    }

    machine return_with_nested_write_scratch(value: &mut u64) -> &mut u64 {
        let mut prior: u64 = 0;
        let scratch: u64 = scratch_from(write_scratch(&mut prior));
        value
    }

    machine return_with_deep_scratch(value: &mut u64) -> &mut u64 {
        let scratch: u64 = scratch_from(scratch_from(make_scratch()));
        value
    }

    machine return_with_external_write_scratch(value: &mut u64) -> &mut u64 {
        let mut prior: u64 = 0;
        let scratch: u64 = scratch_from(mixed_scratch(&mut prior, value));
        value
    }

    machine return_with_recursive_scratch(value: &mut u64) -> &mut u64 {
        let scratch: u64 = recursive_scratch();
        value
    }

    machine Main::nested_scratch_result(&mut self) {
        let alias: &mut u64 = return_with_nested_scratch(&mut self.value);
        alias = 3;
    }

    machine Main::nested_write_scratch_result(&mut self) {
        let alias: &mut u64 = return_with_nested_write_scratch(&mut self.value);
        alias = 3;
    }

    machine Main::deep_scratch_result(&mut self) {
        let alias: &mut u64 = return_with_deep_scratch(&mut self.value);
        alias = 3;
    }

    machine Main::external_write_scratch_result(&mut self) {
        let alias: &mut u64 = return_with_external_write_scratch(&mut self.value);
        alias = 3;
    }

    machine Main::recursive_scratch_result(&mut self) {
        let alias: &mut u64 = return_with_recursive_scratch(&mut self.value);
        alias = 3;
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
        "Main::nested_scratch_result",
        "Main::nested_write_scratch_result",
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
            Some(["self.value".to_owned()].as_slice()),
            "{name} must hide writes confined to earlier caller-isolated scratch roots"
        );
    }

    for name in [
        "Main::deep_scratch_result",
        "Main::external_write_scratch_result",
        "Main::recursive_scratch_result",
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
            "{name} must remain opaque outside the bounded isolated-scratch rung"
        );
    }
}

#[test]
fn mutable_slice_views_preserve_array_storage_origins() {
    let source = r#"
    data Main {
        cells: [u64; 2];
    }

    machine return_slice(cells: &mut [u64; 2]) -> &mut [u64] {
        let view: &mut [u64] = cells.as_mut_slice();
        view
    }

    machine return_recursive_slice(cells: &mut [u64; 2]) -> &mut [u64] {
        return_recursive_slice(cells)
    }

    machine write_slice(view: &mut [u64]) {
        transition view.len > 0 {
            true -> write(view)
            false -> {}
        }

        state write(view: &mut [u64]) {
            view[0] = 1;
        }
    }

    machine write_slices(first: &mut [u64], second: &mut [u64]) {
        write_slice(first);
        write_slice(second);
    }

    machine noop() {}

    machine write_value(value: &mut u64) {
        value = 1;
    }

    machine return_value(value: &mut u64) -> &mut u64 {
        value
    }

    machine return_after_discarded_slice_view<'value, 'cells>(
        value: &'value mut u64,
        cells: &'cells mut [u64; 2]
    ) -> &'value mut u64 {
        cells.as_mut_slice().len;
        value
    }

    machine return_after_discarded_shared_slice_view<'value, 'cells>(
        value: &'value mut u64,
        cells: &'cells [u64; 2]
    ) -> &'value mut u64 {
        cells.as_slice().len;
        value
    }

    machine return_after_empty_statement_call(value: &mut u64) -> &mut u64 {
        noop();
        value
    }

    machine return_after_write_statement_call(value: &mut u64) -> &mut u64 {
        write_value(value);
        value
    }

    machine return_after_binding_reborrow_statement_call(value: &mut u64) -> &mut u64 {
        write_value(&mut value);
        value
    }

    machine return_after_direct_call_argument<'value, 'cells>(
        value: &'value mut u64,
        cells: &'cells mut [u64; 2]
    ) -> &'value mut u64 {
        write_slice(return_slice(cells));
        value
    }

    machine return_after_recursive_call_argument<'value, 'cells>(
        value: &'value mut u64,
        cells: &'cells mut [u64; 2]
    ) -> &'value mut u64 {
        write_slice(return_recursive_slice(cells));
        value
    }

    machine return_after_deep_call_argument(value: &mut u64) -> &mut u64 {
        write_value(return_value(return_value(value)));
        value
    }

    machine return_after_too_deep_call_argument(value: &mut u64) -> &mut u64 {
        write_value(return_value(return_value(return_value(value))));
        value
    }

    machine return_after_sibling_call_arguments<'value, 'first, 'second>(
        value: &'value mut u64,
        first: &'first mut [u64; 2],
        second: &'second mut [u64; 2]
    ) -> &'value mut u64 {
        write_slices(return_slice(first), return_slice(second));
        value
    }

    machine return_after_mixed_sibling_call_arguments<'value, 'first, 'second>(
        value: &'value mut u64,
        first: &'first mut [u64; 2],
        second: &'second mut [u64; 2]
    ) -> &'value mut u64 {
        write_slices(return_slice(first), return_recursive_slice(second));
        value
    }

    machine Main::direct_view(&mut self) {
        let view: &mut [u64] = self.cells.as_mut_slice();
        view[0] = 1;
    }

    machine Main::helper_view(&mut self) {
        let view: &mut [u64] = return_slice(&mut self.cells);
        view[0] = 1;
    }

    machine Main::recursive_view(&mut self) {
        let view: &mut [u64] = return_recursive_slice(&mut self.cells);
        view[0] = 1;
    }

    machine Main::statement_view(&mut self) {
        write_slice(self.cells.as_mut_slice());
    }

    machine Main::recursive_statement_view(&mut self) {
        write_slice(return_recursive_slice(&mut self.cells));
    }

    machine Main::discarded_slice_view(&mut self) {
        let alias: &mut u64 =
            return_after_discarded_slice_view(&mut self.value, &mut self.cells);
        alias = 1;
    }

    machine Main::empty_statement_call(&mut self) {
        let alias: &mut u64 = return_after_empty_statement_call(&mut self.value);
        alias = 1;
    }

    machine Main::write_statement_call(&mut self) {
        let alias: &mut u64 = return_after_write_statement_call(&mut self.value);
        alias = 1;
    }

    machine Main::binding_reborrow_statement_call(&mut self) {
        let alias: &mut u64 =
            return_after_binding_reborrow_statement_call(&mut self.value);
        alias = 1;
    }

    machine Main::direct_call_argument_statement_call(&mut self) {
        let alias: &mut u64 =
            return_after_direct_call_argument(&mut self.value, &mut self.cells);
        alias = 1;
    }

    machine Main::recursive_call_argument_statement_call(&mut self) {
        let alias: &mut u64 =
            return_after_recursive_call_argument(&mut self.value, &mut self.cells);
        alias = 1;
    }

    machine Main::deep_call_argument_statement_call(&mut self) {
        let alias: &mut u64 = return_after_deep_call_argument(&mut self.value);
        alias = 1;
    }

    machine Main::too_deep_call_argument_statement_call(&mut self) {
        let alias: &mut u64 = return_after_too_deep_call_argument(&mut self.value);
        alias = 1;
    }

    machine Main::sibling_call_arguments_statement_call(&mut self) {
        let alias: &mut u64 = return_after_sibling_call_arguments(
            &mut self.value,
            &mut self.cells,
            &mut self.other_cells
        );
        alias = 1;
    }

    machine Main::mixed_sibling_call_arguments_statement_call(&mut self) {
        let alias: &mut u64 = return_after_mixed_sibling_call_arguments(
            &mut self.value,
            &mut self.cells,
            &mut self.other_cells
        );
        alias = 1;
    }

    machine Main::discarded_shared_slice_view(&mut self) {
        let alias: &mut u64 =
            return_after_discarded_shared_slice_view(&mut self.value, self.cells);
        alias = 1;
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
        "Main::direct_view",
        "Main::helper_view",
        "Main::statement_view",
        "Main::discarded_slice_view",
        "Main::discarded_shared_slice_view",
        "Main::empty_statement_call",
        "Main::write_statement_call",
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
                [if matches!(
                    name,
                    "Main::discarded_slice_view"
                        | "Main::discarded_shared_slice_view"
                        | "Main::empty_statement_call"
                        | "Main::write_statement_call"
                ) {
                    "self.value"
                } else {
                    "self.cells"
                }
                .to_owned()]
                .as_slice()
            ),
            "{name} must retain the mutable view's array storage origin"
        );
    }

    let recursive = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::recursive_view")
        .expect("recursive view caller");
    let recursive_entry = typed
        .machine_states(recursive)
        .first()
        .expect("recursive view caller entry state");
    assert!(
        !resolver
            .inferred_state_write_frame(recursive, recursive_entry)
            .is_complete(),
        "an opaque recursive slice producer must remain a frame fence"
    );

    let recursive_statement = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::recursive_statement_view")
        .expect("recursive statement-view caller");
    let recursive_statement_entry = typed
        .machine_states(recursive_statement)
        .first()
        .expect("recursive statement-view caller entry state");
    let recursive_statement_frame =
        resolver.inferred_state_write_frame(recursive_statement, recursive_statement_entry);
    assert!(
        recursive_statement_frame
            .complete_paths()
            .is_none_or(|paths| paths.iter().any(|path| path == "self")),
        "an opaque recursive statement argument must retain a whole-receiver fence"
    );

    let binding_reborrow_statement_call = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::binding_reborrow_statement_call")
        .expect("binding-reborrow statement-call caller");
    let binding_reborrow_statement_call_entry = typed
        .machine_states(binding_reborrow_statement_call)
        .first()
        .expect("binding-reborrow statement-call caller entry state");
    assert!(
        !resolver
            .inferred_state_write_frame(
                binding_reborrow_statement_call,
                binding_reborrow_statement_call_entry,
            )
            .is_complete(),
        "an explicit mutable-reference binding reborrow must keep the helper relation opaque"
    );

    let direct_call_argument_statement_call = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::direct_call_argument_statement_call")
        .expect("direct call-argument statement caller");
    let direct_call_argument_statement_call_entry = typed
        .machine_states(direct_call_argument_statement_call)
        .first()
        .expect("direct call-argument statement caller entry state");
    assert_eq!(
        resolver
            .inferred_state_write_frame(
                direct_call_argument_statement_call,
                direct_call_argument_statement_call_entry,
            )
            .complete_paths(),
        Some(["self.cells".to_owned(), "self.value".to_owned()].as_slice()),
        "one exact direct value-call argument must preserve both its write and the returned origin"
    );

    let recursive_call_argument_statement_call = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::recursive_call_argument_statement_call")
        .expect("recursive call-argument statement caller");
    let recursive_call_argument_statement_call_entry = typed
        .machine_states(recursive_call_argument_statement_call)
        .first()
        .expect("recursive call-argument statement caller entry state");
    assert!(
        !resolver
            .inferred_state_write_frame(
                recursive_call_argument_statement_call,
                recursive_call_argument_statement_call_entry,
            )
            .is_complete(),
        "an opaque recursive value-call argument must remain a returned-place fence"
    );

    let deep_call_argument_statement_call = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::deep_call_argument_statement_call")
        .expect("deep call-argument statement caller");
    let deep_call_argument_statement_call_entry = typed
        .machine_states(deep_call_argument_statement_call)
        .first()
        .expect("deep call-argument statement caller entry state");
    assert_eq!(
        resolver
            .inferred_state_write_frame(
                deep_call_argument_statement_call,
                deep_call_argument_statement_call_entry,
            )
            .complete_paths(),
        Some(["self.value".to_owned()].as_slice()),
        "a two-level exact value-call argument tree must preserve the returned origin"
    );

    let too_deep_call_argument_statement_call = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::too_deep_call_argument_statement_call")
        .expect("too-deep call-argument statement caller");
    let too_deep_call_argument_statement_call_entry = typed
        .machine_states(too_deep_call_argument_statement_call)
        .first()
        .expect("too-deep call-argument statement caller entry state");
    assert!(
        !resolver
            .inferred_state_write_frame(
                too_deep_call_argument_statement_call,
                too_deep_call_argument_statement_call_entry,
            )
            .is_complete(),
        "a value-call argument tree deeper than two calls must remain opaque"
    );

    let sibling_call_arguments_statement_call = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::sibling_call_arguments_statement_call")
        .expect("sibling call-arguments statement caller");
    let sibling_call_arguments_statement_call_entry = typed
        .machine_states(sibling_call_arguments_statement_call)
        .first()
        .expect("sibling call-arguments statement caller entry state");
    assert_eq!(
        resolver
            .inferred_state_write_frame(
                sibling_call_arguments_statement_call,
                sibling_call_arguments_statement_call_entry,
            )
            .complete_paths(),
        Some(
            [
                "self.cells".to_owned(),
                "self.other_cells".to_owned(),
                "self.value".to_owned(),
            ]
            .as_slice()
        ),
        "exact sibling value-call arguments must compose their writes and the returned origin"
    );

    let mixed_sibling_call_arguments_statement_call = typed
        .machines()
        .iter()
        .find(|machine| {
            machine.name.as_str() == "Main::mixed_sibling_call_arguments_statement_call"
        })
        .expect("mixed sibling call-arguments statement caller");
    let mixed_sibling_call_arguments_statement_call_entry = typed
        .machine_states(mixed_sibling_call_arguments_statement_call)
        .first()
        .expect("mixed sibling call-arguments statement caller entry state");
    assert!(
        !resolver
            .inferred_state_write_frame(
                mixed_sibling_call_arguments_statement_call,
                mixed_sibling_call_arguments_statement_call_entry,
            )
            .is_complete(),
        "one opaque sibling value-call argument must fence the whole returned-place relation"
    );
}
