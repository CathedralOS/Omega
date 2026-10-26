# Nested dispatch address evaluation

Expected exit: 0. `matrix[row][advance(&mut row)]` captures the selected
row before evaluating the outer selector. The selector writes `row = 1` and
returns column zero. The saved row is zero, whose element is 11; using the
new row instead selects 22 and exits 71. The same bound `< 2`, operation and
expected result are retained from the frozen genuine-main ordering witness.

The authored scalar postconditions retain the row and returned column bounds;
no caller equality guard supplies them. Both engines must observe exit zero.
The existing `fail/collections/nested_read_index_uses_selected_extent` control
retains each nested selector's own extent obligation.

The separate named-domain version remains a valid lowering frontier and is
retained in ignored baseline evidence; this fixture does not close it.
