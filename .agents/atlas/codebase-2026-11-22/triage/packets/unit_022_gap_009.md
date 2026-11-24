# Packet unit_022_gap_009: replay checks in image-emission `function_fragments.rs` 7801-13800

Status: in progress (early receipt).

Scope: area 11 only, [record unit_022_gap_009](../../records/unit_022_gap_009.md) and
[units/gap_009.tsv](../../units/gap_009.tsv) (`function_fragments.rs` lines 7801-13800). Baseline
`24ad2700685e6cc6062026c87ed5fd24ed415265`; the file is byte-identical at that baseline and at the review head,
so every line number below is valid for both. `FF` = `source-rust/subsystems/compiler/psi-to-native/pipeline/15_objects-to-image/image-emission/src/function_fragments.rs`.
No merged triage row in [07-13-psi-and-native.md](../07-13-psi-and-native.md) cites this range.

## Claims

### C1. Cleanup rosters of `InvokeNoNormal`, `ExternalCompletion`, `EliminateEmpty`, `Crash` (record §2 fail-open 1; digest 11 §5 `FF:8325-8327` with `:8075-8174`, §6 first `FF` bullet)

- Code path: `control_flow::retained` (`FF:8004`) -> `cleanup_rosters_match` (`FF:8270-8293`).
- Contract: a terminator's cleanup calls in the target block must be exactly the source's `InvokeOwned` cleanup actions.
- Trace: `AbstractOperation::cleanup_rosters` (`abstract-operations/src/operations.rs:800-838`) yields rosters only
  for `Jump`, `Return`, `ReturnUnit`, `ReturnStructural`, `Conditional` and `StructuralCase`. The four named
  abstract operations have no cleanup-action field (`operations.rs:672-701`), and neither do their target
  terminators (`target-operations/src/control_flow.rs`; only `ReturnStructural`, `ReturnScalar`, `Return` and the
  successors carry `cleanup_actions`). For these four, `expected` is 0, so `cleanup_rosters_match` requires the
  candidate block to hold zero `CleanupCall` rows (`FF:8290`); any cleanup call there is refused.
- Outcome: **intended** (fails closed). The skipped `DiscardRoot`/`RetireHome`/`DiscardResidual` actions produce
  no call row by design, and for every terminator that carries a roster the full action list is compared for
  equality (`FF:8188`, `:8211`, `:8239`, `:8267`; `ReturnStructural` at `FF:7965-7992`).
- Proposed fix: none.
