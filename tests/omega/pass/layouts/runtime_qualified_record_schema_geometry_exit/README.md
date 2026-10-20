# Qualified record schema geometry

`Word in Tagged` retains `Word`'s physical layout both as a direct record field
and as a fixed-array element; qualification on the array itself also preserves
its geometry. A plan-laid record containing those fields must
reflect and execute its size/alignment queries with native and Terminal exit 0.
This is geometry-query evidence, not a byte-view or resident initialization test.

The reject twin at
`tests/omega/fail/layouts/qualified_record_layout_requires_alignment` changes
only the second field's offset from 8 to 7. Qualification cannot waive the
ordinary alignment obligation.

`qualified_record_reference_is_not_inline` keeps a borrowed record unsupported
by schema reflection: forgetting qualification must not dereference its carrier.
