use crate::tests::front_end::typed_program;

#[test]
fn transparent_returned_place_accepts_complete_indexed_statement_arguments() {
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

    let typed = typed_program(source);
    let resolver = validation::CallFrameResolver::new(&typed).expect("valid symbol cache");

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
        (
            "Main::deep_indexed_statement_result",
            vec!["self.cells", "self.result"],
        ),
        (
            "Main::deep_alias_indexed_statement_result",
            vec!["self.cells", "self.result"],
        ),
        (
            "Main::deep_slice_view_indexed_statement_result",
            vec!["self.cells", "self.result"],
        ),
        (
            "Main::deep_alias_slice_view_indexed_statement_result",
            vec!["self.cells", "self.result"],
        ),
        (
            "Main::deep_member_alias_slice_view_indexed_statement_result",
            vec!["self.bucket.cells", "self.result"],
        ),
        (
            "Main::deep_helper_slice_view_indexed_statement_result",
            vec!["self.cells", "self.result"],
        ),
        (
            "Main::deep_attached_slice_view_indexed_statement_result",
            vec!["self.cells", "self.result"],
        ),
        (
            "Main::deep_attached_projected_slice_view_indexed_statement_result",
            vec!["self.bucket.cells", "self.result"],
        ),
        (
            "Main::deep_projected_helper_slice_view_indexed_statement_result",
            vec!["self.bucket.cells", "self.result"],
        ),
        (
            "Main::deep_slice_view_member_after_index_statement_result",
            vec!["self.cell_bucket.cells", "self.result"],
        ),
        (
            "Main::deep_repeated_index_statement_result",
            vec!["self.grid_bucket.rows", "self.index_write", "self.result"],
        ),
        (
            "Main::deep_slice_view_repeated_index_statement_result",
            vec!["self.grid_bucket.rows", "self.result"],
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
        "Main::reborrow_indexed_statement_result",
        "Main::recursive_indexed_statement_result",
        "Main::recursive_slice_view_indexed_statement_result",
        "Main::recursive_alias_slice_view_indexed_statement_result",
        "Main::recursive_member_alias_slice_view_indexed_statement_result",
        "Main::recursive_helper_slice_view_indexed_statement_result",
        "Main::recursive_helper_indexed_statement_result",
        "Main::recursive_attached_indexed_statement_result",
        "Main::recursive_attached_slice_view_indexed_statement_result",
        "Main::recursive_attached_projected_slice_view_indexed_statement_result",
        "Main::recursive_projected_helper_statement_result",
        "Main::recursive_projected_helper_slice_view_indexed_statement_result",
        "Main::recursive_slice_view_member_after_index_statement_result",
        "Main::recursive_member_after_index_statement_result",
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
            "{name} must remain opaque without a complete non-rebinding indexed argument"
        );
    }
}

#[test]
fn transparent_returned_place_accepts_finite_isolated_scratch_values() {
    let mut source = r#"
    data Scratch { value: u64; audit: u64; }
    data ScratchChoice { case Filled(record: Scratch); case Empty; }

    data Main {
        value: u64;
    }

    machine make_scratch() -> u64 {
        0
    }

    machine scratch_from(value: u64) -> u64 {
        value
    }

    machine scratch_pair(first: u64, second: u64) -> u64 {
        first
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

    machine return_with_deep_write_scratch(value: &mut u64) -> &mut u64 {
        let mut prior: u64 = 0;
        let scratch: u64 = scratch_from(scratch_from(scratch_from(
            write_scratch(&mut prior)
        )));
        value
    }

    machine return_with_sibling_scratch(value: &mut u64) -> &mut u64 {
        let mut first: u64 = 0;
        let mut second: u64 = 0;
        let scratch: u64 = scratch_pair(
            scratch_from(scratch_from(write_scratch(&mut first))),
            scratch_from(scratch_from(write_scratch(&mut second)))
        );
        value
    }

    machine return_with_computed_scratch(value: &mut u64) -> &mut u64 {
        let scratch: u64 = scratch_from(make_scratch()) + 1;
        value
    }

    machine return_with_external_write_scratch(value: &mut u64) -> &mut u64 {
        let mut prior: u64 = 0;
        let scratch: u64 = scratch_from(mixed_scratch(&mut prior, value));
        value
    }

    machine return_with_recursive_scratch(value: &mut u64) -> &mut u64 {
        let scratch: u64 = scratch_from(scratch_from(recursive_scratch()));
        value
    }

    machine return_with_mixed_sibling_scratch(value: &mut u64) -> &mut u64 {
        let mut prior: u64 = 0;
        let scratch: u64 = scratch_pair(
            scratch_from(scratch_from(write_scratch(&mut prior))),
            scratch_from(scratch_from(write_scratch(value)))
        );
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

    machine Main::deep_write_scratch_result(&mut self) {
        let alias: &mut u64 = return_with_deep_write_scratch(&mut self.value);
        alias = 3;
    }

    machine Main::sibling_scratch_result(&mut self) {
        let alias: &mut u64 = return_with_sibling_scratch(&mut self.value);
        alias = 3;
    }

    machine Main::computed_scratch_result(&mut self) {
        let alias: &mut u64 = return_with_computed_scratch(&mut self.value);
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

    machine Main::mixed_sibling_scratch_result(&mut self) {
        let alias: &mut u64 = return_with_mixed_sibling_scratch(&mut self.value);
        alias = 3;
    }

    machine return_private_scratch_alias(value: &mut u64) -> &mut u64 {
        let mut prior: u64 = 0;
        let alias: &mut u64 = &mut prior;
        alias
    }

    machine Main::escaped_local_scratch_result(&mut self) {
        let alias: &mut u64 = return_private_scratch_alias(&mut self.value);
        alias = 3;
    }
    "#
    .to_owned();

    let additional_cases = [
        (
            "computed_write",
            "let scratch: u64 = ~(write_scratch(&mut prior) + 1);",
            true,
        ),
        (
            "record",
            "let scratch: Scratch = Scratch { value: write_scratch(&mut prior) + 1, audit: make_scratch() };",
            true,
        ),
        (
            "selected_case",
            "let scratch: ScratchChoice = ScratchChoice::Filled { record: Scratch { value: write_scratch(&mut prior) + 1, audit: make_scratch() } };",
            true,
        ),
        (
            "nested_array",
            "let scratch: [[Scratch; 1]; 2] = [[Scratch { value: write_scratch(&mut prior) + 1, audit: make_scratch() }], [Scratch { value: make_scratch(), audit: write_scratch(&mut prior) + 1 }]];",
            true,
        ),
        (
            "local_alias",
            "let alias: &mut u64 = &mut prior; let scratch: Scratch = Scratch { value: write_scratch(alias) + 1, audit: make_scratch() };",
            true,
        ),
        (
            "projected_alias_chain",
            "let mut record: Scratch = Scratch { value: 0, audit: 0 }; let whole: &mut Scratch = &mut record; let member: &mut u64 = &mut whole.value; let chain: &mut u64 = member; let scratch: u64 = write_scratch(chain) + 1;",
            true,
        ),
        (
            "caller_alias",
            "let alias: &mut u64 = value; let scratch: Scratch = Scratch { value: write_scratch(&mut prior) + 1, audit: write_scratch(alias) + 1 };",
            false,
        ),
        (
            "record_external",
            "let scratch: Scratch = Scratch { value: write_scratch(&mut prior) + 1, audit: write_scratch(value) + 1 };",
            false,
        ),
        (
            "case_recursive",
            "let scratch: ScratchChoice = ScratchChoice::Filled { record: Scratch { value: write_scratch(&mut prior) + 1, audit: recursive_scratch() + 1 } };",
            false,
        ),
        (
            "array_reborrow",
            "let mut alias: &mut u64 = &mut prior; let scratch: [[u64; 2]; 1] = [[write_scratch(&mut prior) + 1, write_scratch(&mut alias) + 1]];",
            false,
        ),
    ];
    for (name, initializer, _) in additional_cases {
        source.push_str(&format!(
            "machine return_with_{name}_scratch(value: &mut u64) -> &mut u64 {{
                let mut prior: u64 = 0;
                {initializer}
                value
            }}
            machine Main::{name}_scratch_result(&mut self) {{
                let alias: &mut u64 = return_with_{name}_scratch(&mut self.value);
                alias = 3;
            }}"
        ));
    }

    let typed = typed_program(&source);
    let resolver = validation::CallFrameResolver::new(&typed).expect("valid symbol cache");

    for name in [
        "Main::nested_scratch_result",
        "Main::nested_write_scratch_result",
        "Main::deep_scratch_result",
        "Main::deep_write_scratch_result",
        "Main::sibling_scratch_result",
        "Main::computed_scratch_result",
    ]
    .into_iter()
    .map(str::to_owned)
    .chain(
        additional_cases
            .into_iter()
            .filter(|(_, _, complete)| *complete)
            .map(|(name, _, _)| format!("Main::{name}_scratch_result")),
    ) {
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
        "Main::external_write_scratch_result",
        "Main::recursive_scratch_result",
        "Main::mixed_sibling_scratch_result",
        "Main::escaped_local_scratch_result",
    ]
    .into_iter()
    .map(str::to_owned)
    .chain(
        additional_cases
            .into_iter()
            .filter(|(_, _, complete)| !*complete)
            .map(|(name, _, _)| format!("Main::{name}_scratch_result")),
    ) {
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
            "{name} must remain opaque without a complete non-rebinding initializer confined to isolated roots"
        );
    }
}
