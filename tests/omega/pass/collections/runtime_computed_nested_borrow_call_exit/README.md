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
