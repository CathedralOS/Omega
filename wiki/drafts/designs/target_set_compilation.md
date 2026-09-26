# Target-set compilation

Plan for making target multiplicity data inside the compilation instead of
control flow around it. Companion to the **TARGET-SET-COMPILATION** board item.
Delete once that item closes: one compile admits a target set, the per-target
outer loops and post-hoc reuse structures are gone, and the spine reads in one
place.

Measured at `c5a798a411` (2026-09-24, Windows host).

## Finding

Two distinct architecture problems, both verified against the code:

**The pipeline spine is not stated anywhere.** For `omega <root.omg>` the stage
sequence exists only by traversal:

```
cli::compilation::compile_project_command
  → omega::compilation::compile_project          (workspace/occupancy, package fork)
  → compiler::compile                            (compiler.rs: per-target loop)
    → PreparedCheckedSource::check               (assembled-syntax-to-checked-compilation)
      → build_continuation::evaluate_build_and_continue
          phase_transitions: syntax→resolved→typed, build eval, generated-source re-entry
          typed→checked + settlement wedge, then settle_selected_execution
    → admit_checked_compilation / admit_requested_product
    → product: Check | TerminalArtifact (produce_terminal_report → terminal_production::produce)
                  | NativeArtifact (prepare_native_product → NativeInputReuse::realize
                      → realization::realize → realize_native_artifact → realize_image
                      → emit_realization_object → optimization/target/physical stage wrappers
                      → stage_optimized_verified_physical_pipeline
                      → emit_optimized_fragments → assemble_requested_native_artifact)
```

Nine sequencing functions across five crates, six different verbs
(check/produce/prepare/realize/lower/stage/emit) for one route. Work that is a
stage in all but name hides between the named stages: the settlement wedge
inside `phase_transitions::typed_trees_to_checked_trees`, all of
`settle_selected_execution`, and the checked+terminal→native-request derivation
in `native_product/realization.rs`.

**Multi-target compilation is repeated single-target compilation, at two
layers.** `compiler.rs` does
`for (target, source) in request.targets.into_iter().zip(repeat_n(source))`.
The only cross-target sharing is parsing: `repeat_n` clones the
`PreparedCheckedSource` checkpoint, which shares parsed arenas. Everything
after re-runs per target — and it must, because
`source_checkpoint.for_exact_target(name).assemble()` *filters* the source
graph: `linux_x86_64`-qualified declarations do not exist in another target's
`AssembledSyntax` at all (`source_assembly/checkpoint.rs::assemble`).
Per target: assemble → resolve → type → **build.omg executes** → generated-source
re-entry → typed→checked + settlement → `settle_selected_execution` → trust
admission → terminal production → native realization.

The batch surface (`CompileRequest::with_target_configurations`,
`ExplicitTargetSet`, `TargetCompileConfiguration`) has **no production
caller** — only tests. The real multi-target surfaces loop one layer up:
`packages/manager` operations iterate `for target in targets`
(`operations/inspect_packages/execution.rs`, with its own
`CandidateSourcePreparation`; `package_change` documents its unit as "one
exact target"). Three structures exist only to deduplicate work the model
duplicates: the `repeat_n` checkpoint clones, `NativeInputReuse`
(identity-keyed `PreparedNativeRealizationInput` — computing the
equivalence class *after* doing the work), and `CandidateSourcePreparation`.

## Model

Target multiplicity is data. The coordinator compiles a program for a target
*set*; each stage receives the whole set and internally partitions only where
meaning genuinely diverges:

```rust
let request = validate(request)?;
let checked = psi::compile(request.program, &request.targets)?;   // once
let terminal = psi::produce_terminal(&checked, &request.targets)?;
match request.product {
    Check      => report(terminal),
    Terminal   => publish(terminal),
    Native     => omega::realize(terminal, &request.targets),
}
```

Products remain physical stops (a Check run does not enter native legs) but
the *concept* is output selection, not pipeline truncation — the existing
`admit_requested_product` fences already enforce which evidence a stop can
satisfy.

Per-target outcomes stay data (`CompileTargetOutcome` per profile), not
control-flow isolation.

## What genuinely diverges per target

This is the load-bearing constraint: today the per-target program is a
*different program*, not the same program with a different flag.

- **Assembly filters.** `for_exact_target` removes other targets' declarations
  and resolves package imports per target; the entry-contract seed and build
  prelude are target-keyed. A set-valued assembly must retain *all* targets'
  qualified declarations simultaneously — same qualified name, different
  target qualifier, coexisting in one syntax. That is a namespace/resolution
  model change in `symbol-resolved-trees`, not a loop refactor.
- **Build evaluation observes the target.** `builder.roots.bind(
  linux_x86_64::ProgramEntry, ...)` and provider selections read it. One
  evaluation serving the set either makes the `Build` vocabulary set-aware or
  evaluates once per distinct build-relevant signature — an internal
  partition, allowed by the model, but its restricted-request consent and
  snapshot custody are per-activation today and must stay attributable.
- **Provider plans, entry binding, task plans, dispatch edits** are
  target-scoped facts settled during typed→checked and execution settlement.
  They become target-indexed rows on one checked program.
- **Terminal production is keyed to the selected program entry**, which is
  target-owned — `produce_terminal` partitions by (entry, target) internally;
  the semantic spine before entry selection is shared.
- **Native lowering is genuinely per-ISA** — instruction selection onward
  partitions by architecture. Identical partitions share work by construction,
  not by identity-keyed lookup.

## Phases

Each phase lands verified and independently; later phases need earlier ones.

**P0 — Name the spine (small).** Give the route one readable statement: the
native product leg's sequence (`prepare_native_product` →
`realization::realize` → `realize_image` → `emit_realization_object` →
`physical_pipeline` → `emit_optimized_fragments` → `assemble_*`) is refactored
so stage calls appear in order at one owner, with admission legs named as
fences between stages. Promote `settle_selected_execution` to a named
checked→settled stage. Wire `RequestedCompileProduct::TerminalArtifact` to the
CLI (`--psi`-style product → `publish_retained_terminal_artifact`, which
already writes `<root>.psi` + `.proof`; `publish_compilation` currently
rejects it as unpublishable). No behavior change; the diff is readability plus
one exposed product.

**P1 — Set-valued assembly.** `AssembledSyntax` carries the requested target
set; target-qualified declarations coexist with a target index instead of
being filtered out; package imports and entry-contract seeds resolve per
target into indexed rows. Proof: one `AssembledSyntax` serves a two-target
request; a target-qualified declaration is invisible to non-matching targets
*at consumption* (resolution produces per-target views), not absent.

**P2 — Set-valued checking.** Resolution/typing/checking run once over the
superset program; provider plans, entry bindings, task plans, dispatch edits
become `target → row` maps on `CheckedCompilation`. Build evaluation becomes
one activation producing target-indexed config, or partitions internally by
build-relevant signature — decide by whether `Build` vocabulary exposes the
target set; keep per-activation restricted-request consent and snapshot
custody attributable either way. `admit_checked_compilation` reconstructs
shared obligations once, target-indexed obligations per row.

**P3 — Terminal per set.** `produce_terminal` takes the set; shared lowering/
optimization/publication runs once, entry-keyed product rows partition per
(entry, target). `RetainedTerminalArtifact` gains a target dimension.

**P4 — Native cohort.** `realize` takes the cohort; terminal→abstract lowering
runs once per equivalence class (today: `NativeInputReuse` identity lookup —
dissolve it); abstract→target and everything physical partitions per target.
`NativeRealizationRequest` assembly moves to a named
terminal→native-request derivation.

**P5 — Converge the outer loops.** `packages/manager` multi-target operations
(`inspect_packages`, install/update candidate checks) call the set-valued
route once instead of `for target in targets`; `CandidateSourcePreparation`
and the `repeat_n` checkpoint cloning retire; `with_target_configurations`
either becomes the real surface or deletes.

## Invariants

- **Per-target failure isolation** is preserved as data: one target's rejection
  produces its `CompileTargetOutcome`; siblings still report.
- **Build-effect custody**: each build activation's snapshot/restricted-request
  accounting remains attributable per target even under a shared evaluation.
- **Psi↔Omega firewall unchanged**: target indexing lives inside
  representations; Omega still consumes only Terminal Psi.
- **Admission fences stay checks**: moving sequencing does not weaken or
  relocate trust settlement, product fences, or realization replays.

## Acceptance

- One invocation over `{linux_x86_64, windows_x86_64}` performs source
  loading, parsing, resolution, typing, and build evaluation once each
  (measurable via `--timings` legs or a test counting stage entries).
- A two-target package operation (e.g. `omega audit packages --target a
  --target b`) makes one compile call, not one per target.
- Per-target diagnostics and outcomes are unchanged in content and
  attribution; a one-target rejection does not suppress siblings.
- `NativeInputReuse`, `repeat_n` source cloning, and
  `CandidateSourcePreparation` are gone or reduced to trivial ownership
  transfer — identical work is never performed rather than deduplicated after.
- Single-target CLI behavior (`--check`, native, `run`, `inspect-terminal`)
  is byte-identical in observable output.
- The named spine exists: the stage order for each product is readable in one
  function per product, matching `omega-rust/pipeline.md`'s table.

## Open questions

- Whether `Build` vocabulary exposes the target set to the build machine
  (set-aware evaluation) or the stage evaluates once per build-relevant
  signature. Decide in P2 with a measured build-config divergence check.
- Whether generated-source re-entry (build-generated source re-entering
  resolution/typing) becomes set-valued at the extension or per target —
  interacts with P1's coexistence model.
- Interaction with **TWO-AXIS-TERMINAL-AUTHORITY-REVIEW** and
  **PIPELINE-OWNER-CONSOLIDATION** — both touch the same coordinator estate;
  sequence P0's spine naming so it does not fight the wrapper-object
  relocation.
