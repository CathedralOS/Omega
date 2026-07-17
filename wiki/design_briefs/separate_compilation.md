# Design Brief: Separate Compilation / Component Artifact Model

Scouted 2026-06-12. Status: AWAITING SIGN-OFF (decisions S1-S6 in TASKS.md).

## Current State: whole-program assumptions by layer

- Frame storage: one global blob, absolute offsets at plan time (HIGH cost
  to relax — needs base-register discipline or relocatable descriptors).
- Dispatch loop: single fused loop, dense global case indices (HIGH — a
  swapped component cannot add cases without indirection).
- Per-call monomorphization spans the whole program (MEDIUM — cross-
  component calls need stable symbolic names, not clones).
- Data/relocations: absolute, computed against the final image; no .o
  format exists (HIGH).
- Symbol identity: arena indices, not stable across recompiles (HIGH —
  exports need symbolic names).
- Entry/boundary registry resolved once (MEDIUM — needs per-component
  manifests).

## Recommendations

1. **Component boundary = PACKAGE** (chapter 15 unit); machines stay swap
   points within any component (deployment unit ≠ swap unit). Artifact =
   sealed IR + boundary manifest + public layout/protocol reports first;
   relocatable `.o` as follow-up.
2. **Linking**: hermetic static composition phase first (a build step
   consuming component artifacts, ordering by dependency DAG); loader-time
   relocation later when Cathedral's loader design crystallizes.
3. **Cross-component calls**: hybrid — indirect calls through an import
   table for plain calls; synthesized boundary-stub machines where the
   state graph must hand off. Keep ONE fused dispatch loop with
   per-component entries (don't split it).
4. **Cross-component ABI**: hybrid — compiler-ENFORCED public layout
   reports (content-addressed, layout-violating change = build error) for
   performance-critical edges; explicit protocol schema/codec contracts for
   evolution-safe edges. Host ABI reused as the calling convention (zero new
   work).
5. **Monomorphization across packages: REJECT in stage 1** (report it),
   resolve at composition time in stage 2.
6. **Omega owns the composition (linker) tool** — linkers are language
   infrastructure; Cathedral consumes it.

## Staging

1. **Discipline + reports (~no codegen)**: per-package boundary manifests,
   public-API/layout/wire exports, reject cross-package monomorphization,
   flag absolute-relocation emitters. Packages become syntactically
   isolated; "what a loader needs" becomes a report.
2. **Object format + static linking**: per-package relocatable artifacts,
   symbolic exports (module paths), composition tool, per-component frame
   regions (base-register discipline), composition-time monomorphization.
3. **Loader integration (Cathedral scope)**: load-time relocation patching,
   provider admission, quiescence, explicit state-migration machines, atomic
   installation, and rollback. The orchestration is a Cathedral proving slice,
   not an Omega `replace` DSL.

## Cross-references

wiki/architecture/whole_program_assumptions.md, cathedral_alignment item 4,
Cathedral component_model/hot_swap docs, chapters 15 + 21.
