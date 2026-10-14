# Linear Ack Arms

Compile-scope probe: linear values consumed through `boundary
requirement` — a named consuming sink with no provider — and
decision-21 branch reconciliation. No standard-library dependency.

- `data Receipt [linear]` — must be consumed exactly once.
- `boundary requirement Receipt::ack(receipt: Receipt)` — the sink; no
  provider body needed.
- Either transition arm may transfer the same linear receipt into a
  consuming state — the arms are alternatives, not sequential copies.
- `Main::run -> i32` — value-returning machine; the entry
  (`ProgramEntry` can't return) reads the result into `hits`.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/linear_ack_arms/main.omg
mbx run -p omega -- update --project samples/cli/probes/linear_ack_arms
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.

## Native realization

Measured on macOS arm64 on 2026-10-13. The probe binds `ProgramEntry`, but its
only disposition of `Receipt` is a `boundary requirement` with no provider, and
native realization correctly refuses a reachable requirement with zero
selected provider rows (pinned by
`tests/omega/fail/providers/consuming_trait_requirement_without_provider`).
A native build needs either a selected provider for `Receipt::ack` or a probe
that binds no entry and moves into `tests/omega/`.
