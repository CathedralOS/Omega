# Product description paths

Build describes a Product data type and named implementation, then verifies their exact
module-qualified paths. A failed comparison performs a missing-entry query,
so the fixture rejects during Build rather than depending on root-binding
discovery to observe the failed branch. Success runs the empty authored entry
and exits 0 on every hosted target.

An unrelated two-argument machine named `path` returns 7 and increments a
counter in both value and discard positions. Its result and final count verify
that compiler interception uses the selected toolchain identity, not the leaf
name. Both issued description kinds are also inspected with discarded results.

The path comparisons are operands of a short-circuit expression passed to another
machine. Its evaluated operands must retain their captured views through that
composition; neither dropping the query nor discarding the inspection may erase
its effect. Forged schema/provider path controls, a missing schema and a plain
data provider control live beside this fixture under `tests/omega/fail/build`.

Run with `omega update --project <this-directory> --target <host>`, then
`omega run <this-directory>/main.omg`. The native corpus route supplies its own
fixture package inputs: `python3 tools/corpus_gate.py --native --filter build/product_description_paths_exit`.
