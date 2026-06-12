# Codegen Representation Cleanup

A standing plan to bring the backend representation / pipeline / selection layers
in line with the [Architecture Rule](architecture.md): *share the same semantic
concepts across stages instead of re-declaring them, and don't model an
annotation-only stage as a whole new representation.*

Execute phase by phase. Each phase keeps the canary suite green and lands as its
own commit. Tick the boxes as phases complete.

## Diagnosis (the evidence this plan removes)

- The value-operand enum was re-declared **identically in 3 crates**
  (abstract/target/assigned) + 2 hand-written conversions. FIXED 2b1397d3 — one
  shared `ValueOperand`, −265 lines.
- The **operation-kind enums** are the same story: target == assigned
  (63 == 63 variants identical); abstract differs by only ~3 host-lowering
  variants.
- **`target → assigned` does no transform.** It copies operations verbatim
  (`operation.kind.clone().into()`) and attaches a `home` (register/scratch slot).
  It is an *annotation* stage masquerading as a representation.
- Rep crates **do not composite** (zero inter-rep deps). The "input+output rep per
  pipeline phase" ideal is already broken: `target→assigned` pulls in **3** reps
  (reaches back to abstract), `control-flow→abstract` pulls in **5** — because they
  cannot *share* types, only re-declare or reach back.
- Selection re-implements every resolver/selector in `_in_table` **and** non-table
  forms, times per-target-shape (frame-slot / field / pointee / indexed) — so one
  logical decision (is-float? byte-size? place?) lives in ~6 drifting copies.

## Phases

### Phase 1 — Share the operation-kind enum (target ⟷ assigned) — DONE (cbf75337)
`AssignedOperationKind` is an alias of `omega_target_operations::TargetOperationKind`
(domain too); the `semantic_domain` classification is inherited from the impl on the
shared type (trait + domain enum already in shared `omega-core`). The two ~700-line
reflexive `From` conversion files + the duplicate classification + tests are gone.
- [x] target canonical; assigned alias; drop the `From`s. Net −2076 lines.

### Phase 2 — Remove the rest of the assigned type duplication — DONE (7b0ac210)
`AssignedOperation`, `AssignedInstructionOperand(/Kind)`, and
`AssignedTargetOperationFunction` were verbatim target copies; now aliases. Their
duplicate defs, `Default`s, the `InstructionOperandLike` impl, and the operand
`From`s are gone. **The assigned crate now declares only what it actually adds:**
register/scratch `homes` (homes.rs) + the `AssignedValueOperand { kind, home }`
wrapper. The abstract/target/assigned representation triplication is eliminated.
- [x] alias the remaining wrapper structs; suite green.
- [ ] (optional Phase 2b) stop COPYING the target arenas into the assigned plan
  — make the assigned plan = target plan + the homes side-table (no clone). This
  is the perf/structure optimization; the dedup itself is already done.

### Phase 3 — Composite reps from a shared vocabulary crate
Extract the cross-stage vocabulary (`ValueOperand`, the shared operation-kind,
`RuntimeStorageRegion`, `StateGuardOperator` / `StateGuardLowering`) into one low
crate that the rep crates *composite* from. "Reaching back to abstract" becomes
"both composite the vocab crate." Transform stages then convert only the sub-part
they actually change.
- [ ] vocab crate; abstract/target/assigned composite it (no re-declaration).
- [ ] suite green; commit.

### Phase 4 — Selection funnel: collapse `_in_table` / non-table
NOTE (investigated): this is **not** mechanical duplication like Phases 0–2. The
`_in_table` resolvers take checked-tree `ExpressionHandle`+`ExpressionTable`; the
non-table siblings take owned `&Expression` trees produced by **alias/binding
substitution**. The call pattern is "try the table path; if it returns false,
`expressions.to_tree(handle)` to OWNED trees and fall back to the non-table path"
(e.g. branches/leaf.rs ~496–529; same in prelude.rs / straight_line.rs). So the
non-table path is a more-complete **fallback** over a different expression
representation, ~40 paired functions deep. Collapsing it is an INCOMPLETE-MIGRATION
problem, not a rename: porting the non-table-only resolution into the table path,
case by case, until the fallback is dead, then deleting it. The scalar classifier
(ef680466) and the value operand already funnel; the place/guard resolvers are the
bulk.

Two viable strategies (pick per-resolver):
- **Complete the migration**: make `*_in_table` handle every case the non-table
  sibling does (verify the fallback stops firing — instrument it), then delete the
  non-table function. Lowest conceptual surface, but per-case porting.
- **Generic `ExpressionSource` trait**: one body per resolver, generic over a trait
  with the few accessors the resolvers need (normalized name path, binary/cast/
  indexed operands yielding sub-sources), impl'd for both `&Expression` and
  `(&ExpressionTable, ExpressionHandle)`. Removes the fallback entirely. Bigger
  up-front design (recursion yields sub-`impl ExpressionSource`), but no per-case
  porting and no behavior risk.

UPDATE (investigated deeper): the table vs non-table resolvers are **same logic,
different access** — identical structure (fixed-indexed → normalize name path →
find slot → nested layout → machine-owned fallback), differing only in
`normalized_storage_expression(&Expression)`+`NamePath` vs
`normalized_storage_name_path_in_table(table,handle)`+`StorageNamePath`. So this is
NOT a divergent migration — the fallback exists only because alias-resolved trees
are owned, not handles.

`ExpressionTable::insert_tree(&Expression) -> ExpressionHandle` exists (the inverse
of `to_tree`). So the simplest collapse is: each non-table resolver becomes a thin
adapter — `insert_tree` the owned expression into a scratch table, then delegate to
`*_in_table`. One implementation; the non-table bodies disappear.

CAVEAT before doing it: the non-table path has TWO callers — (a) the leaf
**fallback** (`to_tree` + non-table at leaf.rs ~514) and (b) genuine **alias-resolved
branch-arm** expressions. For (a), if the resolvers are truly same-logic the
fallback is already dead (the table path at ~496 would have succeeded); for (b) it
is load-bearing. So: first instrument the non-table path to confirm whether it ever
emits where the table path didn't (run the full suite + dungeon). If it never does,
delete the leaf `to_tree` fallback outright; either way, convert the remaining
non-table resolvers to `insert_tree`+delegate so the logic lives once.

RESULT (first cut DONE): the non-table **mutation-write** path was instrumented
exactly as planned — a `selected_instructions.len()` before/after probe around
`select_runtime_resolved_mutation_write`. Across the full canary suite (133) it was
reached 0 times; in the dungeon stress sample it was reached **470 times and emitted
0 instructions**. It is a *dead emitter*: the `_in_table` write + text-builder paths
handle every case that lowers; the non-table fallback always bailed without pushing.
So the migration was already **complete** — `insert_tree`+delegate was unnecessary
(it would have routed to a table path that already returns false for these cases).
Deleted outright: the ~250-line `select_runtime_resolved_mutation_write` /
`_impl` pair, all 6 fallback call sites (leaf / prelude / straight_line ×3 /
state_bodies), and the cascade of non-table-only helpers it uniquely fed
(`select_runtime_resolved_binary_mutation_write`, the local non-table
`resolve_runtime_value_operand`, the non-table place-resolver imports, dead local
`state_names`/`source_machine_name`). Net **−489 lines**. **Proof of safety:** the
dungeon PE is **byte-identical** before/after (sha256 `ee2f534f…`), suite 133 green.

This validates the hypothesis crate-wide: every non-table caller `to_tree`s a handle
first, so the non-table path is *always* a fallback after a table attempt — never a
primary path. The remaining non-table resolver families (place / guard / value) are
very likely the same dead-emitter shape. NEXT: probe each the same way (len-delta
around the non-table resolver) before deleting; if any *does* emit, that case is a
real table-path gap to port first.
- [x] **branch mutation-write family — probed dead, deleted (−489 lines, dungeon byte-identical).**
- [~] **state-body mutation-write (`writes/mod.rs` → `mutation::select_runtime_mutation_writes`)
  — probed PARTIALLY live.** Same fallback shape (table-first at
  `select_runtime_storage_resolved_mutation_write_in_table_with_scratch`, then
  `to_tree`+non-table). Suite reaches it 0×; the dungeon reaches it and emits **exactly
  1 instruction**. So unlike the branch family it is NOT a free delete — there is ONE
  real case the `_in_table` storage path doesn't handle. NEXT here is the
  *port-then-delete* strategy: instrument to print the emitting mutation's
  target/value/statement (richer probe at the `writes/mod.rs` call site), find why
  `select_runtime_storage_resolved_mutation_write_in_table_with_scratch` returns false
  for it, port that one case into the table path, re-probe to 0, then delete the
  non-table writer + its fallback as before. `insert_tree`+delegate is NOT safe here
  (the table path already returns false for this case, so delegating would drop the
  emission) — the gap must be closed in the table path first.

  IDENTIFIED (richer probe): the one emitting case is `MazeBuilder::carve_room` stmt 6
  — `room.description = self.room_description(depth, branch_id)`, a `Member` target
  (String field) = `Call` value (value-position stateful method call result). This is
  the SAME construct as the known-broken `room_description` 5-arm dispatch-
  specialization (see [[multi-arm-branching-value-middle-arm]] /
  [[runtime-conditional-value-primitive]]): the table storage-write path has no
  call-result-to-String-field rule, so it falls back. So this collapse is **entangled
  with the call/dispatch lowering**, not a clean Phase-4 port — defer it behind that
  work (Phase 5 frame/dispatch). The branch-family deletion already captured the bulk
  of the mutation duplication; the place/value/guard resolver pairs are the better
  next Phase-4 target.

  NOT-THE-CULPRIT note (2026-06-12): the dungeon's empty side-room descriptions
  (R05/R06) were long suspected to be this gap, but the hunt proved this fallback
  emits correctly in every carve dispatch (main hall AND side room). The real bug
  was upstream — `should_carve`'s nested `chance` leaf value (`roll < numerator`)
  lost its call-result write because `numerator` bound to a caller-local with no
  frame slot, so the side-room transitions never fired at all (fixed in
  branches/leaf.rs `resolve_leaf_caller_local_initializer_names`; canary
  dungeon/runtime_nested_value_call_caller_local_guard_exit). This item remains a
  representation-cleanup deferral only, not a live correctness bug.
- [x] **place-resolver family — collapsed 7 of 10 (aaa24483, −319 lines).** Each
  non-table place resolver becomes `insert_tree`+delegate to its `_in_table` sibling
  (`insert_tree` is a faithful inverse of `to_tree` — preserves every symbol handle).
  Equivalence was PROVEN per-resolver with a **differential probe** (compute orig and
  delegated, `format!("{:?}",..)`-compare, log mismatches) run across the full suite
  AND the dungeon. 7 showed zero mismatches in both → delegated (frame_indexed,
  frame_base_indexed, machine_indexed, pointee_slot_offset, pointee_fixed_indexed,
  frame_fixed_indexed, fixed_array_length). Dungeon byte-identical, suite 133 green.
  LESSON: the suite caught a divergence (`fixed_indexed_place`, 60×) the dungeon did
  NOT — always probe BOTH.
- [x] **the remaining 3 place resolvers (`resolve_runtime_storage_place`,
  `…_primitive_type`, `…_fixed_indexed_place`) — UNBLOCKED and collapsed.** They were
  blocked on the `array[const].field` index bug (the `_in_table` form never applied
  index>0 to the byte offset; the non-table normalizer folded `rooms[0]` into the
  segment name and resolved nothing). 97d70a9d fixed the index family (descriptor-aware
  indexed-copy argument strategy + resolver refusal for a root index over a slice
  descriptor; `fixed_array_element_guard` promoted to pass/). RE-PROBED post-fix with
  the standard differential probe (orig vs insert_tree+delegate, `{:?}`-compare) across
  the full suite (193) AND the dungeon: **zero divergent resolutions** — every mismatch
  was strictly `orig=None → delegated=Some`, i.e. the `_in_table` form is now a proven
  strict superset. The extra resolutions are exactly (a) the fixed `array[const].field`
  family with the index correctly applied (`items[1].value` → offset 8, elem 8; frame
  and machine regions), and (b) `fixed_array.as_slice()[const]` (the table normalizer
  sees through the slice view; the non-table `fixed_indexed_target_path` never matched
  `Call`), and for `…_primitive_type` (c) nested member paths (`self.count`) the
  non-table "first cut" only resolved single-segment. Collapsed all 3 to
  `insert_tree`+delegate (`…_fixed_indexed_place` deleted outright — only caller was
  `resolve_runtime_storage_place`), harvesting the dead non-table helpers
  (`slot_matches_path`, `runtime_frame_slot_for_expression`,
  `fixed_indexed_target_path`, `resolve_indexed_target_suffix_layout/_cursor`,
  `FixedIndexedTargetPath`, `IndexedTargetPath`). **Proof:** dungeon PE byte-identical
  before/after (sha256 `b4fac566…`) — the strictly-more-resolving fallback changed no
  emission — and suite 193/193 green both ways.
- [x] **guards.rs `resolve_runtime_value_operand` — collapsed with them.** Probed with
  a content-rendering differential (recursive `Binary`/`Convert` deref so two arena
  handles compare by value): its only divergence was the same index family via the
  final `resolve_runtime_storage_place` call, exactly as predicted. Now
  `insert_tree`+delegate to `resolve_runtime_value_operand_in_table`; covered by the
  same byte-identical dungeon + green suite.
- [!] **`writes/mutation/value_operands.rs` non-table — still deferred.** It takes
  `aliases`+`alias_expressions` and does alias resolution its `_in_table` form does
  not; collapsing would drop that. Separate port (alias resolution into the table
  form, or callers pre-substitute) before it can funnel.
- [x] suite green per family; commit each (place family committed aaa24483).

### Phase 5 — Deeper representation redesigns (separate axis; schedule after 1–4)
Beyond type-dedup; these are correctness/representation rewrites.
- [ ] **Width-as-layout.** Replace the hand-maintained per-instruction width
  functions (which must exactly match the emitters or relocations silently drift →
  runtime segfault) with a symbolic-emit + single layout/relocation pass.
- [ ] **Frame model.** A real call stack (or provably-disjoint frame stacking) for
  dispatched self-looping callees, replacing the fixed-data-address overlapping
  frames that can corrupt threaded args (the post-dispatch continuation bugs).
  Attribution correction (2026-06-11): the dungeon `find_room`/hot-potato segfault
  on arm64 was NOT frame overlap — it was the aarch64 encoder using x18 (Darwin's
  kernel-zeroed platform register) as the slot-copy scratch, so threaded `&mut`
  args lost their pointer whenever a timer interrupt split a copy pair. With x18
  replaced by x26 the dungeon's per-call-context frame slots (verified via
  slots.txt: each call context gets disjoint ranges) run the full scripted loop
  correctly, so the dungeon no longer evidences a frame-overlap bug; this redesign
  remains motivated by self-looping callees and separate compilation.
  Attribution correction (2026-06-11, second instance): the shrinking-slice
  threaded-scalar accumulation bug (`self.accumulate(items[1..], items[0].value)`
  natively totaled 4*ptr+24 instead of 70) was ALSO not the shared-frame model —
  the same-context overlap staging (source -> scratch -> target) is sound. The
  argument lowering resolved `items[0].value` as a plain place over the slice
  descriptor slot (root index dropped, no deref), reading the data pointer's low
  bytes as the element; fixed in selection (descriptor-aware fixed-indexed copy +
  resolver refusal), pinned by canary
  `termination/runtime_shrinking_slice_recursion_exit`. The overlapping-frames
  family has so far produced miscompiles whose roots were elsewhere; the redesign
  case still rests on self-looping callees and separate compilation, not on
  observed frame corruption.

## Done-when

- No value-operand / operation-kind type is declared more than once.
- No pipeline stage that only annotates produces a whole new representation.
- Selection has one resolver per concept (place / value / scalar-type), not a
  table×non-table×per-shape matrix.
- The suite stays green throughout; every phase is its own commit.
