# Repair partition

Generated 2026-09-26 against `aedc8140fc`. This is a view over the execution
boards, not a fourth board: items live and die in `TASKS.md`,
`TASKS_BOOTSTRAP.md`, and `TASKS_OPTIMIZER.md`, and this file must not
accumulate its own acceptance criteria or status prose. Refresh or delete it
when the partition is stale; delete it outright when the repair and basis
sections are empty.

The premise: red trees are tolerable while the basis is unstable, so repair
and basis work outranks feature work. **Repair** means a valid program that
the compiler rejects, omits, or runs wrong. **Basis** means the pipeline
shape a repair lands on. Everything else on the boards is feature work and
waits behind this list unless a basis item depends on it.

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

- `LOWERING-ROUTE-CONSOLIDATION` — consolidate competing producers while
  preserving ordinary/composed operation coverage and execution oracles.
- `PCC-CANONICAL-SEMANTIC-LEDGER` — trusted fusion replaced by a checkable
  ledger.

## Repair (TASKS.md)

- `OWNED-SELF-RECEIVER-AFFINE-DISCARD` — receiver-only exemption bypasses
  cleanup validation.
- `FILTERED-CALLEE-CALL-SITES` — finish statement-call and same-target-caller
  acceptance for the existing unavailable-callee diagnostic.
- `CALLBACK-PRIVATE-MATERIALIZATION` — reported `fragment import lacks
  selected call custody` failure; the item says reproduce and repair.
- `NOMINAL-FIELD-FLOW` — receiver-specific default-domain exception;
  implementation cleanup.
- `CANONICALIZE-SCALAR-RANGE-CONTRACTS` — unfinished owner-mandated removal
  of bracketed range spellings.
- `CHECK-CLOSURE-ONCE` — `--check` compiles the bundled library twice.
- `C2L-PROOF-SEARCH-BLOWUP-CONTAINMENT` — compile-time proof blowup from
  repeated whole-module reconstruction.

## Repair: the pending-pin omission family (TASKS.md)

Measured silent omissions of valid programs, each pinned under
`tests/omega/pending/` and bound to a named sample. Defect-shaped (the surface
exists; admission declines or omits without diagnostic), not new-capability:

- `MEMBER-POLICY-REMOVES-COMPOSITE-ROWS` — a member-level arithmetic policy
  removes a Terminal row the same policy on the composite keeps.
- `REFERENCE-LOCAL-BINDING-ROUTES` — `&` bindings into `let` locals omit at
  any depth; `&mut` works only for a self-rooted direct name.
- `RECORD-LITERAL-ARRAY-FIELD` — array-typed record-literal fields have no
  route.
- `VALUE-CALL-SUBEXPRESSION` — value calls omit in any non-root expression
  position.
- `BYTE-CARRIER-STORE-LITERAL-ONLY` — owned `[u8; N]` field store admits only
  a bare string literal; anything else declines with no diagnostic.
- `COMPOSITE-TRANSITION-ARGUMENT` — composite transition arguments route only
  from locals.
- `VALUE-CALL-TRANSITION-ARGUMENT` — value calls omit as transition arguments.
- `BOUNDARY-PRIMITIVE-ARRAY-PARAMETER` — primitive fixed-array boundary
  parameters/results have no lowering route (3 GUI samples pinned).

## Repair (TASKS_BOOTSTRAP.md)

- `ALPHA-WINDOWS-DIV-TRAP` — Windows `div`/`mod` take hardware faults instead
  of the audited trap routine.

## Basis: corpus and release gates

- `CANARY-CORPUS` — the roster accounting is repair; the red cluster's causes
  belong to their named capability items. The wave's drain attributed the
  dominant corpus-rejection family (154 `ProgramEntry rejoins 0 attachment
  identities` — missing transitive Unit plan) to `GENERAL-CYCLIC-EXECUTION`,
  `ENTRY-CONTENT-ROOTS`, and `STATE-LOCAL-VALUE-FRONTIER`; those stay
  feature-classed, but their reds are the canary ledger's live load.
- `SAMPLE-CORPUS` — the sample refusal census (unit-plan omission, `Clock`
  provider) is the basis reading; fixing the underlying causes belongs to
  their owning items. 75/150 samples build natively after the review sweep;
  the remaining reds attribute to the pending-pin family above plus
  `MATCH-SELECTIVE-LOWERING` (float `==` position binding) and
  `selection_provenance`'s auto-select of uninstantiated generic candidates.
- `MATCH-SELECTIVE-LOWERING` — owns three measured `float_match_*`
  package-evidence reds (match-equality position binding for `Float::equal`).
- `RC-REPOSITORY-CLOSURE` — the wave's natural acceptance gate.
- `RC-BUILD-AND-PACKAGES`, `RC-PCC-REPLAY`, `RC-RELEASE-RECORD-AND-CLOSURE`
- `RC-NATIVE-MATRIX-LINUX-X86-64`, `RC-NATIVE-MATRIX-LINUX-ARM64`,
  `RC-NATIVE-MATRIX-MACOS-ARM64`, `RC-NATIVE-MATRIX-WINDOWS-X64`,
  `MACOS-X64-HOST-PROFILE` — matching-host evidence legs.
- `SYMBOLIC-MATERIALIZATION`

## Basis (TASKS_OPTIMIZER.md)

- `PIPELINE-OWNER-CONSOLIDATION` — connected-route conformance: no competing
  entrances or orphan outputs; structural integrity, classed basis.
- `WORKSPACE-ROLLOUT`, `TRANSLATION-VALIDATION`,
  `CUSTODY-MUTATION-COVERAGE`, `PER-RULE-COVERAGE` — coverage/validation
  surfaces; the remaining optimizer items are missing optimizations and are
  feature-classed for this partition.

## Swarm discipline for the wave

- Fewer lanes than a feature wave; repair and basis lanes hold claims on the
  shared paths they touch so a concurrent feature landing cannot silently
  revert the fix (the `912b490c8f` failure mode).
- **A slot idles when no program is claimable.** Refill order is: named
  residuals on the settled item, then unclaimed repair items, then
  unclaimed basis items — and if none claim cleanly, the slot stays empty.
- "Repaired" means the item's program runs at current HEAD, witnessed by
  its command. A red test that went green names what changed in the
  compiler to make it pass.
- Baseline rows in `wiki/drafts/measurements/known_baseline_failures.md` are
  the work queue; a row closes when its program runs, or when the harness
  that measured it is gone and the row is re-measured against what exists.
- All landings through `tools/landing.py`; check
  `python tools/claims.py status` before touching shared paths.
