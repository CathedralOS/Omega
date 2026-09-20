# RC-PCC-REPLAY — linux_x86_64 row

Release-matrix row for `RC-PCC-REPLAY` per
[rust_compiler_completion](rust_compiler_completion.md): requested
artifact/`.proof` pairs must round-trip; hostile or substituted evidence must
reject before PCC-required interpretation or lowering; ordinary output still
checks without publishing PCC.

| Field | Value |
| --- | --- |
| Commit | `ff782bdf21f0c63a695b5a90f47c59411cb9e5e2` |
| Toolchain | `nightly-2026-09-04` (rustc 1.100.0-nightly a69a63265) |
| Host | Linux x86_64 (`mbx` unavailable; Cargo used directly) |
| Result | **RED** — 74 of 3721 replay tests failed across all five packages |

## Invocation as published

```bash
mbx nextest run -p checked-trees-to-lowered-psi -p terminal-codec \
    -p terminal-verifier -p terminal-interpreter \
    -p terminal-psi-to-abstract-operations --no-fail-fast
```

(`mbx` absent on this host; `cargo nextest` used. `--no-fail-fast` kept.)

Summary `[1786.047s] 3721 tests run: 3647 passed (13 slow), 74 failed, 0
skipped`. The runner was terminated while
`checked-trees-to-lowered-psi::suite
nominal_affine_source::integer_comparison::mixed_nominal_integer_comparison_converges_before_one_shared_cleanup_return`
was still executing (SLOW markers through >1620 s); the termination is itself
row-blocking evidence — a non-converging replay cannot satisfy the gate.

Per-package failures: checked-trees-to-lowered-psi 54, terminal-codec 20,
terminal-interpreter 6, terminal-psi-to-abstract-operations 4,
terminal-verifier 2.

## Failure classes

- **Bare boundary-trait field rejection (~30, c2l suite `tests.rs:82`)** —
  fixtures now reject at check time: `field 'console' on data 'Main' names
  bare boundary trait 'Console' in value position; the intrinsic 'Service<R>'
  carrier is the only service value spelling`. The replay surface built on
  the old fixture shape (composed units, dynamic units, attachment cases)
  fails before lowering.
- **Missing transitive machine plan (~20, c2l `provider_attachment_source`,
  `unit_state_graph::provider_attachments`, `unit_plan_omissions`)** —
  `InvalidUnitMachinePlan { machine: "Main::main", reason: "attached Unit
  closure is missing a checked transitive machine plan", omission:
  "'Main::main' has no admitted body (local construction stopped at
  signature)" }`.
- **Wire-encoding drift (20, terminal-codec `block_wire`)** —
  `assertion left == right failed` in byte-field index/store, primitive-local
  tags, record operand-kind/constructor-tag, residual-jump path/tag,
  root-only-jump, structural-result/call tag, and write-only store
  round-trips.
- **Interpreter ordering (6, terminal-interpreter)** — affine-cleanup
  edge-charge ordering (`conditional_commits_only_the_selected_affine_cleanup_after_edge_charge`,
  `scalar_return_performs_affine_discard_only_after_edge_charge`) and
  `case_membership::projected_case_encoding_requires_the_extended_operation_format`.
- **Continuation/custody ordering (4, terminal-psi-to-abstract-operations)** —
  `partial_affine_call_results::continuations` result-owner ordering and
  `scalar_affine_cleanup::structural_return` singleton custody rows.
- **Trusted-surface digest drift (2, terminal-verifier)** —
  `trusted_surface::recorded_digests_match_the_working_tree` reports
  `validation/machine/scalar_result_operations.rs` sha256 `ddb3a1bb…` →
  `5d5eb15a…` for entries `["formation:machine-validation"]` — a source
  change landed (or is in flight) without digest revalidation.
- **Non-terminating (1)** — the integer-comparison straggler above.

## Doc leg

`mbx test --doc` for the same five packages (`cargo test --doc` used): exit 0;
these crates carry essentially no doctests (one passing item across the
selection).

## Disposition

Row stays open. The failure breadth crosses the whole replay chain —
check-time fixture rejection, plan omission, codec drift, interpreter
ordering, and trusted-surface digest — consistent with a wave of in-flight
sibling changes landing out of order rather than one broken leg. Re-run after
the wave settles; attribute residual failures per package then.

Skips: none taken. Windows/macOS/QEMU rows: not run on this host.
