# Index Every Width

## Nine-place native witnesses

The original root `main.omg` and `build.omg` are retained byte for byte as a
historical compile-scope probe. The root has no ProcessExit provider. Its
bracketed scalar ranges predate the canonical contract syntax; historical
compile-only observations do not establish native acceptance for this row.

Every integer width with a proved bound `0 <= index <= 3` indexes a four-element
array and XORs the selected `u64` values. The `local`, `field`, `parameter`,
`call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, and
`target_body` projects each carry that same fact and operation. Their native
oracle is exit 0: `[1, 2, 4, 8]` indexed at `0, 1, 2, 3, 0, 1, 2, 2` XORs to 12.
The match witness also executes its other arm, whose eight indices are 3 and
whose XOR is 0. A wrong result crashes `Trap`.

The corresponding `tests/omega/pass/facts/index_every_width_through_*_exit`
fixtures mirror the sample sources. Each corresponding fail fixture widens only
the `i64` upper bound to 4 (or changes its constant to 4), retaining the same
four-element array and indexing operation. Rejection must identify that access,
rather than an unrelated formation, contract-admission, or lowering error.

The existing language rules govern these witnesses:

- [Counts, indices, and addresses](../../../../wiki/spec/language/counts_and_addresses.md#bounds-and-lowering):
  "Scalar indexing requires `0 <= index < live_length`." A fixed array has its
  exact declared length; every signed width also needs nonnegativity evidence.
- [Numeric values](../../../../wiki/spec/language/numeric_values.md): bounds use
  data `where`, parameter `requires`, result `ensures`, and ordinary local flow
  facts. The numerical bound remains 0 through 3; no range syntax is added.
- [State contracts](../../../../wiki/spec/language/state_contracts.md#bindings-and-arrivals):
  internal state arrivals establish the target state's own requirements.

Run the native projects through the focused sample gate:

```sh
python3 tools/sample_native_gate.py --cli /absolute/path/to/recorded/omega \
  --filter cli/facts/index_every_width/ --jobs 2
python3 tools/corpus_gate.py --filter facts/index_every_width_through_
python3 tools/corpus_gate.py --native --filter facts/index_every_width_through_
```

In the saved cloud environment, sandboxed ownership inspection presents `/` as
UID 65534, but the supported approved execution path verifies `/` as UID 0 and
runs the compiler with unchanged custody. Use that approved path for builds and
checks; source `/workspace/cloud-setup/activate.sh` to select the installed
pinned toolchain and cache environment. Do not change ownership, permissions, or
custody checks to accommodate the sandbox presentation.
