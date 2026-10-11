# build_optimization_requests

Each `builder.optimizations` counter is an at-most-once request, not a
tally: `= 1` selects the optimization (or `human_report`), and
`Optimizations::enable` / `Optimizations::emit_report` increment the
same counters — the counter value IS the request state, so two routes
to the same counter collide identically.

Controls — each rejects at build-time Build validation:

- `invalid_dup_counter`: `copy_propagation = 2` — "optimization
  `CopyPropagation` is enabled more than once".
- `invalid_dup_enable`: `Optimizations::enable` twice on
  `CopyPropagation` — same diagnostic, counter not toggle.
- `invalid_dup_report`: `Optimizations::emit_report` twice —
  "optimization human report is requested more than once".
