# NEW-WOB-IEEE-STORE-BRANCH-RECOVERY — recovery record at 891194236afa

Status: recovery ledger for the parked computed-IEEE-stores slice of
WRITE-ONLY-BORROW (TASKS.md). This note records whether the unpublished
branch survives, what it carried, and the two sanctioned paths forward. It
authorizes no new gate and changes no fence.

## The parked slice

Recorded by `324eb6d84a0d` (2026-09-16, swarm parked-WIP evidence): an
unmerged local branch `write-only-borrow`, tip `71a647f464`, sitting over
`348c542350ce` ("omega: project selected operator crash routes through
package review") in the **Windows coordinator checkout** — not on `origin`.
It carries the computed-IEEE-stores slice of WRITE-ONLY-BORROW:

- selected IEEE binary operations through the native pipeline, and
- `&write` field reads / computed stores,

135 files in total. The parent row records: "Ask the coordinator whether it
still exists before re-implementing."

## Recovery evidence on `891194236afa` (linux x86-64)

The tip commit is not recoverable from this repository:

- `git cat-file -t 71a647f464` — not a valid object name; the object is
  absent from the local object store.
- `git branch -r --contains 71a647f464` — malformed object name; no remote
  ref carries it.
- No remote ref is named `write-only-borrow`. The only remote branches
  touching this surface are annotations, not the slice:
  - `origin/zergling/z149-write-only-borrow-residue` — a single TASKS.md
    claim-state wording correction (`dff3205dd10b`).
  - `origin/checkpoint/write-only-diagnostic-expectations-20260905` — two
    commits pinning expected diagnostics (`d2608663e4ae`, `d5e80e371866`).
- The base `348c542350ce` **is** an ancestor of `origin/main`, so the parked
  work sat on a real commit; only the branch objects are missing here.

Conclusion: the slice exists, if anywhere, only in the Windows coordinator
checkout it was parked on. This machine cannot fetch it.

## Path A — coordinator recovery (preferred, per the parent row)

The parent row's recorded protocol stands: the coordinator merges the parked
branch to main through the landing queue. That preserves the slice's exact
contents — 135 files touching Psi producer planning and Omega provider/
graph routes — whose drift against current main is unknown but was authored
against `348c542350ce`.

If the Windows checkout still holds the branch, the coordinator push makes
this leg reviewable instead of re-derived.

## Path B — re-implementation contract

If the coordinator checkout is gone, the slice is re-implemented, and this
note is the contract for what "recovered" means:

**Failing pin to fix.** `tests/native-differential/tests/terminal_psi_
indexed_receivers/frontier_pins.rs::guarded_index_and_computed_stores_
still_miss_the_checked_control_plan` asserts three shapes reject with "no
source-independent checked scalar control plan". The IEEE leg is:

```text
machine forward(values: &mut [f64; 4], left: f64, right: f64) {
    values[2] = left + right;
}
```

a computed (non-literal) floating store into borrowed caller storage. The
sibling pinned shapes — guarded dynamic index store and guarded dynamic
index receiver call — share the checked-control-plan hole but belong to the
runtime-index legs, not this slice.

**What the slice must carry** (from the row's bullet): a source-selected
floating operation, its result transport, and an ordinary store, retaining
format, selected occurrence, and result evidence through Psi's
`execution/unit/selected_ieee_float.rs` and Omega's shared graph/provider
route. Widening store admission is not the repair.

**Current producer state.** `typed-trees-to-checked-trees/src/execution/
unit/selected_ieee_float.rs` only emits `SelectedIeeeFloatFusedMultiplyAdd`
for intrinsic FMA applications bound to `LocalInitializer` locals with
literal operands (`ieee_format_for_primitive` gates F32/F64). The missing
piece is the store direction: a computed floating result reaching a
write-only/mutable place while preserving format and result evidence.

**Owners and neighbors.** FLOAT-PROVIDERS owns the selected floating
operations themselves; this slice is the transport into borrowed storage.
The regression suite is the maintained `terminal_psi_indexed_receivers`
integration target — run `mbx nextest run -p omega-native-differential-test
--test terminal_psi_indexed_receivers --no-fail-fast --no-tests fail` (with
`RUST_MIN_STACK=67108864` on macOS ARM64); preserve `held_borrows`,
`indexed_stores`, `owned_subloans`, `borrowed_arguments`, and
`primitive_stores` as coverage, and extend the shared place/loan sequencer
under STATE-LOCAL-VALUE-FRONTIER rather than a one-off store emitter. The
acceptance wants the computed floating store observed on the caller, not
only in checking or a copied frame home.

## Fence snapshot at 891194236afa (advisory, drifts with rotation)

Surfaces this leg touches that were live-claimed when this record was
written — treat as coordination points, not permanent fences:
`execution/unit/*` neighbors under CLEANUP-HOOK-SELECTION-AND-ERASED-OWNERSHIP
(~08:46Z), PROVIDER-ATTACHMENT-MACHINE-PLAN (~09:49Z), and
GENERAL-CYCLIC-EXECUTION-OPTIMIZER (~state_graph/composed_control); the
native lowering lane under FLOAT-PROVIDERS family claims. Re-check
`tools/claims.py status` at implementation time.
