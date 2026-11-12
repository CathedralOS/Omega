# Element-view assignment retains its coordinate

The signed selector is captured before the RHS changes its source. The first
u32 element-view write and a following literal-index write consume their checked
index decisions; neither may search again for an unrelated live selector.
Refined element value ranges remain separate obligations.

`omega run --both tests/omega/pass/collections/runtime_element_view_selector_snapshot_exit/main.omg`
must return native and checked interpreter exit 0.

`python3 tools/corpus_gate.py --both --filter collections/runtime_element_view_selector_snapshot_exit`
must return native and Terminal interpreter exit 0.
