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
- [!] **the other 3 place resolvers (`resolve_runtime_storage_place`,
  `…_primitive_type`, `…_fixed_indexed_place`) are BLOCKED — left non-table.** They
  diverge from their `_in_table` siblings on `array[const].field`
  (`rooms[0].cell`): the non-table normalizer folds the index into the segment name
  (`"rooms[0]"`) and fails to match the slot named `rooms` → None; the `_in_table`
  form keeps the base name and tracks the index via `member_index`. Delegating them is
  NOT behavior-preserving. Worse, an experiment delegating all 10 showed the
  `_in_table` form is *also* wrong for index>0 (`cells[1].value` reads element 0 —
  the constant array index is never applied to the byte offset). So this is one
  underlying correctness bug — the **same root** as the existing pending canary
  `control_flow/fixed_array_element_guard` ("when guard operand resolution applies the
  index, this will compile") — and it underlies the dungeon `find_room` room lookup.
  See [[nontable-array-const-field-gap]]. It must be FIXED (apply index*elem to the
  offset, consistently in read/write/guard) before these 3 — and the value/guard
  resolvers that bottom out in them — can be collapsed. Not a refactor; a correctness
  task gated on Phase 5.
- [!] **value-operand + guard resolvers — NOT clean duplication, deferred.** Two
  reasons beyond the array-index gap: (a) `writes/mutation/value_operands.rs`
  non-table takes `aliases`+`alias_expressions` and does alias resolution the
  `_in_table` form does not — collapsing would drop it; (b) these return **arena
  handles** (insert into `runtime_value_operands`), so the `{:?}`-diff probe can't
  compare them without dereffing + arena side-effects. The `guards.rs`
  `resolve_runtime_value_operand` is a pure dispatcher whose ONLY divergence from its
  table form is the final `resolve_runtime_storage_place` call — i.e. it too is gated
  on the same array-index gap. So the whole value/guard collapse reduces to fixing
  that one resolver; do it after the gap fix, then delegate.
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

## Done-when

- No value-operand / operation-kind type is declared more than once.
- No pipeline stage that only annotates produces a whole new representation.
- Selection has one resolver per concept (place / value / scalar-type), not a
  table×non-table×per-shape matrix.
- The suite stays green throughout; every phase is its own commit.
