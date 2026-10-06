Run `omega run --both tests/omega/pending/collections/runtime_nested_record_element_window_store_exit/main.omg` from the repository root; the authored result is exit 0.

The record-field write and read use the original window backing. The nested
primitive-array write currently lowers as a primitive projection of the
element-view descriptor instead of an indexed write of its selected backing,
and stage 05 refuses `primitive projection has an incompatible or out-of-bounds path`.
`CHECKER-DISCHARGES-OPERATION-OBLIGATIONS` owns the shared indexed-store route and
its occurrence-bound decisions; preserve the authored program across that repair.
