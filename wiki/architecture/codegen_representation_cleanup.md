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
One `resolve_place` / `resolve_value_operand` / `classify_scalar` that handles both
the table-handle and resolved-`Expression` forms, and every target shape, in one
place. Route all write/guard/convert/branch/straight-line producers through it;
remove the duplicated per-shape, per-context selectors. (Phase started: ef680466
added the scalar classifier funnel.)
- [ ] unify the place resolver (table + non-table + per-shape).
- [ ] unify the value-operand resolver.
- [ ] route all producers; delete the duplicates.
- [ ] suite green; commit.

### Phase 5 — Deeper representation redesigns (separate axis; schedule after 1–4)
Beyond type-dedup; these are correctness/representation rewrites.
- [ ] **Width-as-layout.** Replace the hand-maintained per-instruction width
  functions (which must exactly match the emitters or relocations silently drift →
  runtime segfault) with a symbolic-emit + single layout/relocation pass.
- [ ] **Frame model.** A real call stack (or provably-disjoint frame stacking) for
  dispatched self-looping callees, replacing the fixed-data-address overlapping
  frames that corrupt threaded args (root cause of the dungeon `find_room` bug and
  the post-dispatch continuation bugs).

## Done-when

- No value-operand / operation-kind type is declared more than once.
- No pipeline stage that only annotates produces a whole new representation.
- Selection has one resolver per concept (place / value / scalar-type), not a
  table×non-table×per-shape matrix.
- The suite stays green throughout; every phase is its own commit.
