# RC PCC replay — linux_x86_64 row

First evidence record for the `RC-PCC-REPLAY` release gate (matrix row in
`wiki/drafts/rust_compiler_completion.md:39`, commands encoded at
`tools/release/release_record.py:72-84`). The only prior release record
(`wiki/drafts/release_record_e12b9e8e06.md:26`) marked the gate "not
completed — did not reach a verdict within this leg's bound". This row
reproduces the gate commands verbatim (cargo spelling — `mbx` absent on
this host) on the Linux x86-64 host.

Recorded at `5ae1ed1fe51c`, host `x86_64-unknown-linux-gnu`,
cargo-nextest 0.9.140.

## Command 1 — library/nextest leg

```
mbx nextest run -p checked-trees-to-lowered-psi -p terminal-codec \
  -p terminal-verifier -p terminal-interpreter \
  -p terminal-psi-to-abstract-operations --no-fail-fast
```

Run verbatim as `cargo nextest run ... --no-fail-fast`.

Verdict: **terminates at this revision — red, 1474.9s.** `3840 tests
run: 3786 passed (13 slow), 54 failed, 0 skipped` — 53 FAIL lines plus
one member killed by the harness after 1385.2s.

The row's hypothesized non-termination is confirmed in mechanism but
not in outcome: `checked-trees-to-lowered-psi`'s
`nominal_affine_source::integer_comparison::mixed_nominal_integer_comparison_converges_before_one_shared_cleanup_return`
never returns a verdict on its own — it was still running at >1200s
when the harness SIGTERM'd it at 1385.2s — so the gate can only complete
because the test harness bounds the member. Owner:
**C2L-PROOF-SEARCH-BLOWUP-CONTAINMENT** (live claim, claude-opus-goal,
exp ~2026-09-21T09:18Z).

### Per-package tally

| package | passed | failed | notes |
|---------|--------|--------|-------|
| checked-trees-to-lowered-psi | 2181 | 22 FAIL + 1 SIGTERM | provider-attachment + scalar-return custody + plan-omission families + the blowup member |
| terminal-codec | 359 | 22 FAIL | canonical wire-tag drift |
| terminal-verifier | 842 | 4 FAIL | nominal-affine-cleanup contract drift |
| terminal-interpreter | 290 | 3 FAIL | affine-cleanup + case-membership legs |
| terminal-psi-to-abstract-operations | 114 | 2 FAIL | affine-continuation/cleanup legs |

### Failure attribution

Every failure reproduces a family already recorded in
`wiki/drafts/known_baseline_failures.md` or named on the board — nothing
unattributed appeared.

- `checked-trees-to-lowered-psi` **provider-attachment lane (15)** —
  `provider_attachment_source` ×6 + `unit_state_graph::provider_attachments`
  ×9 + `unit_plan_omissions` ×3, all panicking on
  `InvalidUnitMachinePlan { "attached Unit closure is missing a checked
  transitive machine plan" }` or the sibling omission-stage assertion.
  Owner: the provider-attachment/transitive-machine-plan lane
  (C2L-BASELINE-FAILURE-ATTRIBUTION family; transitive-plan residual
  assigned to GENERAL-CYCLIC-EXECUTION + UEFI-OS-HANDOFF).
- `checked-trees-to-lowered-psi` **scalar-return custody (4)** —
  `owned_record_return_source` ×3 +
  `guarded_scalar_returns_source::stored_returned_cases_support_borrowed_refined_getters`.
  Owner: C2L-SCALAR-RETURN-SOURCE-CUSTODY-FAILURES (live claim,
  devin-c944dc29, exp ~10:35Z); two members additionally route to
  WRITE-ONLY-BORROW's claimed surfaces.
- `checked-trees-to-lowered-psi` **SIGTERM (1)** — the proof-search
  blowup above. The C2L-UNATTRIBUTED-FAILURE-TAIL census at
  `e5bbe53956` recorded one additional member
  (`unit_state_graph::bindings` ×1) that now passes — net c2l shrinkage
  of one.
- `terminal-codec` **canonical wire-tag drift (22)** — every failure is
  an exact-bytes round-trip pinning the previous vocabulary tag
  (`left: [104, …]`, `right: [103, …]`): `canonical::*` ×11
  (suspension_and_scalar_round_trips ×8, bounded_integer_fields,
  operation_crash_contracts, structural_call_results),
  `ledger_spike` ×3, `publication` ×3, `trust_graph` ×2,
  `structural_block_wire_tests` ×2, `quotient_correspondence` ×1.
  Same family the RC-REPOSITORY-CLOSURE row recorded as "terminal-codec
  20 (wire-tag drift)" after `f94e78ec39`.
- `terminal-verifier` **nominal-affine-cleanup contract (4)** —
  `validate_module` no longer raises `InvalidNominalAffineCleanup` for
  the pinned contract-carrying/nonexact-closure cases
  (`tests/structural_unit/nominal_affine_cleanup.rs:427/:470` and
  siblings). Verifier-side validation drift — no owning row recorded.
- `terminal-interpreter` (3) — `affine_cleanups` ×2
  (conditional-commits / scalar-return edge-charge ordering) and
  `case_membership::projected_case_encoding` extended-format leg.
- `terminal-psi-to-abstract-operations` (2) —
  `partial_affine_call_results::continuations` and
  `scalar_affine_cleanup::structural_return` legs.

## Command 2 — doctest leg

```
mbx test --doc -p checked-trees-to-lowered-psi -p terminal-codec \
  -p terminal-verifier -p terminal-interpreter \
  -p terminal-psi-to-abstract-operations
```

Run verbatim as `cargo test --doc ...`.

Verdict: **green.** 1 doctest
(`terminal_psi_to_abstract_operations::artifact_admission::retention`,
compile-fail, ok); the other four packages carry no doctests.

## Disposition

The gate now has its first linux_x86_64 record: **red but measured** —
command 1 completes at this revision (previously recorded as
never reaching a verdict; the difference is the harness bounding the
hang member at 1385s, so the gate verdict is `test run failed` rather
than silence). All 54 failures attribute to owned residual families or
recorded drift; the gate's unblock order is C2L-PROOF-SEARCH-BLOWUP-
CONTAINMENT first (the non-terminating member), then the c2l
provider-attachment/scalar-return lanes, then the codec wire-tag
repin and the verifier/interpreter/t2a affine legs. Per
`rust_compiler_completion.md` the gate also needs matching records on
linux_arm64, macos_arm64 and windows_x86_64 hosts, which this host
cannot produce.
