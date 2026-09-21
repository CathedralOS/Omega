# GENERAL-CYCLIC-EXECUTION-OPTIMIZER — re-verification ledger

Re-verified on Linux x86-64 at `e7c0099cb2b7` (board tip at claim time) by
Zergling-126 under claim ticket `b66e78a0`. The item re-mines the optimizer
half of GENERAL-CYCLIC-EXECUTION: `TASKS_OPTIMIZER.md` carries the same-named
item owning the post-Terminal stages (receiving graph, native selection,
replay) starting at admitted cyclic input; this board's TASKS.md:1929 item
owns the Psi half.

## Facts re-verified on this revision

1. `TASKS_OPTIMIZER.md:355` — the optimizer-half row stands: natural-ranked
   and unranked modules already take the ordinary verification and
   abstract-lowering route; "artifact admission alone establishes no
   downstream optimization, target lowering, or publication support for
   those cycles."
2. `wiki/spec/language/termination.md:124` — § Ranked callees on projected
   receivers still documents the gap: projected calls (`compiler.parser.scan(...)`)
   borrow the parser field as the callee's whole receiver through ranked
   backedges; composed argument references, call/return, cleanup, callee
   measure checking, and composed resource evidence remain beyond today's
   whole-entry-only admission.
3. `omega-rust/omega/pipeline/terminal-psi-to-abstract-operations/src/artifact_admission/native.rs`
   still retains only whole-artifact `Vec<AcceptedControlCycle>` rosters
   (`accepted_control_cycles` field + accessor at :21/:43/:60/:81) — no
   per-callee call/return composition has landed.

## Live-claim state at re-verification

- `typed-trees-to-checked-trees/src/execution/unit/composed_control` — fenced
  to CONSERVATION-CONTRACT (9053768c, exp ~09:57Z Sep 21).
- `checked-trees-to-lowered-psi/tests/unit_state_graph*` — fenced to
  LOWERED-PSI-BASELINE-TAIL (ec0b88b0, exp ~12:21Z).
- The native-side `lowering/control_flow` + `receiver_calls` joins stay under
  sibling claims per the row's own record.

## Verdict

No independent slice exists: this is an extend-the-common-graph item owned by
GENERAL-CYCLIC-EXECUTION's optimizer half, and every implementing surface is
live-fenced elsewhere. Sibling re-mine: RANKED-PROJECTED-RECEIVER-COMPOSITION.
