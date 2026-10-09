Run the authored caller-storage oracle with:

```sh
omega run --both tests/omega/pass/collections/runtime_computed_nested_borrow_call_exit/main.omg
```

Both executions must exit 0. Two arithmetic selectors cross an intervening
record field before lending a mutable primitive leaf. The callee increments
the selected caller cell; adjacent cells and the row tag retain their values.
The source is the unchanged regression that failed at parameter-path planning.

The companion selector-order and selector-join fixtures reset the selector
fields in a later argument. Their earlier borrow must retain the original
selected cell, including across the short-circuit evaluation join. The field-only
order fixture pins the same snapshot rule without arithmetic in the indices.
The signed
shared-borrow fixture reads the same nested geometry with distinct signed
integer carriers. The selector-once fixture counts effectful helper calls
inside both indices and a later argument; each must execute once.
The selected-extent and shared-authority fail fixtures
retain independent bounds and access refusals.

Independent review adds dynamic Unit call coverage: the checked dispatch slot owns the retained formal roster, and its dispatch selector leads the same staged scalar operand namespace. `runtime_dynamic_indexed_borrow_call_exit` preserves the exact reviewer witness; `runtime_dynamic_borrow_selector_order_exit` and `runtime_dynamic_borrow_selector_join_exit` preserve captured field indices across later mutation and private joins. Each authored native and Terminal exit is 0.

The unchanged restricted shared-read witness in `pending/layouts/runtime_indexed_restricted_field_read` still reaches `SourceCustodyMismatch` at machine 2, operation 18. Computed-index dynamic trait probes remain an inherited parameter-path planning limit on exact main and candidate; they are preserved with unchanged exit-0 oracles in the review evidence packet. These frontiers are not closed by this selector repair.
