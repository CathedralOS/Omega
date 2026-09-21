# PARTIAL-OWNERSHIP-CLEANUP-EXPANSION — re-verification ledger

Re-verified on Linux x86-64 at `75650d2e94` (board tip at claim time) by
zergling Z28 ("Devin / w10-w10-30-partial-ownership-cleanup") under claim
ticket `0cb8b3ca`. The row is a resolved re-mine of CML4's named remaining
work (TASKS.md, the CML4 item) — the expansion legs are CML4's declared
bullets, not an independent item.

## Drift since the row was written

The row's evidence ("`lowering/function/mod.rs` rejects a Jump with
`residual_affine_discards` under `UnsupportedPartialAffineContinuation`") is
stale: `aa698892c9a6` ("cml4: admit projected owned move arguments and
residual affine discards on jump edges") landed the Jump leg. On this
revision:

- `lowering/control_flow/terminator.rs:380-412` — `AbstractOperation::Jump`
  maps `residual_affine_discards` to
  `TerminalAffineCleanupAction::DiscardResidual`, rejects only a residual
  boundary overlapping a transferred subtree (:391-397), and runs
  `plain_home_cleanup` over the mixed action set (:411).
- `terminator.rs:104-135` — `plain_home_cleanup` handles `DiscardResidual`
  through `residual_cleanup` (place/path-pair dedup, live home or arrival
  required, suspended roots reject). `Return` (:322) and `ReturnUnit` (:361)
  reach it through their unified `cleanup_actions`; `ReturnUnitPartialAffine`
  residuals already lower into `DiscardResidual` actions at
  `lowering/machine/terminator.rs:298-310`, so residual discards on those
  return forms realize natively when producers emit them.
- Conditional edges still carry only `trivial_affine_discards` (the
  `successor` closure at `terminator.rs:191`); `ReturnStructural` likewise
  (:219); `StructuralCase` delegates to `structural_case::lower` (:358).
- `lowering/function/mod.rs` remains a 43-line file whose sole rejection is
  `ScalarBoundaryArgumentsRequireNativeRealization` (:25); it does not
  inspect `residual_affine_discards`, and `UnsupportedPartialAffineContinuation`
  has zero hits in any `.rs` file.
- `lowering/unobserved_owned.rs` — the earlier ledger bullet is stale. Since
  `c58da7957f6` the gate no longer requires
  `residual_affine_discards.is_empty()` on Jump: the Jump arm (:96-110)
  forwards residuals to `edge_discards` (:169-204), which admits each
  strictly pathed residual rooted at a declared owned affine arrival,
  non-overlapping prior discards and transferred structural arguments, and
  `plain_type`-restricted. Conditional arms still pass `&[]` residuals
  (:122).
- `image-emission/src/function_fragments/source/control_flow.rs:121` —
  fragment-source matching still requires residuals empty for the Jump leg
  it pins.

## Representation still lacks residual slots on branching edges

- Abstract operations: `AbstractSuccessor`
  (`abstract_operations/control_flow/edges.rs:13`) and
  `AbstractStructuralCaseSuccessor` (:29) carry only
  `trivial_affine_discards`.
- Terminal Psi: `SuccessorEdge` (:208), `StructuralCaseSuccessorEdge` (:220),
  `ReturnUnit` (:62), and `ReturnStructural` (:88) in
  `control_flow/termination.rs` carry only `trivial_affine_discards`; the
  residual-capable forms are `Jump` (:31), `Return`'s unified
  `cleanup_actions` (:54), and `ReturnUnitPartialAffine` (:71). Per CML4's
  row, `encoding.md` gives the successor-edge wire row no residual slot, so
  extending it is a spec amendment plus a five-surface chain (representation,
  producer, codec, verifier, interpreter, lowering).
- Producers emit non-empty residuals only at
  `checked-trees-to-lowered-psi/src/unit/attached_unit/structural_values/
  emission.rs:1556-1609` and `.../argument_evaluation.rs:497`; every
  Conditional and ReturnUnit site writes `Vec::new()`.

## Open legs still owned by CML4's row

- Residual cleanup on conditional and structural-case edges (currently
  trivial-only), boundary call-result homes, projected copies, computed
  scalar bindings, boundary-result projections, and cyclic control without
  delaying cleanup until final return.
- Entry-origin scalar continuation storage extended to operation-result
  values and their exact defining identities.
- Psi: anonymous projected helper-result operands across multiple producers
  and non-Unit consumers; the type-directed record/array complement extended
  to construction-local roots and mixed dying-root schedules.

## Verdict

Resolved as recorded — a re-mine of CML4's named remaining work. The Jump
residual leg landed at `aa698892c9a6` and the unobserved-owned gate admits
its residual discards since `c58da7957f6`; conditional and structural-case
edges still lack residual slots at both representation levels, and the
listed producer/storage legs remain CML4-owned. No independent slice exists
under this name.
