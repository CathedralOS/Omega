# PARTIAL-OWNERSHIP-CLEANUP-EXPANSION — re-verification ledger

Re-verified on Linux x86-64 at `beaa8e1c1be7` (board tip at claim time) by
Zergling-126 under claim ticket `6c8ce536`. The row is a resolved re-mine of
CML4's named remaining work (TASKS.md:5269) — the expansion legs are CML4's
declared bullets, not an independent item.

## Drift since the row was written

The row's evidence ("`lowering/function/mod.rs` rejects a Jump with
`residual_affine_discards` under `UnsupportedPartialAffineContinuation`") is
stale: `aa698892c9a6` ("cml4: admit projected owned move arguments and
residual affine discards on jump edges") landed the Jump leg. On this
revision:

- `lowering/control_flow/terminator.rs:380-417` — `AbstractOperation::Jump`
  now maps `residual_affine_discards` to
  `TerminalAffineCleanupAction::DiscardResidual`, rejects only a residual
  boundary overlapping a transferred subtree (:391-397), and runs
  `plain_home_cleanup` over the mixed action set (:409).
- `terminator.rs:124-133` — `plain_home_cleanup` handles `DiscardResidual`
  through `residual_cleanup` (place/path-pair dedup, live home or arrival
  required, suspended roots reject).
- Conditional edges (:191, :219) still carry only `trivial_affine_discards`
  — residual cleanup on conditional edges is not yet realized.
- `lowering/unobserved_owned.rs:104` — the unobserved-owned fast path still
  requires `residual_affine_discards.is_empty()` on Jump.
- `image-emission/src/function_fragments/source/control_flow.rs:121` —
  fragment-source matching still requires residuals empty for the Jump leg
  it pins.

## Open legs still owned by CML4's row

- Residual cleanup on conditional edges (currently trivial-only), boundary
  call-result homes, projected copies, computed scalar bindings,
  boundary-result projections, cyclic control without delaying cleanup to
  final return.
- Entry-origin scalar continuation storage extended to operation-result
  values and defining identities.
- Psi: anonymous projected helper-result operands across multiple producers
  and non-Unit consumers.

## Verdict

Resolved as recorded — a re-mine of CML4's named remaining work. The Jump
residual leg has since landed at `aa698892c9a6` (row evidence was stale);
conditional-edge residuals and the listed storage/consumer legs remain
CML4-owned. No independent slice exists under this name.
