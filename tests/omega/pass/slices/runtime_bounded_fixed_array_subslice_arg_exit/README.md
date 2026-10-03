# Runtime-bounded array window

After ordinary package preparation, run:

```sh
cargo run -p omega -- run --both tests/omega/pass/slices/runtime_bounded_fixed_array_subslice_arg_exit/main.omg
```

Expected exit: **3**. Both bounds are runtime parameters. The guard must precede
slice formation, and the consumer must receive the selected extent through an
ordinary call. The empty endpoint window returns 0; reversed and out-of-range
bounds return 8 without forming a slice. Any disagreement exits 9.

The view helper borrows the entry shared and passes only the slice to the
consumer. Passing an exclusive entry borrow beside its live array loan would
alias the backing. Console exit happens after the shared call returns.

The corpus route is `python3 tools/corpus_gate.py --both --filter
slices/runtime_bounded_fixed_array_subslice_arg_exit`. It uses repository test
package inputs; it does not establish ordinary project/package approval.
