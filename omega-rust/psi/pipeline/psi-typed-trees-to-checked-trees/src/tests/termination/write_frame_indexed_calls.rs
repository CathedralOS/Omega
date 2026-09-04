use super::*;

#[test]
fn transparent_returned_index_frame_accepts_a_finite_exact_call_tree() {
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

    data Row {
        cells: [u64; 2];
    }

    data Main {
        value: u64;
        other_value: u64;
        cells: [u64; 2];
        matrix: [[u64; 2]; 2];
        bucket: Bucket;
        cell_bucket: CellBucket;
        grid_bucket: GridBucket;
        row_items: [Row; 2];
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

    machine return_row(row: &mut Row) -> &mut Row {
        row
    }

    machine return_bucket(bucket: &mut Bucket) -> &mut Bucket {
        bucket
    }

    machine recursive_bucket(bucket: &mut Bucket) -> &mut Bucket {
        recursive_bucket(bucket)
    }

    machine Main::return_attached_bucket(&mut self) -> &mut Bucket {
        &mut self.bucket
    }

    machine Main::recursive_attached_bucket(&mut self) -> &mut Bucket {
        self.recursive_attached_bucket()
    }

    machine return_local_index(cells: &mut [u64; 2]) -> &mut u64 {
        let index: u64 = 0;
        &mut cells[index]
    }

    machine return_call_index(cells: &mut [u64; 2]) -> &mut u64 {
        &mut cells[make_index()]
    }

    machine return_write_call_index<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut u64 {
        &mut cells[write_index(value)]
    }

    machine return_nested_call_index(cells: &mut [u64; 2]) -> &mut u64 {
        &mut cells[identity_index(make_index())]
    }

    machine return_nested_write_call_index<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut u64 {
        &mut cells[identity_index(write_index(value))]
    }

    machine return_slice_view_call_index<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut u64 {
        &mut cells.as_mut_slice()[identity_index(write_index(value))]
    }

    machine return_helper_slice_view_call_index<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut u64 {
        &mut return_cells(cells).as_mut_slice()[
            identity_index(write_index(value))
        ]
    }

    machine return_recursive_helper_slice_view_call_index(
        cells: &mut [u64; 2]
    ) -> &mut u64 {
        &mut recursive_cells(cells).as_mut_slice()[make_index()]
    }

    machine return_projected_helper_slice_view_call_index<'bucket, 'value>(
        bucket: &'bucket mut Bucket,
        value: &'value mut u64
    ) -> &'bucket mut u64 {
        &mut return_bucket(bucket).cells.as_mut_slice()[
            identity_index(write_index(value))
        ]
    }

    machine return_deep_projected_helper_slice_view_call_index(
        bucket: &mut Bucket
    ) -> &mut u64 {
        &mut return_bucket(bucket).cells.as_mut_slice()[
            identity_index(identity_index(make_index()))
        ]
    }

    machine return_recursive_projected_helper_slice_view_call_index(
        bucket: &mut Bucket
    ) -> &mut u64 {
        &mut recursive_bucket(bucket).cells.as_mut_slice()[make_index()]
    }

    machine Main::return_attached_projected_slice_view_call_index(
        &mut self
    ) -> &mut u64 {
        &mut self.return_attached_bucket().cells.as_mut_slice()[
            identity_index(write_index(&mut self.value))
        ]
    }

    machine Main::return_deep_attached_projected_slice_view_call_index(
        &mut self
    ) -> &mut u64 {
        &mut self.return_attached_bucket().cells.as_mut_slice()[
            identity_index(identity_index(make_index()))
        ]
    }

    machine Main::return_recursive_attached_projected_slice_view_call_index(
        &mut self
    ) -> &mut u64 {
        &mut self.recursive_attached_bucket().cells.as_mut_slice()[make_index()]
    }

    machine return_slice_view_member_call_index<'bucket, 'value>(
        bucket: &'bucket mut CellBucket,
        value: &'value mut u64
    ) -> &'bucket mut u64 {
        &mut bucket.cells.as_mut_slice()[
            identity_index(write_index(value))
        ].value
    }

    machine return_deep_slice_view_member_call_index(
        bucket: &mut CellBucket
    ) -> &mut u64 {
        &mut bucket.cells.as_mut_slice()[
            identity_index(identity_index(make_index()))
        ].value
    }

    machine return_recursive_slice_view_member_call_index(
        bucket: &mut CellBucket
    ) -> &mut u64 {
        &mut bucket.cells.as_mut_slice()[recursive_index()].value
    }

    machine return_alias_slice_view_call_index<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut u64 {
        let collection: &mut [u64; 2] = cells;
        &mut collection.as_mut_slice()[identity_index(write_index(value))]
    }

    machine return_deep_alias_slice_view_call_index(
        cells: &mut [u64; 2]
    ) -> &mut u64 {
        let collection: &mut [u64; 2] = cells;
        &mut collection.as_mut_slice()[
            identity_index(identity_index(make_index()))
        ]
    }

    machine return_recursive_alias_slice_view_call_index(
        cells: &mut [u64; 2]
    ) -> &mut u64 {
        let collection: &mut [u64; 2] = cells;
        &mut collection.as_mut_slice()[recursive_index()]
    }

    machine return_member_alias_slice_view_call_index<'bucket, 'value>(
        bucket: &'bucket mut Bucket,
        value: &'value mut u64
    ) -> &'bucket mut u64 {
        let collection: &mut [u64; 2] = &mut bucket.cells;
        &mut collection.as_mut_slice()[identity_index(write_index(value))]
    }

    machine return_deep_member_alias_slice_view_call_index(
        bucket: &mut Bucket
    ) -> &mut u64 {
        let collection: &mut [u64; 2] = &mut bucket.cells;
        &mut collection.as_mut_slice()[
            identity_index(identity_index(make_index()))
        ]
    }

    machine return_recursive_member_alias_slice_view_call_index(
        bucket: &mut Bucket
    ) -> &mut u64 {
        let collection: &mut [u64; 2] = &mut bucket.cells;
        &mut collection.as_mut_slice()[recursive_index()]
    }

    machine return_repeated_alias_slice_view_call_index<'matrix, 'first, 'second>(
        matrix: &'matrix mut [[u64; 2]; 2],
        first: &'first mut u64,
        second: &'second mut u64
    ) -> &'matrix mut u64 {
        let collection: &mut [[u64; 2]; 2] = matrix;
        &mut collection.as_mut_slice()[write_index(first)][write_index(second)]
    }

    machine return_deep_repeated_alias_slice_view_call_index(
        matrix: &mut [[u64; 2]; 2]
    ) -> &mut u64 {
        let collection: &mut [[u64; 2]; 2] = matrix;
        &mut collection.as_mut_slice()[
            identity_index(identity_index(make_index()))
        ][make_index()]
    }

    machine return_recursive_repeated_alias_slice_view_call_index(
        matrix: &mut [[u64; 2]; 2]
    ) -> &mut u64 {
        let collection: &mut [[u64; 2]; 2] = matrix;
        &mut collection.as_mut_slice()[recursive_index()][make_index()]
    }

    machine return_member_repeated_alias_slice_view_call_index<
        'bucket, 'first, 'second
    >(
        bucket: &'bucket mut GridBucket,
        first: &'first mut u64,
        second: &'second mut u64
    ) -> &'bucket mut u64 {
        let collection: &mut [[u64; 2]; 2] = &mut bucket.rows;
        &mut collection.as_mut_slice()[write_index(first)][write_index(second)]
    }

    machine return_deep_member_repeated_alias_slice_view_call_index(
        bucket: &mut GridBucket
    ) -> &mut u64 {
        let collection: &mut [[u64; 2]; 2] = &mut bucket.rows;
        &mut collection.as_mut_slice()[
            identity_index(identity_index(make_index()))
        ][make_index()]
    }

    machine return_recursive_member_repeated_alias_slice_view_call_index(
        bucket: &mut GridBucket
    ) -> &mut u64 {
        let collection: &mut [[u64; 2]; 2] = &mut bucket.rows;
        &mut collection.as_mut_slice()[recursive_index()][make_index()]
    }

    machine return_alias_chain_slice_view_call_index<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut u64 {
        let parent: &mut [u64; 2] = cells;
        let collection: &mut [u64; 2] = &mut parent;
        &mut collection.as_mut_slice()[identity_index(write_index(value))]
    }

    machine return_deep_alias_chain_slice_view_call_index(
        cells: &mut [u64; 2]
    ) -> &mut u64 {
        let parent: &mut [u64; 2] = cells;
        let collection: &mut [u64; 2] = &mut parent;
        &mut collection.as_mut_slice()[
            identity_index(identity_index(make_index()))
        ]
    }

    machine return_recursive_alias_chain_slice_view_call_index(
        cells: &mut [u64; 2]
    ) -> &mut u64 {
        let parent: &mut [u64; 2] = cells;
        let collection: &mut [u64; 2] = &mut parent;
        &mut collection.as_mut_slice()[recursive_index()]
    }

    machine return_member_alias_chain_slice_view_call_index<'bucket, 'value>(
        bucket: &'bucket mut Bucket,
        value: &'value mut u64
    ) -> &'bucket mut u64 {
        let parent: &mut Bucket = bucket;
        let collection: &mut [u64; 2] = &mut parent.cells;
        &mut collection.as_mut_slice()[identity_index(write_index(value))]
    }

    machine return_deep_member_alias_chain_slice_view_call_index(
        bucket: &mut Bucket
    ) -> &mut u64 {
        let parent: &mut Bucket = bucket;
        let collection: &mut [u64; 2] = &mut parent.cells;
        &mut collection.as_mut_slice()[
            identity_index(identity_index(make_index()))
        ]
    }

    machine return_recursive_member_alias_chain_slice_view_call_index(
        bucket: &mut Bucket
    ) -> &mut u64 {
        let parent: &mut Bucket = bucket;
        let collection: &mut [u64; 2] = &mut parent.cells;
        &mut collection.as_mut_slice()[recursive_index()]
    }

    machine return_coarse_alias_slice_view_call_index<'matrix, 'first, 'second>(
        matrix: &'matrix mut [[u64; 2]; 2],
        first: &'first mut u64,
        second: &'second mut u64
    ) -> &'matrix mut u64 {
        let row: &mut [u64; 2] = &mut matrix[write_index(first)];
        &mut row.as_mut_slice()[write_index(second)]
    }

    machine return_deep_coarse_alias_slice_view_call_index(
        matrix: &mut [[u64; 2]; 2]
    ) -> &mut u64 {
        let row: &mut [u64; 2] = &mut matrix[
            identity_index(identity_index(make_index()))
        ];
        &mut row.as_mut_slice()[make_index()]
    }

    machine return_recursive_coarse_alias_slice_view_call_index(
        matrix: &mut [[u64; 2]; 2]
    ) -> &mut u64 {
        let row: &mut [u64; 2] = &mut matrix[recursive_index()];
        &mut row.as_mut_slice()[make_index()]
    }

    machine return_coarse_member_slice_view_call_index<'rows, 'first, 'second>(
        rows: &'rows mut [Row; 2],
        first: &'first mut u64,
        second: &'second mut u64
    ) -> &'rows mut u64 {
        let row: &mut Row = &mut rows[write_index(first)];
        &mut row.cells.as_mut_slice()[write_index(second)]
    }

    machine return_deep_coarse_member_slice_view_call_index(
        rows: &mut [Row; 2]
    ) -> &mut u64 {
        let row: &mut Row = &mut rows[
            identity_index(identity_index(make_index()))
        ];
        &mut row.cells.as_mut_slice()[make_index()]
    }

    machine return_recursive_coarse_member_slice_view_call_index(
        rows: &mut [Row; 2]
    ) -> &mut u64 {
        let row: &mut Row = &mut rows[recursive_index()];
        &mut row.cells.as_mut_slice()[make_index()]
    }

    machine return_coarse_helper_member_slice_view_call_index<
        'rows, 'first, 'second
    >(
        rows: &'rows mut [Row; 2],
        first: &'first mut u64,
        second: &'second mut u64
    ) -> &'rows mut u64 {
        let row: &mut Row = &mut rows[write_index(first)];
        &mut return_row(row).cells.as_mut_slice()[write_index(second)]
    }

    machine return_deep_coarse_helper_member_slice_view_call_index(
        rows: &mut [Row; 2]
    ) -> &mut u64 {
        let row: &mut Row = &mut rows[
            identity_index(identity_index(make_index()))
        ];
        &mut return_row(row).cells.as_mut_slice()[make_index()]
    }

    machine return_recursive_coarse_helper_member_slice_view_call_index(
        rows: &mut [Row; 2]
    ) -> &mut u64 {
        let row: &mut Row = &mut rows[recursive_index()];
        &mut return_row(row).cells.as_mut_slice()[make_index()]
    }

    machine return_deep_slice_view_call_index(
        cells: &mut [u64; 2]
    ) -> &mut u64 {
        &mut cells.as_mut_slice()[
            identity_index(identity_index(make_index()))
        ]
    }

    machine return_recursive_slice_view_call_index(
        cells: &mut [u64; 2]
    ) -> &mut u64 {
        &mut cells.as_mut_slice()[recursive_index()]
    }

    machine return_deep_call_index(cells: &mut [u64; 2]) -> &mut u64 {
        &mut cells[identity_index(identity_index(make_index()))]
    }

    machine return_recursive_call_index(cells: &mut [u64; 2]) -> &mut u64 {
        &mut cells[recursive_index()]
    }

    machine return_repeated_call_index<'matrix, 'first, 'second>(
        matrix: &'matrix mut [[u64; 2]; 2],
        first: &'first mut u64,
        second: &'second mut u64
    ) -> &'matrix mut u64 {
        &mut matrix[write_index(first)][write_index(second)]
    }

    machine return_slice_view_repeated_call_index<'matrix, 'first, 'second>(
        matrix: &'matrix mut [[u64; 2]; 2],
        first: &'first mut u64,
        second: &'second mut u64
    ) -> &'matrix mut u64 {
        &mut matrix.as_mut_slice()[write_index(first)][write_index(second)]
    }

    machine return_deep_slice_view_repeated_call_index(
        matrix: &mut [[u64; 2]; 2]
    ) -> &mut u64 {
        &mut matrix.as_mut_slice()[
            identity_index(identity_index(make_index()))
        ][make_index()]
    }

    machine return_deep_repeated_call_index(
        matrix: &mut [[u64; 2]; 2]
    ) -> &mut u64 {
        &mut matrix[identity_index(identity_index(make_index()))][make_index()]
    }

    machine Main::local_index_result(&mut self) {
        let alias: &mut u64 = return_local_index(&mut self.cells);
        alias = 1;
    }

    machine Main::call_index_result(&mut self) {
        let alias: &mut u64 = return_call_index(&mut self.cells);
        alias = 1;
    }

    machine Main::write_call_index_result(&mut self) {
        let alias: &mut u64 =
            return_write_call_index(&mut self.cells, &mut self.value);
        alias = 1;
    }

    machine Main::nested_call_index_result(&mut self) {
        let alias: &mut u64 = return_nested_call_index(&mut self.cells);
        alias = 1;
    }

    machine Main::nested_write_call_index_result(&mut self) {
        let alias: &mut u64 =
            return_nested_write_call_index(&mut self.cells, &mut self.value);
        alias = 1;
    }

    machine Main::slice_view_call_index_result(&mut self) {
        let alias: &mut u64 =
            return_slice_view_call_index(&mut self.cells, &mut self.value);
        alias = 1;
    }

    machine Main::helper_slice_view_call_index_result(&mut self) {
        let alias: &mut u64 = return_helper_slice_view_call_index(
            &mut self.cells,
            &mut self.value
        );
        alias = 1;
    }

    machine Main::recursive_helper_slice_view_call_index_result(&mut self) {
        let alias: &mut u64 =
            return_recursive_helper_slice_view_call_index(&mut self.cells);
        alias = 1;
    }

    machine Main::projected_helper_slice_view_call_index_result(&mut self) {
        let alias: &mut u64 = return_projected_helper_slice_view_call_index(
            &mut self.bucket,
            &mut self.value
        );
        alias = 1;
    }

    machine Main::deep_projected_helper_slice_view_call_index_result(&mut self) {
        let alias: &mut u64 =
            return_deep_projected_helper_slice_view_call_index(&mut self.bucket);
        alias = 1;
    }

    machine Main::recursive_projected_helper_slice_view_call_index_result(
        &mut self
    ) {
        let alias: &mut u64 =
            return_recursive_projected_helper_slice_view_call_index(
                &mut self.bucket
            );
        alias = 1;
    }

    machine Main::attached_projected_slice_view_call_index_result(&mut self) {
        let alias: &mut u64 =
            self.return_attached_projected_slice_view_call_index();
        alias = 1;
    }

    machine Main::deep_attached_projected_slice_view_call_index_result(
        &mut self
    ) {
        let alias: &mut u64 =
            self.return_deep_attached_projected_slice_view_call_index();
        alias = 1;
    }

    machine Main::recursive_attached_projected_slice_view_call_index_result(
        &mut self
    ) {
        let alias: &mut u64 =
            self.return_recursive_attached_projected_slice_view_call_index();
        alias = 1;
    }

    machine Main::slice_view_member_call_index_result(&mut self) {
        let alias: &mut u64 = return_slice_view_member_call_index(
            &mut self.cell_bucket,
            &mut self.value
        );
        alias = 1;
    }

    machine Main::deep_slice_view_member_call_index_result(&mut self) {
        let alias: &mut u64 =
            return_deep_slice_view_member_call_index(&mut self.cell_bucket);
        alias = 1;
    }

    machine Main::recursive_slice_view_member_call_index_result(&mut self) {
        let alias: &mut u64 =
            return_recursive_slice_view_member_call_index(&mut self.cell_bucket);
        alias = 1;
    }

    machine Main::alias_slice_view_call_index_result(&mut self) {
        let alias: &mut u64 = return_alias_slice_view_call_index(
            &mut self.cells,
            &mut self.value
        );
        alias = 1;
    }

    machine Main::deep_alias_slice_view_call_index_result(&mut self) {
        let alias: &mut u64 =
            return_deep_alias_slice_view_call_index(&mut self.cells);
        alias = 1;
    }

    machine Main::recursive_alias_slice_view_call_index_result(&mut self) {
        let alias: &mut u64 =
            return_recursive_alias_slice_view_call_index(&mut self.cells);
        alias = 1;
    }

    machine Main::member_alias_slice_view_call_index_result(&mut self) {
        let alias: &mut u64 = return_member_alias_slice_view_call_index(
            &mut self.bucket,
            &mut self.value
        );
        alias = 1;
    }

    machine Main::deep_member_alias_slice_view_call_index_result(&mut self) {
        let alias: &mut u64 =
            return_deep_member_alias_slice_view_call_index(&mut self.bucket);
        alias = 1;
    }

    machine Main::recursive_member_alias_slice_view_call_index_result(
        &mut self
    ) {
        let alias: &mut u64 =
            return_recursive_member_alias_slice_view_call_index(
                &mut self.bucket
            );
        alias = 1;
    }

    machine Main::repeated_alias_slice_view_call_index_result(&mut self) {
        let alias: &mut u64 = return_repeated_alias_slice_view_call_index(
            &mut self.matrix,
            &mut self.value,
            &mut self.other_value
        );
        alias = 1;
    }

    machine Main::deep_repeated_alias_slice_view_call_index_result(&mut self) {
        let alias: &mut u64 =
            return_deep_repeated_alias_slice_view_call_index(&mut self.matrix);
        alias = 1;
    }

    machine Main::recursive_repeated_alias_slice_view_call_index_result(
        &mut self
    ) {
        let alias: &mut u64 =
            return_recursive_repeated_alias_slice_view_call_index(
                &mut self.matrix
            );
        alias = 1;
    }

    machine Main::member_repeated_alias_slice_view_call_index_result(&mut self) {
        let alias: &mut u64 = return_member_repeated_alias_slice_view_call_index(
            &mut self.grid_bucket,
            &mut self.value,
            &mut self.other_value
        );
        alias = 1;
    }

    machine Main::deep_member_repeated_alias_slice_view_call_index_result(
        &mut self
    ) {
        let alias: &mut u64 =
            return_deep_member_repeated_alias_slice_view_call_index(
                &mut self.grid_bucket
            );
        alias = 1;
    }

    machine Main::recursive_member_repeated_alias_slice_view_call_index_result(
        &mut self
    ) {
        let alias: &mut u64 =
            return_recursive_member_repeated_alias_slice_view_call_index(
                &mut self.grid_bucket
            );
        alias = 1;
    }

    machine Main::alias_chain_slice_view_call_index_result(&mut self) {
        let alias: &mut u64 = return_alias_chain_slice_view_call_index(
            &mut self.cells,
            &mut self.value
        );
        alias = 1;
    }

    machine Main::deep_alias_chain_slice_view_call_index_result(&mut self) {
        let alias: &mut u64 =
            return_deep_alias_chain_slice_view_call_index(&mut self.cells);
        alias = 1;
    }

    machine Main::recursive_alias_chain_slice_view_call_index_result(&mut self) {
        let alias: &mut u64 =
            return_recursive_alias_chain_slice_view_call_index(&mut self.cells);
        alias = 1;
    }

    machine Main::member_alias_chain_slice_view_call_index_result(&mut self) {
        let alias: &mut u64 = return_member_alias_chain_slice_view_call_index(
            &mut self.bucket,
            &mut self.value
        );
        alias = 1;
    }

    machine Main::deep_member_alias_chain_slice_view_call_index_result(
        &mut self
    ) {
        let alias: &mut u64 =
            return_deep_member_alias_chain_slice_view_call_index(
                &mut self.bucket
            );
        alias = 1;
    }

    machine Main::recursive_member_alias_chain_slice_view_call_index_result(
        &mut self
    ) {
        let alias: &mut u64 =
            return_recursive_member_alias_chain_slice_view_call_index(
                &mut self.bucket
            );
        alias = 1;
    }

    machine Main::coarse_alias_slice_view_call_index_result(&mut self) {
        let alias: &mut u64 = return_coarse_alias_slice_view_call_index(
            &mut self.matrix,
            &mut self.value,
            &mut self.other_value
        );
        alias = 1;
    }

    machine Main::deep_coarse_alias_slice_view_call_index_result(&mut self) {
        let alias: &mut u64 =
            return_deep_coarse_alias_slice_view_call_index(&mut self.matrix);
        alias = 1;
    }

    machine Main::recursive_coarse_alias_slice_view_call_index_result(
        &mut self
    ) {
        let alias: &mut u64 =
            return_recursive_coarse_alias_slice_view_call_index(
                &mut self.matrix
            );
        alias = 1;
    }

    machine Main::coarse_member_slice_view_call_index_result(&mut self) {
        let alias: &mut u64 = return_coarse_member_slice_view_call_index(
            &mut self.row_items,
            &mut self.value,
            &mut self.other_value
        );
        alias = 1;
    }

    machine Main::deep_coarse_member_slice_view_call_index_result(&mut self) {
        let alias: &mut u64 =
            return_deep_coarse_member_slice_view_call_index(&mut self.row_items);
        alias = 1;
    }

    machine Main::recursive_coarse_member_slice_view_call_index_result(
        &mut self
    ) {
        let alias: &mut u64 =
            return_recursive_coarse_member_slice_view_call_index(
                &mut self.row_items
            );
        alias = 1;
    }

    machine Main::coarse_helper_member_slice_view_call_index_result(&mut self) {
        let alias: &mut u64 = return_coarse_helper_member_slice_view_call_index(
            &mut self.row_items,
            &mut self.value,
            &mut self.other_value
        );
        alias = 1;
    }

    machine Main::deep_coarse_helper_member_slice_view_call_index_result(
        &mut self
    ) {
        let alias: &mut u64 =
            return_deep_coarse_helper_member_slice_view_call_index(
                &mut self.row_items
            );
        alias = 1;
    }

    machine Main::recursive_coarse_helper_member_slice_view_call_index_result(
        &mut self
    ) {
        let alias: &mut u64 =
            return_recursive_coarse_helper_member_slice_view_call_index(
                &mut self.row_items
            );
        alias = 1;
    }

    machine Main::deep_slice_view_call_index_result(&mut self) {
        let alias: &mut u64 =
            return_deep_slice_view_call_index(&mut self.cells);
        alias = 1;
    }

    machine Main::recursive_slice_view_call_index_result(&mut self) {
        let alias: &mut u64 =
            return_recursive_slice_view_call_index(&mut self.cells);
        alias = 1;
    }

    machine Main::deep_call_index_result(&mut self) {
        let alias: &mut u64 = return_deep_call_index(&mut self.cells);
        alias = 1;
    }

    machine Main::recursive_call_index_result(&mut self) {
        let alias: &mut u64 = return_recursive_call_index(&mut self.cells);
        alias = 1;
    }

    machine Main::repeated_call_index_result(&mut self) {
        let alias: &mut u64 = return_repeated_call_index(
            &mut self.matrix,
            &mut self.value,
            &mut self.other_value
        );
        alias = 1;
    }

    machine Main::slice_view_repeated_call_index_result(&mut self) {
        let alias: &mut u64 = return_slice_view_repeated_call_index(
            &mut self.matrix,
            &mut self.value,
            &mut self.other_value
        );
        alias = 1;
    }

    machine Main::deep_slice_view_repeated_call_index_result(&mut self) {
        let alias: &mut u64 =
            return_deep_slice_view_repeated_call_index(&mut self.matrix);
        alias = 1;
    }

    machine Main::deep_repeated_call_index_result(&mut self) {
        let alias: &mut u64 = return_deep_repeated_call_index(&mut self.matrix);
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

    let local = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::local_index_result")
        .expect("local-index helper caller");
    let local_entry = typed
        .machine_states(local)
        .first()
        .expect("local-index helper caller entry state");
    assert_eq!(
        resolver
            .inferred_state_write_frame(local, local_entry)
            .complete_paths(),
        Some(["self.cells".to_owned()].as_slice()),
        "an effect-free local index preserves the returned collection origin"
    );

    for (name, expected_paths) in [
        ("Main::call_index_result", vec!["self.cells"]),
        (
            "Main::write_call_index_result",
            vec!["self.cells", "self.value"],
        ),
        ("Main::nested_call_index_result", vec!["self.cells"]),
        (
            "Main::nested_write_call_index_result",
            vec!["self.cells", "self.value"],
        ),
        (
            "Main::slice_view_call_index_result",
            vec!["self.cells", "self.value"],
        ),
        (
            "Main::helper_slice_view_call_index_result",
            vec!["self.cells", "self.value"],
        ),
        (
            "Main::projected_helper_slice_view_call_index_result",
            vec!["self.bucket.cells", "self.value"],
        ),
        (
            "Main::attached_projected_slice_view_call_index_result",
            vec!["self.bucket.cells", "self.value"],
        ),
        (
            "Main::slice_view_member_call_index_result",
            vec!["self.cell_bucket.cells", "self.value"],
        ),
        (
            "Main::alias_slice_view_call_index_result",
            vec!["self.cells", "self.value"],
        ),
        (
            "Main::member_alias_slice_view_call_index_result",
            vec!["self.bucket.cells", "self.value"],
        ),
        (
            "Main::repeated_alias_slice_view_call_index_result",
            vec!["self.matrix", "self.other_value", "self.value"],
        ),
        (
            "Main::member_repeated_alias_slice_view_call_index_result",
            vec!["self.grid_bucket.rows", "self.other_value", "self.value"],
        ),
        (
            "Main::alias_chain_slice_view_call_index_result",
            vec!["self.cells", "self.value"],
        ),
        (
            "Main::member_alias_chain_slice_view_call_index_result",
            vec!["self.bucket.cells", "self.value"],
        ),
        (
            "Main::coarse_alias_slice_view_call_index_result",
            vec!["self.matrix", "self.other_value", "self.value"],
        ),
        (
            "Main::coarse_member_slice_view_call_index_result",
            vec!["self.other_value", "self.row_items", "self.value"],
        ),
        (
            "Main::coarse_helper_member_slice_view_call_index_result",
            vec!["self.other_value", "self.row_items", "self.value"],
        ),
        (
            "Main::repeated_call_index_result",
            vec!["self.matrix", "self.other_value", "self.value"],
        ),
        (
            "Main::slice_view_repeated_call_index_result",
            vec!["self.matrix", "self.other_value", "self.value"],
        ),
        ("Main::deep_call_index_result", vec!["self.cells"]),
        ("Main::deep_repeated_call_index_result", vec!["self.matrix"]),
        (
            "Main::deep_slice_view_call_index_result",
            vec!["self.cells"],
        ),
        (
            "Main::deep_slice_view_repeated_call_index_result",
            vec!["self.matrix"],
        ),
        (
            "Main::deep_projected_helper_slice_view_call_index_result",
            vec!["self.bucket.cells"],
        ),
        (
            "Main::deep_attached_projected_slice_view_call_index_result",
            vec!["self.bucket.cells"],
        ),
        (
            "Main::deep_slice_view_member_call_index_result",
            vec!["self.cell_bucket.cells"],
        ),
        (
            "Main::deep_alias_slice_view_call_index_result",
            vec!["self.cells"],
        ),
        (
            "Main::deep_member_alias_slice_view_call_index_result",
            vec!["self.bucket.cells"],
        ),
        (
            "Main::deep_repeated_alias_slice_view_call_index_result",
            vec!["self.matrix"],
        ),
        (
            "Main::deep_member_repeated_alias_slice_view_call_index_result",
            vec!["self.grid_bucket.rows"],
        ),
        (
            "Main::deep_alias_chain_slice_view_call_index_result",
            vec!["self.cells"],
        ),
        (
            "Main::deep_member_alias_chain_slice_view_call_index_result",
            vec!["self.bucket.cells"],
        ),
        (
            "Main::deep_coarse_alias_slice_view_call_index_result",
            vec!["self.matrix"],
        ),
        (
            "Main::deep_coarse_member_slice_view_call_index_result",
            vec!["self.row_items"],
        ),
        (
            "Main::deep_coarse_helper_member_slice_view_call_index_result",
            vec!["self.row_items"],
        ),
    ] {
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap_or_else(|| panic!("{name} helper caller"));
        let entry = typed
            .machine_states(machine)
            .first()
            .unwrap_or_else(|| panic!("{name} helper caller entry state"));
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
            "{name} must publish the index call's writes and preserve the coarse collection origin"
        );
    }

    for name in [
        "Main::recursive_call_index_result",
        "Main::recursive_helper_slice_view_call_index_result",
        "Main::recursive_projected_helper_slice_view_call_index_result",
        "Main::recursive_attached_projected_slice_view_call_index_result",
        "Main::recursive_slice_view_member_call_index_result",
        "Main::recursive_alias_slice_view_call_index_result",
        "Main::recursive_member_alias_slice_view_call_index_result",
        "Main::recursive_repeated_alias_slice_view_call_index_result",
        "Main::recursive_member_repeated_alias_slice_view_call_index_result",
        "Main::recursive_alias_chain_slice_view_call_index_result",
        "Main::recursive_member_alias_chain_slice_view_call_index_result",
        "Main::recursive_coarse_alias_slice_view_call_index_result",
        "Main::recursive_coarse_member_slice_view_call_index_result",
        "Main::recursive_coarse_helper_member_slice_view_call_index_result",
        "Main::recursive_slice_view_call_index_result",
    ] {
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap_or_else(|| panic!("{name} helper caller"));
        let entry = typed
            .machine_states(machine)
            .first()
            .unwrap_or_else(|| panic!("{name} helper caller entry state"));
        assert!(
            !resolver
                .inferred_state_write_frame(machine, entry)
                .is_complete(),
            "{name} must remain opaque without a complete non-rebinding index frame"
        );
    }
}

#[test]
fn stable_alias_index_frame_accepts_a_finite_exact_call_tree() {
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

    data Row {
        cells: [u64; 2];
    }

    data Main {
        value: u64;
        other_value: u64;
        cells: [u64; 2];
        other_cells: [u64; 2];
        matrix: [[u64; 2]; 2];
        other_matrix: [[u64; 2]; 2];
        bucket: Bucket;
        other_bucket: Bucket;
        cell_bucket: CellBucket;
        other_cell_bucket: CellBucket;
        grid_bucket: GridBucket;
        other_grid_bucket: GridBucket;
        row_items: [Row; 2];
        other_row_items: [Row; 2];
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

    machine return_row(row: &mut Row) -> &mut Row {
        row
    }

    machine return_bucket(bucket: &mut Bucket) -> &mut Bucket {
        bucket
    }

    machine recursive_bucket(bucket: &mut Bucket) -> &mut Bucket {
        recursive_bucket(bucket)
    }

    machine Main::return_attached_bucket(&mut self) -> &mut Bucket {
        &mut self.bucket
    }

    machine Main::recursive_attached_bucket(&mut self) -> &mut Bucket {
        self.recursive_attached_bucket()
    }

    machine Main::local_index_alias(&mut self) {
        let index: u64 = 0;
        let alias: &mut u64 = &mut self.cells[index];
        alias = 1;
    }

    machine Main::call_index_alias(&mut self) {
        let alias: &mut u64 = &mut self.cells[make_index()];
        alias = 1;
    }

    machine Main::write_call_index_alias(&mut self) {
        let alias: &mut u64 = &mut self.cells[write_index(&mut self.value)];
        alias = 1;
    }

    machine Main::nested_call_index_alias(&mut self) {
        let alias: &mut u64 = &mut self.cells[identity_index(make_index())];
        alias = 1;
    }

    machine Main::nested_write_call_index_alias(&mut self) {
        let alias: &mut u64 =
            &mut self.cells[identity_index(write_index(&mut self.value))];
        alias = 1;
    }

    machine Main::slice_view_call_index_alias(&mut self) {
        let alias: &mut u64 = &mut self.cells.as_mut_slice()[
            identity_index(write_index(&mut self.value))
        ];
        alias = 1;
    }

    machine Main::helper_slice_view_call_index_alias(&mut self) {
        let alias: &mut u64 = &mut return_cells(&mut self.cells).as_mut_slice()[
            identity_index(write_index(&mut self.value))
        ];
        alias = 1;
    }

    machine Main::recursive_helper_slice_view_call_index_alias(&mut self) {
        let alias: &mut u64 =
            &mut recursive_cells(&mut self.cells).as_mut_slice()[make_index()];
        alias = 1;
    }

    machine Main::projected_helper_slice_view_call_index_alias(&mut self) {
        let alias: &mut u64 = &mut return_bucket(&mut self.bucket)
            .cells
            .as_mut_slice()[identity_index(write_index(&mut self.value))];
        alias = 1;
    }

    machine Main::deep_projected_helper_slice_view_call_index_alias(&mut self) {
        let alias: &mut u64 = &mut return_bucket(&mut self.bucket)
            .cells
            .as_mut_slice()[identity_index(identity_index(make_index()))];
        alias = 1;
    }

    machine Main::recursive_projected_helper_slice_view_call_index_alias(
        &mut self
    ) {
        let alias: &mut u64 = &mut recursive_bucket(&mut self.bucket)
            .cells
            .as_mut_slice()[make_index()];
        alias = 1;
    }

    machine Main::attached_projected_slice_view_call_index_alias(&mut self) {
        let alias: &mut u64 = &mut self.return_attached_bucket()
            .cells
            .as_mut_slice()[identity_index(write_index(&mut self.value))];
        alias = 1;
    }

    machine Main::deep_attached_projected_slice_view_call_index_alias(
        &mut self
    ) {
        let alias: &mut u64 = &mut self.return_attached_bucket()
            .cells
            .as_mut_slice()[identity_index(identity_index(make_index()))];
        alias = 1;
    }

    machine Main::recursive_attached_projected_slice_view_call_index_alias(
        &mut self
    ) {
        let alias: &mut u64 = &mut self.recursive_attached_bucket()
            .cells
            .as_mut_slice()[make_index()];
        alias = 1;
    }

    machine Main::slice_view_member_call_index_alias(&mut self) {
        let alias: &mut u64 = &mut self.cell_bucket.cells.as_mut_slice()[
            identity_index(write_index(&mut self.value))
        ].value;
        alias = 1;
    }

    machine Main::deep_slice_view_member_call_index_alias(&mut self) {
        let alias: &mut u64 = &mut self.cell_bucket.cells.as_mut_slice()[
            identity_index(identity_index(make_index()))
        ].value;
        alias = 1;
    }

    machine Main::recursive_slice_view_member_call_index_alias(&mut self) {
        let alias: &mut u64 = &mut self.cell_bucket.cells.as_mut_slice()[
            recursive_index()
        ].value;
        alias = 1;
    }

    machine Main::collection_alias_slice_view_call_index_alias(&mut self) {
        let collection: &mut [u64; 2] = &mut self.cells;
        let alias: &mut u64 = &mut collection.as_mut_slice()[
            identity_index(write_index(&mut self.value))
        ];
        alias = 1;
    }

    machine Main::deep_collection_alias_slice_view_call_index_alias(&mut self) {
        let collection: &mut [u64; 2] = &mut self.cells;
        let alias: &mut u64 = &mut collection.as_mut_slice()[
            identity_index(identity_index(make_index()))
        ];
        alias = 1;
    }

    machine Main::recursive_collection_alias_slice_view_call_index_alias(
        &mut self
    ) {
        let collection: &mut [u64; 2] = &mut self.cells;
        let alias: &mut u64 =
            &mut collection.as_mut_slice()[recursive_index()];
        alias = 1;
    }

    machine Main::member_collection_alias_slice_view_call_index_alias(
        &mut self
    ) {
        let collection: &mut [u64; 2] = &mut self.bucket.cells;
        let alias: &mut u64 = &mut collection.as_mut_slice()[
            identity_index(write_index(&mut self.value))
        ];
        alias = 1;
    }

    machine Main::deep_member_collection_alias_slice_view_call_index_alias(
        &mut self
    ) {
        let collection: &mut [u64; 2] = &mut self.bucket.cells;
        let alias: &mut u64 = &mut collection.as_mut_slice()[
            identity_index(identity_index(make_index()))
        ];
        alias = 1;
    }

    machine Main::recursive_member_collection_alias_slice_view_call_index_alias(
        &mut self
    ) {
        let collection: &mut [u64; 2] = &mut self.bucket.cells;
        let alias: &mut u64 =
            &mut collection.as_mut_slice()[recursive_index()];
        alias = 1;
    }

    machine Main::repeated_collection_alias_slice_view_call_index_alias(
        &mut self
    ) {
        let collection: &mut [[u64; 2]; 2] = &mut self.matrix;
        let alias: &mut u64 = &mut collection.as_mut_slice()[
            write_index(&mut self.value)
        ][write_index(&mut self.other_value)];
        alias = 1;
    }

    machine Main::deep_repeated_collection_alias_slice_view_call_index_alias(
        &mut self
    ) {
        let collection: &mut [[u64; 2]; 2] = &mut self.matrix;
        let alias: &mut u64 = &mut collection.as_mut_slice()[
            identity_index(identity_index(make_index()))
        ][make_index()];
        alias = 1;
    }

    machine Main::recursive_repeated_collection_alias_slice_view_call_index_alias(
        &mut self
    ) {
        let collection: &mut [[u64; 2]; 2] = &mut self.matrix;
        let alias: &mut u64 = &mut collection.as_mut_slice()[
            recursive_index()
        ][make_index()];
        alias = 1;
    }

    machine Main::member_repeated_collection_alias_slice_view_call_index_alias(
        &mut self
    ) {
        let collection: &mut [[u64; 2]; 2] = &mut self.grid_bucket.rows;
        let alias: &mut u64 = &mut collection.as_mut_slice()[
            write_index(&mut self.value)
        ][write_index(&mut self.other_value)];
        alias = 1;
    }

    machine Main::deep_member_repeated_collection_alias_slice_view_call_index_alias(
        &mut self
    ) {
        let collection: &mut [[u64; 2]; 2] = &mut self.grid_bucket.rows;
        let alias: &mut u64 = &mut collection.as_mut_slice()[
            identity_index(identity_index(make_index()))
        ][make_index()];
        alias = 1;
    }

    machine Main::recursive_member_repeated_collection_alias_slice_view_call_index_alias(
        &mut self
    ) {
        let collection: &mut [[u64; 2]; 2] = &mut self.grid_bucket.rows;
        let alias: &mut u64 = &mut collection.as_mut_slice()[
            recursive_index()
        ][make_index()];
        alias = 1;
    }

    machine Main::alias_chain_slice_view_call_index_alias(&mut self) {
        let parent: &mut [u64; 2] = &mut self.cells;
        let collection: &mut [u64; 2] = &mut parent;
        let alias: &mut u64 = &mut collection.as_mut_slice()[
            identity_index(write_index(&mut self.value))
        ];
        alias = 1;
    }

    machine Main::deep_alias_chain_slice_view_call_index_alias(&mut self) {
        let parent: &mut [u64; 2] = &mut self.cells;
        let collection: &mut [u64; 2] = &mut parent;
        let alias: &mut u64 = &mut collection.as_mut_slice()[
            identity_index(identity_index(make_index()))
        ];
        alias = 1;
    }

    machine Main::recursive_alias_chain_slice_view_call_index_alias(&mut self) {
        let parent: &mut [u64; 2] = &mut self.cells;
        let collection: &mut [u64; 2] = &mut parent;
        let alias: &mut u64 =
            &mut collection.as_mut_slice()[recursive_index()];
        alias = 1;
    }

    machine Main::member_alias_chain_slice_view_call_index_alias(&mut self) {
        let parent: &mut Bucket = &mut self.bucket;
        let collection: &mut [u64; 2] = &mut parent.cells;
        let alias: &mut u64 = &mut collection.as_mut_slice()[
            identity_index(write_index(&mut self.value))
        ];
        alias = 1;
    }

    machine Main::deep_member_alias_chain_slice_view_call_index_alias(
        &mut self
    ) {
        let parent: &mut Bucket = &mut self.bucket;
        let collection: &mut [u64; 2] = &mut parent.cells;
        let alias: &mut u64 = &mut collection.as_mut_slice()[
            identity_index(identity_index(make_index()))
        ];
        alias = 1;
    }

    machine Main::recursive_member_alias_chain_slice_view_call_index_alias(
        &mut self
    ) {
        let parent: &mut Bucket = &mut self.bucket;
        let collection: &mut [u64; 2] = &mut parent.cells;
        let alias: &mut u64 =
            &mut collection.as_mut_slice()[recursive_index()];
        alias = 1;
    }

    machine Main::coarse_alias_slice_view_call_index_alias(&mut self) {
        let row: &mut [u64; 2] =
            &mut self.matrix[write_index(&mut self.value)];
        let alias: &mut u64 =
            &mut row.as_mut_slice()[write_index(&mut self.other_value)];
        alias = 1;
    }

    machine Main::deep_coarse_alias_slice_view_call_index_alias(&mut self) {
        let row: &mut [u64; 2] = &mut self.matrix[
            identity_index(identity_index(make_index()))
        ];
        let alias: &mut u64 = &mut row.as_mut_slice()[make_index()];
        alias = 1;
    }

    machine Main::recursive_coarse_alias_slice_view_call_index_alias(&mut self) {
        let row: &mut [u64; 2] = &mut self.matrix[recursive_index()];
        let alias: &mut u64 = &mut row.as_mut_slice()[make_index()];
        alias = 1;
    }

    machine Main::coarse_member_slice_view_call_index_alias(&mut self) {
        let row: &mut Row =
            &mut self.row_items[write_index(&mut self.value)];
        let alias: &mut u64 =
            &mut row.cells.as_mut_slice()[write_index(&mut self.other_value)];
        alias = 1;
    }

    machine Main::deep_coarse_member_slice_view_call_index_alias(&mut self) {
        let row: &mut Row = &mut self.row_items[
            identity_index(identity_index(make_index()))
        ];
        let alias: &mut u64 = &mut row.cells.as_mut_slice()[make_index()];
        alias = 1;
    }

    machine Main::recursive_coarse_member_slice_view_call_index_alias(
        &mut self
    ) {
        let row: &mut Row = &mut self.row_items[recursive_index()];
        let alias: &mut u64 = &mut row.cells.as_mut_slice()[make_index()];
        alias = 1;
    }

    machine Main::coarse_helper_member_slice_view_call_index_alias(&mut self) {
        let row: &mut Row =
            &mut self.row_items[write_index(&mut self.value)];
        let alias: &mut u64 = &mut return_row(row)
            .cells
            .as_mut_slice()[write_index(&mut self.other_value)];
        alias = 1;
    }

    machine Main::deep_coarse_helper_member_slice_view_call_index_alias(
        &mut self
    ) {
        let row: &mut Row = &mut self.row_items[
            identity_index(identity_index(make_index()))
        ];
        let alias: &mut u64 =
            &mut return_row(row).cells.as_mut_slice()[make_index()];
        alias = 1;
    }

    machine Main::recursive_coarse_helper_member_slice_view_call_index_alias(
        &mut self
    ) {
        let row: &mut Row = &mut self.row_items[recursive_index()];
        let alias: &mut u64 =
            &mut return_row(row).cells.as_mut_slice()[make_index()];
        alias = 1;
    }

    machine Main::deep_slice_view_call_index_alias(&mut self) {
        let alias: &mut u64 = &mut self.cells.as_mut_slice()[
            identity_index(identity_index(make_index()))
        ];
        alias = 1;
    }

    machine Main::recursive_slice_view_call_index_alias(&mut self) {
        let alias: &mut u64 =
            &mut self.cells.as_mut_slice()[recursive_index()];
        alias = 1;
    }

    machine Main::deep_call_index_alias(&mut self) {
        let alias: &mut u64 =
            &mut self.cells[identity_index(identity_index(make_index()))];
        alias = 1;
    }

    machine Main::recursive_call_index_alias(&mut self) {
        let alias: &mut u64 = &mut self.cells[recursive_index()];
        alias = 1;
    }

    machine Main::repeated_call_index_alias(&mut self) {
        let alias: &mut u64 =
            &mut self.matrix[write_index(&mut self.value)][
                write_index(&mut self.other_value)
            ];
        alias = 1;
    }

    machine Main::slice_view_repeated_call_index_alias(&mut self) {
        let alias: &mut u64 =
            &mut self.matrix.as_mut_slice()[write_index(&mut self.value)][
                write_index(&mut self.other_value)
            ];
        alias = 1;
    }

    machine Main::deep_slice_view_repeated_call_index_alias(&mut self) {
        let alias: &mut u64 = &mut self.matrix.as_mut_slice()[
            identity_index(identity_index(make_index()))
        ][make_index()];
        alias = 1;
    }

    machine Main::call_index_alias_rebind(&mut self) {
        let alias: &mut u64 = &mut self.cells[0];
        alias = &mut self.other_cells[make_index()];
        alias = 1;
    }

    machine Main::write_call_index_alias_rebind(&mut self) {
        let alias: &mut u64 = &mut self.cells[0];
        alias = &mut self.other_cells[write_index(&mut self.value)];
        alias = 1;
    }

    machine Main::prior_alias_survives_call_index_rebind(&mut self) {
        let alias: &mut u64 = &mut self.cells[0];
        let prior: &mut u64 = &mut alias;
        alias = &mut self.other_cells[make_index()];
        prior = 1;
        alias = 2;
    }

    machine Main::prior_alias_survives_slice_view_call_index_rebind(&mut self) {
        let alias: &mut u64 = &mut self.cells[0];
        let prior: &mut u64 = &mut alias;
        alias = &mut self.other_cells.as_mut_slice()[
            identity_index(write_index(&mut self.value))
        ];
        prior = 1;
        alias = 2;
    }

    machine Main::prior_alias_survives_helper_slice_view_rebind(&mut self) {
        let alias: &mut u64 = &mut self.cells[0];
        let prior: &mut u64 = &mut alias;
        alias = &mut return_cells(&mut self.other_cells).as_mut_slice()[
            identity_index(write_index(&mut self.value))
        ];
        prior = 1;
        alias = 2;
    }

    machine Main::prior_alias_survives_attached_projected_slice_view_rebind(
        &mut self
    ) {
        let alias: &mut u64 = &mut self.cells[0];
        let prior: &mut u64 = &mut alias;
        alias = &mut self.return_attached_bucket()
            .cells
            .as_mut_slice()[identity_index(write_index(&mut self.value))];
        prior = 1;
        alias = 2;
    }

    machine Main::prior_alias_survives_slice_view_member_rebind(&mut self) {
        let alias: &mut u64 = &mut self.cells[0];
        let prior: &mut u64 = &mut alias;
        alias = &mut self.other_cell_bucket.cells.as_mut_slice()[
            identity_index(write_index(&mut self.value))
        ].value;
        prior = 1;
        alias = 2;
    }

    machine Main::prior_alias_survives_member_repeated_collection_view_rebind(
        &mut self
    ) {
        let collection: &mut [[u64; 2]; 2] = &mut self.other_grid_bucket.rows;
        let alias: &mut u64 = &mut self.cells[0];
        let prior: &mut u64 = &mut alias;
        alias = &mut collection.as_mut_slice()[write_index(&mut self.value)][
            write_index(&mut self.other_value)
        ];
        prior = 1;
        alias = 2;
    }

    machine Main::helper_slice_view_call_index_alias_rebind(&mut self) {
        let alias: &mut u64 = &mut self.cells[0];
        alias = &mut return_cells(&mut self.other_cells).as_mut_slice()[
            identity_index(write_index(&mut self.value))
        ];
        alias = 1;
    }

    machine Main::recursive_helper_slice_view_call_index_alias_rebind(&mut self) {
        let alias: &mut u64 = &mut self.cells[0];
        alias = &mut recursive_cells(&mut self.other_cells).as_mut_slice()[
            make_index()
        ];
        alias = 1;
    }

    machine Main::projected_helper_slice_view_call_index_alias_rebind(&mut self) {
        let alias: &mut u64 = &mut self.cells[0];
        alias = &mut return_bucket(&mut self.other_bucket)
            .cells
            .as_mut_slice()[identity_index(write_index(&mut self.value))];
        alias = 1;
    }

    machine Main::deep_projected_helper_slice_view_call_index_alias_rebind(
        &mut self
    ) {
        let alias: &mut u64 = &mut self.cells[0];
        alias = &mut return_bucket(&mut self.other_bucket)
            .cells
            .as_mut_slice()[identity_index(identity_index(make_index()))];
        alias = 1;
    }

    machine Main::recursive_projected_helper_slice_view_call_index_alias_rebind(
        &mut self
    ) {
        let alias: &mut u64 = &mut self.cells[0];
        alias = &mut recursive_bucket(&mut self.other_bucket)
            .cells
            .as_mut_slice()[make_index()];
        alias = 1;
    }

    machine Main::attached_projected_slice_view_call_index_alias_rebind(
        &mut self
    ) {
        let alias: &mut u64 = &mut self.cells[0];
        alias = &mut self.return_attached_bucket()
            .cells
            .as_mut_slice()[identity_index(write_index(&mut self.value))];
        alias = 1;
    }

    machine Main::deep_attached_projected_slice_view_call_index_alias_rebind(
        &mut self
    ) {
        let alias: &mut u64 = &mut self.cells[0];
        alias = &mut self.return_attached_bucket()
            .cells
            .as_mut_slice()[identity_index(identity_index(make_index()))];
        alias = 1;
    }

    machine Main::recursive_attached_projected_slice_view_call_index_alias_rebind(
        &mut self
    ) {
        let alias: &mut u64 = &mut self.cells[0];
        alias = &mut self.recursive_attached_bucket()
            .cells
            .as_mut_slice()[make_index()];
        alias = 1;
    }

    machine Main::slice_view_member_call_index_alias_rebind(&mut self) {
        let alias: &mut u64 = &mut self.cells[0];
        alias = &mut self.other_cell_bucket.cells.as_mut_slice()[
            identity_index(write_index(&mut self.value))
        ].value;
        alias = 1;
    }

    machine Main::deep_slice_view_member_call_index_alias_rebind(&mut self) {
        let alias: &mut u64 = &mut self.cells[0];
        alias = &mut self.other_cell_bucket.cells.as_mut_slice()[
            identity_index(identity_index(make_index()))
        ].value;
        alias = 1;
    }

    machine Main::recursive_slice_view_member_call_index_alias_rebind(&mut self) {
        let alias: &mut u64 = &mut self.cells[0];
        alias = &mut self.other_cell_bucket.cells.as_mut_slice()[
            recursive_index()
        ].value;
        alias = 1;
    }

    machine Main::collection_alias_slice_view_call_index_alias_rebind(&mut self) {
        let collection: &mut [u64; 2] = &mut self.other_cells;
        let alias: &mut u64 = &mut self.cells[0];
        alias = &mut collection.as_mut_slice()[
            identity_index(write_index(&mut self.value))
        ];
        alias = 1;
    }

    machine Main::deep_collection_alias_slice_view_call_index_alias_rebind(
        &mut self
    ) {
        let collection: &mut [u64; 2] = &mut self.other_cells;
        let alias: &mut u64 = &mut self.cells[0];
        alias = &mut collection.as_mut_slice()[
            identity_index(identity_index(make_index()))
        ];
        alias = 1;
    }

    machine Main::recursive_collection_alias_slice_view_call_index_alias_rebind(
        &mut self
    ) {
        let collection: &mut [u64; 2] = &mut self.other_cells;
        let alias: &mut u64 = &mut self.cells[0];
        alias = &mut collection.as_mut_slice()[recursive_index()];
        alias = 1;
    }

    machine Main::member_collection_alias_slice_view_call_index_alias_rebind(
        &mut self
    ) {
        let collection: &mut [u64; 2] = &mut self.other_bucket.cells;
        let alias: &mut u64 = &mut self.cells[0];
        alias = &mut collection.as_mut_slice()[
            identity_index(write_index(&mut self.value))
        ];
        alias = 1;
    }

    machine Main::deep_member_collection_alias_slice_view_call_index_alias_rebind(
        &mut self
    ) {
        let collection: &mut [u64; 2] = &mut self.other_bucket.cells;
        let alias: &mut u64 = &mut self.cells[0];
        alias = &mut collection.as_mut_slice()[
            identity_index(identity_index(make_index()))
        ];
        alias = 1;
    }

    machine Main::recursive_member_collection_alias_slice_view_call_index_alias_rebind(
        &mut self
    ) {
        let collection: &mut [u64; 2] = &mut self.other_bucket.cells;
        let alias: &mut u64 = &mut self.cells[0];
        alias = &mut collection.as_mut_slice()[recursive_index()];
        alias = 1;
    }

    machine Main::repeated_collection_alias_slice_view_call_index_alias_rebind(
        &mut self
    ) {
        let collection: &mut [[u64; 2]; 2] = &mut self.other_matrix;
        let alias: &mut u64 = &mut self.cells[0];
        alias = &mut collection.as_mut_slice()[write_index(&mut self.value)][
            write_index(&mut self.other_value)
        ];
        alias = 1;
    }

    machine Main::deep_repeated_collection_alias_slice_view_call_index_alias_rebind(
        &mut self
    ) {
        let collection: &mut [[u64; 2]; 2] = &mut self.other_matrix;
        let alias: &mut u64 = &mut self.cells[0];
        alias = &mut collection.as_mut_slice()[
            identity_index(identity_index(make_index()))
        ][make_index()];
        alias = 1;
    }

    machine Main::recursive_repeated_collection_alias_slice_view_call_index_alias_rebind(
        &mut self
    ) {
        let collection: &mut [[u64; 2]; 2] = &mut self.other_matrix;
        let alias: &mut u64 = &mut self.cells[0];
        alias = &mut collection.as_mut_slice()[recursive_index()][make_index()];
        alias = 1;
    }

    machine Main::member_repeated_collection_alias_slice_view_call_index_alias_rebind(
        &mut self
    ) {
        let collection: &mut [[u64; 2]; 2] = &mut self.other_grid_bucket.rows;
        let alias: &mut u64 = &mut self.cells[0];
        alias = &mut collection.as_mut_slice()[write_index(&mut self.value)][
            write_index(&mut self.other_value)
        ];
        alias = 1;
    }

    machine Main::deep_member_repeated_collection_alias_slice_view_call_index_alias_rebind(
        &mut self
    ) {
        let collection: &mut [[u64; 2]; 2] = &mut self.other_grid_bucket.rows;
        let alias: &mut u64 = &mut self.cells[0];
        alias = &mut collection.as_mut_slice()[
            identity_index(identity_index(make_index()))
        ][make_index()];
        alias = 1;
    }

    machine Main::recursive_member_repeated_collection_alias_slice_view_call_index_alias_rebind(
        &mut self
    ) {
        let collection: &mut [[u64; 2]; 2] = &mut self.other_grid_bucket.rows;
        let alias: &mut u64 = &mut self.cells[0];
        alias = &mut collection.as_mut_slice()[recursive_index()][make_index()];
        alias = 1;
    }

    machine Main::alias_chain_slice_view_call_index_alias_rebind(&mut self) {
        let parent: &mut [u64; 2] = &mut self.other_cells;
        let collection: &mut [u64; 2] = &mut parent;
        let alias: &mut u64 = &mut self.cells[0];
        alias = &mut collection.as_mut_slice()[
            identity_index(write_index(&mut self.value))
        ];
        alias = 1;
    }

    machine Main::deep_alias_chain_slice_view_call_index_alias_rebind(
        &mut self
    ) {
        let parent: &mut [u64; 2] = &mut self.other_cells;
        let collection: &mut [u64; 2] = &mut parent;
        let alias: &mut u64 = &mut self.cells[0];
        alias = &mut collection.as_mut_slice()[
            identity_index(identity_index(make_index()))
        ];
        alias = 1;
    }

    machine Main::recursive_alias_chain_slice_view_call_index_alias_rebind(
        &mut self
    ) {
        let parent: &mut [u64; 2] = &mut self.other_cells;
        let collection: &mut [u64; 2] = &mut parent;
        let alias: &mut u64 = &mut self.cells[0];
        alias = &mut collection.as_mut_slice()[recursive_index()];
        alias = 1;
    }

    machine Main::member_alias_chain_slice_view_call_index_alias_rebind(
        &mut self
    ) {
        let parent: &mut Bucket = &mut self.other_bucket;
        let collection: &mut [u64; 2] = &mut parent.cells;
        let alias: &mut u64 = &mut self.cells[0];
        alias = &mut collection.as_mut_slice()[
            identity_index(write_index(&mut self.value))
        ];
        alias = 1;
    }

    machine Main::deep_member_alias_chain_slice_view_call_index_alias_rebind(
        &mut self
    ) {
        let parent: &mut Bucket = &mut self.other_bucket;
        let collection: &mut [u64; 2] = &mut parent.cells;
        let alias: &mut u64 = &mut self.cells[0];
        alias = &mut collection.as_mut_slice()[
            identity_index(identity_index(make_index()))
        ];
        alias = 1;
    }

    machine Main::recursive_member_alias_chain_slice_view_call_index_alias_rebind(
        &mut self
    ) {
        let parent: &mut Bucket = &mut self.other_bucket;
        let collection: &mut [u64; 2] = &mut parent.cells;
        let alias: &mut u64 = &mut self.cells[0];
        alias = &mut collection.as_mut_slice()[recursive_index()];
        alias = 1;
    }

    machine Main::coarse_alias_slice_view_call_index_alias_rebind(&mut self) {
        let row: &mut [u64; 2] =
            &mut self.other_matrix[write_index(&mut self.value)];
        let alias: &mut u64 = &mut self.cells[0];
        alias = &mut row.as_mut_slice()[write_index(&mut self.other_value)];
        alias = 1;
    }

    machine Main::deep_coarse_alias_slice_view_call_index_alias_rebind(
        &mut self
    ) {
        let row: &mut [u64; 2] = &mut self.other_matrix[
            identity_index(identity_index(make_index()))
        ];
        let alias: &mut u64 = &mut self.cells[0];
        alias = &mut row.as_mut_slice()[make_index()];
        alias = 1;
    }

    machine Main::recursive_coarse_alias_slice_view_call_index_alias_rebind(
        &mut self
    ) {
        let row: &mut [u64; 2] = &mut self.other_matrix[recursive_index()];
        let alias: &mut u64 = &mut self.cells[0];
        alias = &mut row.as_mut_slice()[make_index()];
        alias = 1;
    }

    machine Main::coarse_member_slice_view_call_index_alias_rebind(&mut self) {
        let row: &mut Row =
            &mut self.other_row_items[write_index(&mut self.value)];
        let alias: &mut u64 = &mut self.cells[0];
        alias = &mut row.cells.as_mut_slice()[write_index(&mut self.other_value)];
        alias = 1;
    }

    machine Main::deep_coarse_member_slice_view_call_index_alias_rebind(
        &mut self
    ) {
        let row: &mut Row = &mut self.other_row_items[
            identity_index(identity_index(make_index()))
        ];
        let alias: &mut u64 = &mut self.cells[0];
        alias = &mut row.cells.as_mut_slice()[make_index()];
        alias = 1;
    }

    machine Main::recursive_coarse_member_slice_view_call_index_alias_rebind(
        &mut self
    ) {
        let row: &mut Row = &mut self.other_row_items[recursive_index()];
        let alias: &mut u64 = &mut self.cells[0];
        alias = &mut row.cells.as_mut_slice()[make_index()];
        alias = 1;
    }

    machine Main::coarse_helper_member_slice_view_call_index_alias_rebind(
        &mut self
    ) {
        let row: &mut Row =
            &mut self.other_row_items[write_index(&mut self.value)];
        let alias: &mut u64 = &mut self.cells[0];
        alias = &mut return_row(row)
            .cells
            .as_mut_slice()[write_index(&mut self.other_value)];
        alias = 1;
    }

    machine Main::deep_coarse_helper_member_slice_view_call_index_alias_rebind(
        &mut self
    ) {
        let row: &mut Row = &mut self.other_row_items[
            identity_index(identity_index(make_index()))
        ];
        let alias: &mut u64 = &mut self.cells[0];
        alias = &mut return_row(row).cells.as_mut_slice()[make_index()];
        alias = 1;
    }

    machine Main::recursive_coarse_helper_member_slice_view_call_index_alias_rebind(
        &mut self
    ) {
        let row: &mut Row = &mut self.other_row_items[recursive_index()];
        let alias: &mut u64 = &mut self.cells[0];
        alias = &mut return_row(row).cells.as_mut_slice()[make_index()];
        alias = 1;
    }

    machine Main::deep_slice_view_call_index_alias_rebind(&mut self) {
        let alias: &mut u64 = &mut self.cells[0];
        alias = &mut self.other_cells.as_mut_slice()[
            identity_index(identity_index(make_index()))
        ];
        alias = 1;
    }

    machine Main::recursive_slice_view_call_index_alias_rebind(&mut self) {
        let alias: &mut u64 = &mut self.cells[0];
        alias = &mut self.other_cells.as_mut_slice()[recursive_index()];
        alias = 1;
    }

    machine Main::nested_call_index_alias_rebind(&mut self) {
        let alias: &mut u64 = &mut self.cells[0];
        alias = &mut self.other_cells[identity_index(make_index())];
        alias = 1;
    }

    machine Main::nested_write_call_index_alias_rebind(&mut self) {
        let alias: &mut u64 = &mut self.cells[0];
        alias =
            &mut self.other_cells[identity_index(write_index(&mut self.value))];
        alias = 1;
    }

    machine Main::deep_call_index_alias_rebind(&mut self) {
        let alias: &mut u64 = &mut self.cells[0];
        alias =
            &mut self.other_cells[identity_index(identity_index(make_index()))];
        alias = 1;
    }

    machine Main::binding_reborrow_call_index_alias_rebind(&mut self) {
        let alias: &mut u64 = &mut self.cells[0];
        alias = &mut self.other_cells[identity_index(write_index(&mut alias))];
        alias = 1;
    }

    machine Main::recursive_call_index_alias_rebind(&mut self) {
        let alias: &mut u64 = &mut self.cells[0];
        alias = &mut self.other_cells[recursive_index()];
        alias = 1;
    }

    machine Main::repeated_call_index_alias_rebind(&mut self) {
        let alias: &mut u64 = &mut self.cells[0];
        alias = &mut self.other_matrix[write_index(&mut self.value)][
            write_index(&mut self.other_value)
        ];
        alias = 1;
    }

    machine Main::slice_view_repeated_call_index_alias_rebind(&mut self) {
        let alias: &mut u64 = &mut self.cells[0];
        alias = &mut self.other_matrix.as_mut_slice()[
            write_index(&mut self.value)
        ][write_index(&mut self.other_value)];
        alias = 1;
    }

    machine Main::deep_slice_view_repeated_call_index_alias_rebind(&mut self) {
        let alias: &mut u64 = &mut self.cells[0];
        alias = &mut self.other_matrix.as_mut_slice()[
            identity_index(identity_index(make_index()))
        ][make_index()];
        alias = 1;
    }

    machine Main::deep_repeated_call_index_alias_rebind(&mut self) {
        let alias: &mut u64 = &mut self.cells[0];
        alias = &mut self.other_matrix[
            identity_index(identity_index(make_index()))
        ][make_index()];
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

    let local = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::local_index_alias")
        .expect("local-index alias machine");
    let local_entry = typed
        .machine_states(local)
        .first()
        .expect("local-index alias entry state");
    assert_eq!(
        resolver
            .inferred_state_write_frame(local, local_entry)
            .complete_paths(),
        Some(["self.cells".to_owned()].as_slice()),
        "an effect-free local index preserves the alias's collection origin"
    );

    for (name, expected_paths) in [
        ("Main::call_index_alias", vec!["self.cells"]),
        (
            "Main::write_call_index_alias",
            vec!["self.cells", "self.value"],
        ),
        ("Main::nested_call_index_alias", vec!["self.cells"]),
        (
            "Main::nested_write_call_index_alias",
            vec!["self.cells", "self.value"],
        ),
        (
            "Main::slice_view_call_index_alias",
            vec!["self.cells", "self.value"],
        ),
        (
            "Main::helper_slice_view_call_index_alias",
            vec!["self.cells", "self.value"],
        ),
        (
            "Main::projected_helper_slice_view_call_index_alias",
            vec!["self.bucket.cells", "self.value"],
        ),
        (
            "Main::attached_projected_slice_view_call_index_alias",
            vec!["self.bucket.cells", "self.value"],
        ),
        (
            "Main::slice_view_member_call_index_alias",
            vec!["self.cell_bucket.cells", "self.value"],
        ),
        (
            "Main::collection_alias_slice_view_call_index_alias",
            vec!["self.cells", "self.value"],
        ),
        (
            "Main::member_collection_alias_slice_view_call_index_alias",
            vec!["self.bucket.cells", "self.value"],
        ),
        (
            "Main::repeated_collection_alias_slice_view_call_index_alias",
            vec!["self.matrix", "self.other_value", "self.value"],
        ),
        (
            "Main::member_repeated_collection_alias_slice_view_call_index_alias",
            vec!["self.grid_bucket.rows", "self.other_value", "self.value"],
        ),
        (
            "Main::alias_chain_slice_view_call_index_alias",
            vec!["self.cells", "self.value"],
        ),
        (
            "Main::member_alias_chain_slice_view_call_index_alias",
            vec!["self.bucket.cells", "self.value"],
        ),
        (
            "Main::coarse_alias_slice_view_call_index_alias",
            vec!["self.matrix", "self.other_value", "self.value"],
        ),
        (
            "Main::coarse_member_slice_view_call_index_alias",
            vec!["self.other_value", "self.row_items", "self.value"],
        ),
        (
            "Main::coarse_helper_member_slice_view_call_index_alias",
            vec!["self.other_value", "self.row_items", "self.value"],
        ),
        (
            "Main::repeated_call_index_alias",
            vec!["self.matrix", "self.other_value", "self.value"],
        ),
        (
            "Main::slice_view_repeated_call_index_alias",
            vec!["self.matrix", "self.other_value", "self.value"],
        ),
        ("Main::deep_call_index_alias", vec!["self.cells"]),
        ("Main::deep_slice_view_call_index_alias", vec!["self.cells"]),
        (
            "Main::deep_slice_view_repeated_call_index_alias",
            vec!["self.matrix"],
        ),
        (
            "Main::deep_projected_helper_slice_view_call_index_alias",
            vec!["self.bucket.cells"],
        ),
        (
            "Main::deep_attached_projected_slice_view_call_index_alias",
            vec!["self.bucket.cells"],
        ),
        (
            "Main::deep_slice_view_member_call_index_alias",
            vec!["self.cell_bucket.cells"],
        ),
        (
            "Main::deep_collection_alias_slice_view_call_index_alias",
            vec!["self.cells"],
        ),
        (
            "Main::deep_member_collection_alias_slice_view_call_index_alias",
            vec!["self.bucket.cells"],
        ),
        (
            "Main::deep_repeated_collection_alias_slice_view_call_index_alias",
            vec!["self.matrix"],
        ),
        (
            "Main::deep_member_repeated_collection_alias_slice_view_call_index_alias",
            vec!["self.grid_bucket.rows"],
        ),
        (
            "Main::deep_alias_chain_slice_view_call_index_alias",
            vec!["self.cells"],
        ),
        (
            "Main::deep_member_alias_chain_slice_view_call_index_alias",
            vec!["self.bucket.cells"],
        ),
        (
            "Main::deep_coarse_alias_slice_view_call_index_alias",
            vec!["self.matrix"],
        ),
        (
            "Main::deep_coarse_member_slice_view_call_index_alias",
            vec!["self.row_items"],
        ),
        (
            "Main::deep_coarse_helper_member_slice_view_call_index_alias",
            vec!["self.row_items"],
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
            "{name} must publish the index call's writes and retain the coarse collection origin"
        );
    }

    for name in [
        "Main::recursive_call_index_alias",
        "Main::recursive_helper_slice_view_call_index_alias",
        "Main::recursive_projected_helper_slice_view_call_index_alias",
        "Main::recursive_attached_projected_slice_view_call_index_alias",
        "Main::recursive_slice_view_member_call_index_alias",
        "Main::recursive_collection_alias_slice_view_call_index_alias",
        "Main::recursive_member_collection_alias_slice_view_call_index_alias",
        "Main::recursive_repeated_collection_alias_slice_view_call_index_alias",
        "Main::recursive_member_repeated_collection_alias_slice_view_call_index_alias",
        "Main::recursive_alias_chain_slice_view_call_index_alias",
        "Main::recursive_member_alias_chain_slice_view_call_index_alias",
        "Main::recursive_coarse_alias_slice_view_call_index_alias",
        "Main::recursive_coarse_member_slice_view_call_index_alias",
        "Main::recursive_coarse_helper_member_slice_view_call_index_alias",
        "Main::recursive_slice_view_call_index_alias",
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
            "{name} must remain opaque without a complete non-rebinding alias index"
        );
    }

    for (name, expected_paths) in [
        ("Main::call_index_alias_rebind", vec!["self.other_cells"]),
        (
            "Main::write_call_index_alias_rebind",
            vec!["self.other_cells", "self.value"],
        ),
        (
            "Main::prior_alias_survives_call_index_rebind",
            vec!["self.cells", "self.other_cells"],
        ),
        (
            "Main::prior_alias_survives_slice_view_call_index_rebind",
            vec!["self.cells", "self.other_cells", "self.value"],
        ),
        (
            "Main::prior_alias_survives_helper_slice_view_rebind",
            vec!["self.cells", "self.other_cells", "self.value"],
        ),
        (
            "Main::prior_alias_survives_attached_projected_slice_view_rebind",
            vec!["self.bucket.cells", "self.cells", "self.value"],
        ),
        (
            "Main::prior_alias_survives_slice_view_member_rebind",
            vec!["self.cells", "self.other_cell_bucket.cells", "self.value"],
        ),
        (
            "Main::prior_alias_survives_member_repeated_collection_view_rebind",
            vec![
                "self.cells",
                "self.other_grid_bucket.rows",
                "self.other_value",
                "self.value",
            ],
        ),
        (
            "Main::helper_slice_view_call_index_alias_rebind",
            vec!["self.other_cells", "self.value"],
        ),
        (
            "Main::projected_helper_slice_view_call_index_alias_rebind",
            vec!["self.other_bucket.cells", "self.value"],
        ),
        (
            "Main::attached_projected_slice_view_call_index_alias_rebind",
            vec!["self.bucket.cells", "self.value"],
        ),
        (
            "Main::slice_view_member_call_index_alias_rebind",
            vec!["self.other_cell_bucket.cells", "self.value"],
        ),
        (
            "Main::collection_alias_slice_view_call_index_alias_rebind",
            vec!["self.other_cells", "self.value"],
        ),
        (
            "Main::member_collection_alias_slice_view_call_index_alias_rebind",
            vec!["self.other_bucket.cells", "self.value"],
        ),
        (
            "Main::repeated_collection_alias_slice_view_call_index_alias_rebind",
            vec!["self.other_matrix", "self.other_value", "self.value"],
        ),
        (
            "Main::member_repeated_collection_alias_slice_view_call_index_alias_rebind",
            vec![
                "self.other_grid_bucket.rows",
                "self.other_value",
                "self.value",
            ],
        ),
        (
            "Main::alias_chain_slice_view_call_index_alias_rebind",
            vec!["self.other_cells", "self.value"],
        ),
        (
            "Main::member_alias_chain_slice_view_call_index_alias_rebind",
            vec!["self.other_bucket.cells", "self.value"],
        ),
        (
            "Main::coarse_alias_slice_view_call_index_alias_rebind",
            vec!["self.other_matrix", "self.other_value", "self.value"],
        ),
        (
            "Main::coarse_member_slice_view_call_index_alias_rebind",
            vec!["self.other_row_items", "self.other_value", "self.value"],
        ),
        (
            "Main::coarse_helper_member_slice_view_call_index_alias_rebind",
            vec!["self.other_row_items", "self.other_value", "self.value"],
        ),
        (
            "Main::nested_call_index_alias_rebind",
            vec!["self.other_cells"],
        ),
        (
            "Main::nested_write_call_index_alias_rebind",
            vec!["self.other_cells", "self.value"],
        ),
        (
            "Main::repeated_call_index_alias_rebind",
            vec!["self.other_matrix", "self.other_value", "self.value"],
        ),
        (
            "Main::slice_view_repeated_call_index_alias_rebind",
            vec!["self.other_matrix", "self.other_value", "self.value"],
        ),
        (
            "Main::deep_call_index_alias_rebind",
            vec!["self.other_cells"],
        ),
        (
            "Main::deep_slice_view_call_index_alias_rebind",
            vec!["self.other_cells"],
        ),
        (
            "Main::deep_slice_view_repeated_call_index_alias_rebind",
            vec!["self.other_matrix"],
        ),
        (
            "Main::deep_projected_helper_slice_view_call_index_alias_rebind",
            vec!["self.other_bucket.cells"],
        ),
        (
            "Main::deep_attached_projected_slice_view_call_index_alias_rebind",
            vec!["self.bucket.cells"],
        ),
        (
            "Main::deep_slice_view_member_call_index_alias_rebind",
            vec!["self.other_cell_bucket.cells"],
        ),
        (
            "Main::deep_collection_alias_slice_view_call_index_alias_rebind",
            vec!["self.other_cells"],
        ),
        (
            "Main::deep_member_collection_alias_slice_view_call_index_alias_rebind",
            vec!["self.other_bucket.cells"],
        ),
        (
            "Main::deep_repeated_collection_alias_slice_view_call_index_alias_rebind",
            vec!["self.other_matrix"],
        ),
        (
            "Main::deep_member_repeated_collection_alias_slice_view_call_index_alias_rebind",
            vec!["self.other_grid_bucket.rows"],
        ),
        (
            "Main::deep_alias_chain_slice_view_call_index_alias_rebind",
            vec!["self.other_cells"],
        ),
        (
            "Main::deep_member_alias_chain_slice_view_call_index_alias_rebind",
            vec!["self.other_bucket.cells"],
        ),
        (
            "Main::deep_coarse_alias_slice_view_call_index_alias_rebind",
            vec!["self.other_matrix"],
        ),
        (
            "Main::deep_coarse_member_slice_view_call_index_alias_rebind",
            vec!["self.other_row_items"],
        ),
        (
            "Main::deep_coarse_helper_member_slice_view_call_index_alias_rebind",
            vec!["self.other_row_items"],
        ),
        (
            "Main::deep_repeated_call_index_alias_rebind",
            vec!["self.other_matrix"],
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
            "{name} must move only the rebound alias to the direct-call indexed origin"
        );
    }

    for name in [
        "Main::binding_reborrow_call_index_alias_rebind",
        "Main::recursive_call_index_alias_rebind",
        "Main::recursive_helper_slice_view_call_index_alias_rebind",
        "Main::recursive_projected_helper_slice_view_call_index_alias_rebind",
        "Main::recursive_attached_projected_slice_view_call_index_alias_rebind",
        "Main::recursive_slice_view_member_call_index_alias_rebind",
        "Main::recursive_collection_alias_slice_view_call_index_alias_rebind",
        "Main::recursive_member_collection_alias_slice_view_call_index_alias_rebind",
        "Main::recursive_repeated_collection_alias_slice_view_call_index_alias_rebind",
        "Main::recursive_member_repeated_collection_alias_slice_view_call_index_alias_rebind",
        "Main::recursive_alias_chain_slice_view_call_index_alias_rebind",
        "Main::recursive_member_alias_chain_slice_view_call_index_alias_rebind",
        "Main::recursive_coarse_alias_slice_view_call_index_alias_rebind",
        "Main::recursive_coarse_member_slice_view_call_index_alias_rebind",
        "Main::recursive_coarse_helper_member_slice_view_call_index_alias_rebind",
        "Main::recursive_slice_view_call_index_alias_rebind",
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
            "{name} must remain opaque without a proven alias replacement origin"
        );
    }
}
