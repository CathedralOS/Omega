# Repository Layout

This page is a map and a placement guide. It should help answer two questions:

- Where should a new crate or module live?
- Which layer is allowed to own a concept?

The pipeline-specific semantic rules live in
[Pipeline Architecture](pipeline/pipeline.md).

How Omega builds *itself* — the trust architecture and the
`alpha`/`beta`/`gamma`/… rung chain under `compiler/` (distinct from the
`omega-rs` crate map below) — lives in
[The Bootstrap Lattice](bootstrap_lattice/bootstrap_lattice.md).

## Design Bias

- Prefer feature-first crates with explicit names.
- Keep durable IR structs in `representations/`.
- Keep transforms in `pipeline/`.
- Keep language meaning in `semantics/`.
- Keep target, ABI, layout, object, linker, and image details in `backend/`.
- Keep orchestration boring: sequence phases, write artifacts, do not absorb phase logic.
- Do not add a crate until a module boundary has stopped moving.

Legend:

- `[CRATE]` means a Cargo workspace package.
- Unprefixed folders are ordinary source/module boundaries inside a crate.

This tree reflects the actual Cargo workspace members. Some sub-areas named in
the placement prose are not yet separate crates: `packages/`, `runtime/startup`,
`tool_support/`, and several backend object/linker/image writers (per-format
object-file writers, the linker crates, and Wasm/RISC-V ISA crates) are still
folded into other crates or do not exist yet. They are placement intent, not
current packages.

```text
Omega/
|-- Cargo.toml
|-- README.md
|-- apps/
|   `-- [CRATE] omega-cli/                              # User-facing `omega` command.
|
|-- compiler/
|   |-- foundation/
|   |   `-- [CRATE] omega-core/                         # Shared primitives, ids, arenas, handles, spans, diagnostics.
|   |
|   |-- semantics/
|   |   |-- [CRATE] omega-types/                        # Type checking, inference, coercions, layout preconditions.
|   |   |-- [CRATE] omega-graph/                        # Machine/state graph construction and graph-facing semantic facts.
|   |   |-- [CRATE] omega-validation/                   # Cross-semantic program validation and diagnostics.
|   |   `-- [CRATE] omega-proof/                        # Proof obligations, invariants, liveness hooks.
|   |
|   |-- representations/
|   |   |-- [CRATE] omega-tokens/                       # Per-file token streams and token-set ownership.
|   |   |-- [CRATE] omega-syntax-trees/                 # Parsed source structure before names and symbols are resolved.
|   |   |-- [CRATE] omega-symbol-resolved-trees/        # SymbolResolvedTrees: syntax shape with declaration/reference symbols resolved.
|   |   |-- [CRATE] omega-typed-trees/                  # Symbol-resolved trees with type/effect information attached.
|   |   |-- [CRATE] omega-facts/                        # Checked semantic facts, invariants, and refinement data embedded in later IRs.
|   |   |-- [CRATE] omega-effects/                      # Effect-set, capability, and provider data shapes embedded in later IRs.
|   |   |-- [CRATE] omega-checked-trees/                # Typed trees plus checked semantic facts after validation/proof-facing checks.
|   |   |-- [CRATE] omega-state-graph/                  # Explicit machine/state graph for proof and scheduling.
|   |   |-- [CRATE] omega-control-flow/                 # Control-flow/data-flow graph.
|   |   |-- [CRATE] omega-abstract-operations/          # Target-independent abstract operations with virtual registers.
|   |   |-- [CRATE] omega-target-operations/            # Target-aware operations after legalization and selection.
|   |   |-- [CRATE] omega-assigned-target-operations/   # Target-aware operations after register and stack assignment.
|   |   |-- [CRATE] omega-machine-instructions/         # Final symbolic machine instructions before encoding.
|   |   |-- [CRATE] omega-machine-program/              # Machine-program artifact assembled from target operations.
|   |   |-- [CRATE] omega-machine-bytes/                # Encoded machine bytes.
|   |   `-- [CRATE] omega-backend-plan/                 # Backend planning data shared across lowering stages.
|   |
|   |-- pipeline/
|   |   |-- [CRATE] omega-source-files-to-tokens/                            # Source files to per-file token streams.
|   |   |-- [CRATE] omega-tokens-to-syntax-trees/                            # Token streams to parsed syntax trees.
|   |   |-- [CRATE] omega-syntax-trees-to-symbol-resolved-trees/             # Syntax trees to SymbolResolvedTrees with symbol identity attached.
|   |   |-- [CRATE] omega-symbol-resolved-trees-to-typed-trees/              # SymbolResolvedTrees to typed/effect-annotated trees.
|   |   |-- [CRATE] omega-typed-trees-to-checked-trees/                      # Typed trees to validated/proof-checked trees with semantic facts.
|   |   |-- [CRATE] omega-checked-trees-to-state-graph/                      # Checked trees to explicit machine/state graph.
|   |   |-- [CRATE] omega-state-graph-to-control-flow/                       # State graph to control-flow/data-flow graph.
|   |   |-- [CRATE] omega-control-flow-to-abstract-operations/               # Lower control flow into target-independent abstract operations with virtual registers.
|   |   |-- [CRATE] omega-abstract-operations-to-target-operations/          # Normalize, legalize, and lower abstract operations into target-aware forms.
|   |   |-- [CRATE] omega-target-operations-to-assigned-target-operations/   # Assign registers, stack slots, and calling-convention homes to target-aware operations.
|   |   |-- [CRATE] omega-assigned-target-operations-to-machine-instructions/ # Convert assigned target-aware operations into final symbolic machine instructions.
|   |   `-- [CRATE] omega-target-operations-to-machine-program/              # Assemble target operations into a machine-program artifact.
|   |
|   |-- backend/
|   |   |-- [CRATE] omega-target/                       # Target triples, cpu/features, os/env/object format matrix.
|   |   |-- [CRATE] omega-runtime-abi/                  # Backend-ABI carrier shapes (fat descriptors, slices, text windows) and their accessors.
|   |   |-- [CRATE] omega-data-planning/                # Data/section planning for emitted program data.
|   |   |-- [CRATE] omega-platform-interface/           # ABI-facing OS/platform imports, host surfaces, loader facts.
|   |   |-- [CRATE] omega-state-calls/                  # State-machine call lowering surface.
|   |   |-- [CRATE] omega-state-storage/               # State storage layout and slot lowering.
|   |   |-- [CRATE] omega-state-values/               # State value lowering and materialization.
|   |   |-- [CRATE] omega-state-schedule/             # State scheduling into backend form.
|   |   |-- [CRATE] omega-state-dispatch/             # State dispatch lowering.
|   |   |-- [CRATE] omega-state-guards/               # State guard/condition lowering.
|   |   |-- [CRATE] omega-runtime-text/               # Runtime text-window carrier lowering.
|   |   |-- [CRATE] omega-runtime-bodies/             # Runtime body/state-body lowering.
|   |   |-- [CRATE] omega-runtime-storage/            # Runtime storage surfaces.
|   |   |-- [CRATE] omega-runtime-branching/          # Runtime branch lowering.
|   |   |-- [CRATE] omega-runtime-dispatch-loop/      # Runtime dispatch-loop lowering.
|   |   |-- [CRATE] omega-calling-conventions/        # ABI rules for registers, stack, parameter/return passing.
|   |   |-- [CRATE] omega-layout/                     # Type layout, alignments, field offsets, calling-convention records.
|   |   |-- [CRATE] omega-instruction-selection/      # Shared instruction selection framework.
|   |   |-- [CRATE] omega-emission-planning/          # Section/symbol plans before machine emission.
|   |   |-- [CRATE] omega-machine-emission/           # Final machine program to encoded bytes.
|   |   |-- [CRATE] omega-backend-report/             # Backend summaries and reports.
|   |   |
|   |   |-- instruction_set_architectures/
|   |   |   |-- [CRATE] omega-isa-aarch64/            # AArch64 instruction defs, encodings, lowering hooks.
|   |   |   `-- [CRATE] omega-isa-x86_64/             # x86_64 instruction defs, encodings, lowering hooks.
|   |   |
|   |   |-- object/
|   |   |   |-- [CRATE] omega-object-file/            # Shared object-file representation: sections, symbols, relocations.
|   |   |   |-- [CRATE] omega-object-file-planning/   # Builds section and symbol plans before object-file or image writing.
|   |   |   `-- [CRATE] omega-relocations/            # Builds relocation records over selected and machine instructions.
|   |   |
|   |   `-- images/
|   |       |-- [CRATE] omega-image/                  # Shared final image data model and fixup helpers.
|   |       |-- [CRATE] omega-image-emission/         # Selects the final executable image writer for a target.
|   |       |-- [CRATE] omega-image-elf/              # Final ELF image layout, program headers, loaders.
|   |       |-- [CRATE] omega-image-macho/            # Final Mach-O image layout, load commands, fixups.
|   |       `-- [CRATE] omega-image-pe/               # Final PE image layout, directories, imports, relocations.
|   |
|   `-- orchestration/
|       |-- [CRATE] omega-artifacts/                  # Phase artifact data and text/binary dumping.
|       |-- [CRATE] omega-backend-pipeline/           # Backend phase sequencing at the orchestration edge.
|       |-- [CRATE] omega-compiler/                   # Top-level check/build API used by cli/tests.
|       `-- [CRATE] omega-visualizations/             # Visualization/dump views of pipeline artifacts.
|
|-- omega/
|   |-- language/
|   |   |-- core/                                       # Always-available language package.
|   |   `-- std/                                        # Higher-level standard package surface.
|   |
|   `-- host/                                           # Cross-platform boundary, authority, and per-target host surfaces.
|
|-- target_runtime/                                     # Linkable runtime payloads shipped with the toolchain.
|   |-- shared/                                         # Target-independent runtime manifests and metadata schemas.
|   |
|   `-- targets/
|       |-- macos_arm64/
|       |   |-- startup_objects/                        # Entry bridges such as `_start` or platform CRT replacements.
|       |   `-- platform/                               # Link-time host adapters and image metadata.
|       |-- linux_x64/
|       |   |-- startup_objects/
|       |   `-- platform/
|       |-- windows_x64/
|       |   |-- startup_objects/
|       |   `-- platform/
|       `-- wasm32/
|           |-- startup_objects/
|           `-- platform/
|
|-- samples/
|   |-- cli_mvp/                                        # Smallest console program.
|   |-- dungeon_crawler_cli/                            # Console input/output and room navigation pressure test.
|   `-- README.md                                       # Notes for sample expectations and local build output.
|
|-- canaries/
|   |-- pass/                                           # Tiny feature canaries expected to check.
|   `-- fail/                                           # Tiny negative canaries with expected diagnostics.
|
|-- tests/
|   |-- integration/                                    # End-to-end compiler tests.
|   |-- target_corpus/                                  # Per-target calling convention, ABI, object, link, and image tests.
|   `-- bootstrap/                                      # Future self-hosting/bootstrap tests.
|
`-- wiki/                                               # Language design notes, target notes, and guide drafts.
```

## Placement Rules

### Front Door

- `apps/` crates stay thin. They parse user intent and call compiler services.
  Today `apps/` holds only `omega-cli`; the language-server and docs-generator
  apps are not yet separate crates.
- `orchestration/` sequences phases, owns artifacts and the top-level
  check/build API (`omega-compiler`, `omega-backend-pipeline`, `omega-artifacts`,
  `omega-visualizations`), and keeps source loading coherent. Session/options and
  incremental-query engines are not yet separate crates.
- `orchestration/` must not become the home for semantic checks or backend
  lowering.

### Source And Packages

- `foundation/` stays dependency-light. If it needs semantic or target details,
  it is in the wrong layer. It is currently a single crate, `omega-core`, holding
  shared primitives, ids, arenas, handles, spans, and diagnostics.
- Source-preserving syntax data belongs in `representations/`; source-to-syntax
  transforms belong in `pipeline/`.
- Package manifests, package graphs, and loading are placement intent for a
  future `packages/` layer; there are no `packages/` crates today, and that work
  must not absorb language semantics when it lands.
- Source discovery/loading should remain an orchestration subsystem, not parser
  responsibility.

### Semantic Ownership

- `semantics/` owns language meaning: names, types, effects, proofs, facts,
  invariants, and validation. It has been consolidated: there are no separate
  `omega-borrow`, `omega-invariants`, `omega-contracts`, or `omega-consteval`
  crates today. Borrow, invariant, contract, and const-evaluation reasoning live
  inside the existing semantic crates (chiefly `omega-types`, `omega-facts`,
  `omega-validation`, and `omega-proof`).
- `omega-facts` and `omega-effects` are data-shape crates and live under
  `representations/` so checked IRs can embed their types without a
  representations-to-semantics edge; semantics crates still own how those
  facts and effects are established.
- `omega-facts` carries checked facts, invariants, and refinement data: what
  remains true.
- `omega-validation` answers cross-semantic obligations, including who may read
  or mutate and what a callable requires or promises.
- `omega-proof` discharges obligations.
- `omega-graph` stays language/proof-facing; do not bury state-machine reasoning
  in backend crates.

### Representations And Pipeline

- `representations/` owns durable IR data structures and arena storage.
- `pipeline/` crates transform one representation into the next.
- Pipeline crates may depend on input and output representations, but should not
  become owners of shared helper structures.
- Long-lived representation boundaries should remain explicit: tokens, syntax,
  symbol-resolved, typed, checked, state graph, control flow, abstract
  operations, target operations, assigned target operations, machine
  instructions, machine program, and machine bytes.
- Do not skip from source-shaped trees to backend-specific structs.

### Backend

- `omega-control-flow-to-abstract-operations` is where backend lowering begins;
  it should produce target-independent operations with explicit values and
  effects.
- `omega-abstract-operations-to-target-operations` owns target legalization and
  instruction-shape lowering.
- `omega-target-operations-to-assigned-target-operations` owns register
  allocation, stack slots, spills, and calling-convention homes.
- `omega-assigned-target-operations-to-machine-instructions` owns symbolic
  machine instruction construction.
- `omega-target-operations-to-machine-program` assembles target operations into
  the machine-program artifact; `omega-machine-emission` produces encoded
  machine bytes, and `backend/object/*` owns sections, symbols, and relocations.
- Register allocation and machine-level optimization are not yet separate crates
  (`omega-regalloc` / `omega-machine-optimization` do not exist); that work lives
  in the assigned-target-operations stage and the relevant backend crates.
- `backend/instruction_set_architectures/*` owns ISA definitions and encodings.
  Only AArch64 and x86_64 exist today. Shared lowering policy belongs in shared
  backend crates.
- The backend also carries fine-grained state and runtime lowering crates
  (`omega-state-*`, `omega-runtime-*`) plus `omega-runtime-abi`, which owns the
  backend-ABI carrier shapes (fat descriptors, slices, text windows) and their
  field-offset/subslice accessors. `omega-layout` and instruction selection
  consume those shapes rather than re-deriving them.
- `omega-calling-conventions` owns ABI value passing rules.
- `omega-platform-interface` owns ABI-facing host/platform surfaces.
- `backend/object/*` and `backend/images/*` should stay separate. Object writing
  and final image layout are different jobs. Per-format object writers and the
  linker crates are not yet separate packages.

### Runtime And Boundaries

- Process entry and startup-runtime replacement are placement intent for a
  future `compiler/runtime/startup/*` layer; there are no compiler-side runtime
  or startup crates today. Linkable startup payloads currently live as data under
  `target_runtime/`.
- `omega/host` models host boundary and authority surfaces, not random backend
  shortcuts.
- Import tables, export tables, loader metadata, startup selection, and final
  fixups are compiler responsibilities because Omega does not assume native
  system linkers.
- `.o` emission is a compatibility/debug bridge. Direct image construction from
  machine program data remains the long-term pressure.

### Identity And Data Shape

- Internal identity is handle-first, not string-first.
- Source text is source-loading, diagnostic, and debug payload, not semantic
  identity after resolution.
- User string literals are program payload. Debug names and linker names are
  edge metadata.
- Repeated durable children should prefer arena `Handle<T>` and `HandleSpan<T>`
  over recursive boxes or scattered owned vectors.
- Symbol lookup should prefer scoped symbol-tree walks. Add maps only for
  measured pathological scopes.

### Hygiene

- Public crate roots should explain exports, not hide implementation.
- Tests should not live in giant `lib.rs` files.
- `mod.rs` and `lib.rs` declare boundaries; they are not junk drawers.
