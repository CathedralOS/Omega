# Repair partition

Generated 2026-09-26 against `aedc8140fc`. This is a view over the execution
boards, not a fourth board: items live and die in `TASKS.md`,
`TASKS_BOOTSTRAP.md`, and `TASKS_OPTIMIZER.md`, and this file must not
accumulate its own acceptance criteria or status prose. Refresh or delete it
when the partition is stale; delete it outright when the repair and basis
sections are empty.

The premise: red trees are tolerable while the basis is unstable, so repair
and basis work outranks feature work. **Repair** means current behavior is
wrong, unsound, or measured red. **Basis** means pipeline-shape conformance
and the gates that define and measure stability. Everything else on the
boards is feature work and waits behind this list unless a basis item depends
on it.

## Ordering

Land in this order so each repair collides with the fewest in-flight changes:

1. **Pipeline route** — the shape stops moving first; later repairs land on
   the settled structure.
2. **Repair items** — correctness holes and measured reds.
3. **Corpus and release gates** — the instruments that report the basis is
   actually stable.

## Basis: pipeline route (TASKS.md, "Pipeline route")

- `OPERATOR-BOUNDARY-CALLS` — also carries an open repair: eight native
  builds still reject pending the generic ProviderPlan join. Land it with
  the route work.
- `SOURCE-SET-UNION`
- `PSI-TARGET-FAMILIES`
- `BUILD-EVALUATES-ONCE`
- `PROVIDER-SELECTION-AFTER-TERMINAL`
- `ONE-DRIVER-PER-STAGE`

## Basis: structural conformance

- `LOWERING-ROUTE-CONSOLIDATION` — seven parallel whole-module producers
  collapse to one route.
- `POST-HANDOFF-WRITER-OWNERSHIP` — portable materialization description vs.
  consumer implementation.
- `PCC-CANONICAL-SEMANTIC-LEDGER` — trusted fusion replaced by a checkable
  ledger.

## Repair (TASKS.md)

- `OWNED-SELF-RECEIVER-AFFINE-DISCARD` — receiver-only exemption bypasses
  cleanup validation.
- `FILTERED-CALLEE-CALL-SITES` — dropped calls produce no diagnostic.
- `TYPED-TREES-OWNED-INDEXES` — symbol caches guess freshness from arena
  lengths.
- `NATIVE-WRAPPER-ENCODING-AARCH64` — selects the x86-64 template for
  aarch64.
- `CALLBACK-PRIVATE-MATERIALIZATION` — reported `fragment import lacks
  selected call custody` failure; the item says reproduce and repair.
- `NOMINAL-FIELD-FLOW` — receiver-specific default-domain exception;
  implementation cleanup.
- `CANONICALIZE-SCALAR-RANGE-CONTRACTS` — unfinished owner-mandated removal
  of bracketed range spellings.
- `CHECK-CLOSURE-ONCE` — `--check` compiles the bundled library twice.
- `C2L-PROOF-SEARCH-BLOWUP-CONTAINMENT` — compile-time proof blowup from
  repeated whole-module reconstruction.

## Repair (TASKS_BOOTSTRAP.md)

- `ALPHA-WINDOWS-DIV-TRAP` — Windows `div`/`mod` take hardware faults instead
  of the audited trap routine.
- `ALPHA-SEED-MEMSIZE-NATIVE-VALIDATION` — appears discharged by
  `1a70b42afe` (34/34 conformance, 78/78 bounds natively); verify and delete
  rather than re-work.

## Basis: corpus and release gates

- `CANARY-CORPUS` — includes the ~42 fixtures no roster compiles and the
  `Lowering(InvalidUnitMachinePlan)` red cluster; attribution belongs to the
  named capability items, the roster accounting is repair.
- `SAMPLE-CORPUS` — the sample refusal census (unit-plan omission, `Clock`
  provider) is the basis reading; fixing the underlying causes belongs to
  their owning items.
- `RC-REPOSITORY-CLOSURE` — the wave's natural acceptance gate.
- `RC-BUILD-AND-PACKAGES`, `RC-PCC-REPLAY`, `RC-RELEASE-RECORD-AND-CLOSURE`
- `RC-NATIVE-MATRIX-LINUX-X86-64`, `RC-NATIVE-MATRIX-LINUX-ARM64`,
  `RC-NATIVE-MATRIX-MACOS-ARM64`, `RC-NATIVE-MATRIX-WINDOWS-X64`,
  `MACOS-X64-HOST-PROFILE` — matching-host evidence legs.
- `GENERATED-CODEC-INDEPENDENT-VERIFICATION`, `SYMBOLIC-MATERIALIZATION`

## Basis (TASKS_OPTIMIZER.md)

- `WORKSPACE-ROLLOUT`, `TRANSLATION-VALIDATION`,
  `CUSTODY-MUTATION-COVERAGE`, `PER-RULE-COVERAGE` — coverage/validation
  surfaces; the remaining optimizer items are missing optimizations and are
  feature-classed for this partition.

## Swarm discipline for the wave

- Fewer lanes than a feature wave; repair and basis lanes hold claims on the
  shared paths they touch so a concurrent feature landing cannot silently
  revert the fix (the `912b490c8f` failure mode).
- No "repaired" claim without the item's witness command re-run at current
  HEAD. A red test that stopped rejecting needs an attribution (stale
  fixture vs. lost check), not a green by default.
- Baseline rows in `wiki/drafts/measurements/known_baseline_failures.md` are
  the work queue; each row closes by fix, re-attribution to a task diff, or
  measured obsolescence — not by deletion.
- All landings through `tools/landing.py`; check
  `python tools/claims.py status` before touching shared paths.
