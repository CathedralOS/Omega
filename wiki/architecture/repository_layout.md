# Repository Layout

This page is a map and a placement guide. It should help answer two questions:

- Where should a new crate or module live?
- Which layer is allowed to own a concept?

The pipeline-specific semantic rules live in
[Pipeline Architecture](pipeline/pipeline.md).

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

```text
Omega/
|-- Cargo.toml
|-- README.md
|-- apps/
|   |-- [CRATE] omega-cli/                              # User-facing `omega` command.
|   |-- [CRATE] omega-language-server/                  # Editor language server over LSP.
|   `-- [CRATE] omega-documentation-generator/          # User-facing docs generator for Omega packages.
|
|-- compiler/
|   |-- foundation/
|   |   |-- [CRATE] omega-base/                         # Small shared primitives, ids, interners, utility traits.
|   |   |-- [CRATE] omega-arena/                        # Arena, paged arena, generational handles, handle spans.
|   |   |-- [CRATE] omega-span/                         # Source positions, file spans, expansion spans.
|   |   |-- [CRATE] omega-diagnostics/                  # Diagnostics, notes, labels, rendering, stable ids.
|   |   |-- [CRATE] omega-source/                       # Source files, source db, virtual paths, line maps.
|   |   |-- [CRATE] omega-virtual-file-system/.         # Real/fs overlay/package virtual filesystem.
|   |   |-- [CRATE] omega-intern/                       # String/symbol interning.
|   |   `-- [CRATE] omega-profiling/                    # Timings, phase counters, artifact metrics.
|   |
|   |-- packages/
|   |   |-- [CRATE] omega-manifest/                     # Package manifests, target declarations, metadata.
|   |   |-- [CRATE] omega-package-graph/                # Package discovery, dependency graph, workspace graph.
|   |   |-- [CRATE] omega-loader/                       # Package/module loading over VFS/source db.
|   |   `-- [CRATE] omega-registry/                     # Package registry/client logic for future distribution.
|   |
|   |-- semantics/
|   |   |-- [CRATE] omega-names/                        # Definitions, scopes, imports, symbol resolution.
|   |   |-- [CRATE] omega-types/                        # Type checking, inference, coercions, layout preconditions.
|   |   |-- [CRATE] omega-effects/                      # Effect surface and authority-flow checking.
|   |   |-- [CRATE] omega-borrow/                       # Ownership, aliasing, lifetime-style checks as needed.
|   |   |-- [CRATE] omega-invariants/                   # Variable/state invariant propagation and refinement checking.
|   |   |-- [CRATE] omega-contracts/                    # Requires/ensures/halts-style callable and machine contracts.
|   |   |-- [CRATE] omega-validation/                   # Cross-semantic program validation and diagnostics.
|   |   |-- [CRATE] omega-consteval/                    # Compile-time evaluation and folding.
|   |   |-- [CRATE] omega-graph/                        # Machine/state graph construction and graph-facing semantic facts.
|   |   |-- [CRATE] omega-proof/                        # Proof obligations, invariants, liveness hooks.
|   |   `-- [CRATE] omega-semantics/                    # Phase glue for semantic passes and canonical reports.
|   |
|   |-- representations/
|   |   |-- [CRATE] omega-source-files/                 # Discovered and loaded source files with stable source identity.
|   |   |-- [CRATE] omega-tokens/                       # Per-file token streams and token-set ownership.
|   |   |-- [CRATE] omega-syntax-trees/                 # Parsed source structure before names and symbols are resolved.
|   |   |-- [CRATE] omega-symbol-resolved-trees/        # SymbolResolvedTrees: syntax shape with declaration/reference symbols resolved.
|   |   |-- [CRATE] omega-typed-trees/                  # Symbol-resolved trees with type/effect information attached.
|   |   |-- [CRATE] omega-checked-trees/                # Typed trees plus checked semantic facts after validation/proof-facing checks.
|   |   |-- [CRATE] omega-state-graph/                  # Explicit machine/state graph for proof and scheduling.
|   |   |-- [CRATE] omega-control-flow/                 # Control-flow/data-flow graph.
|   |   |-- [CRATE] omega-abstract-operations/          # Target-independent abstract operations with virtual registers.
|   |   |-- [CRATE] omega-target-operations/            # Target-aware operations after legalization and selection.
|   |   |-- [CRATE] omega-assigned-target-operations/   # Target-aware operations after register and stack assignment.
|   |   |-- [CRATE] omega-machine-instructions/         # Final symbolic machine instructions before object-file encoding.
|   |   `-- [CRATE] omega-object-file/                  # Relocatable object-file payload with sections, symbols, and relocations.
|   |
|   |-- pipeline/
|   |   |-- [CRATE] omega-source-files-to-tokens/                   # Source files to per-file token streams.
|   |   |-- [CRATE] omega-tokens-to-syntax-trees/                   # Token streams to parsed syntax trees.
|   |   |-- [CRATE] omega-syntax-trees-to-symbol-resolved-trees/.   # Syntax trees to SymbolResolvedTrees with symbol identity attached.
|   |   |-- [CRATE] omega-symbol-resolved-trees-to-typed-trees/.    # SymbolResolvedTrees to typed/effect-annotated trees.
|   |   |-- [CRATE] omega-typed-trees-to-checked-trees/             # Typed trees to validated/proof-checked trees with semantic facts.
|   |   |-- [CRATE] omega-checked-trees-to-state-graph/             # Checked trees to explicit machine/state graph.
|   |   |-- [CRATE] omega-state-graph-to-control-flow/              # State graph to control-flow/data-flow graph.
|   |   |-- [CRATE] omega-control-flow-to-abstract-operations/      # Lower control flow into target-independent abstract operations with virtual registers.
|   |   |-- [CRATE] omega-abstract-operations-to-target-operations/ # Normalize, legalize, and lower abstract operations into target-aware forms.
|   |   |-- [CRATE] omega-target-operations-to-assigned-target-operations/    # Assign registers, stack slots, and calling-convention homes to target-aware operations.
|   |   |-- [CRATE] omega-assigned-target-operations-to-machine-instructions/ # Convert assigned target-aware operations into final symbolic machine instructions.
|   |   `-- [CRATE] omega-machine-instructions-to-object-file/                # Encode machine instructions into object files with symbols and relocations.
|   |
|   |-- backend/
|   |   |-- [CRATE] omega-target/                       # Target triples, cpu/features, os/env/object format matrix.
|   |   |-- [CRATE] omega-platform-interface/           # ABI-facing OS/platform imports, host surfaces, loader facts.
|   |   |-- [CRATE] omega-calling-conventions/          # ABI rules for registers, stack, parameter/return passing.
|   |   |-- [CRATE] omega-layout/                       # Type layout, alignments, field offsets, calling-convention records.
|   |   |-- [CRATE] omega-instruction-selection/        # Shared instruction selection framework.
|   |   |-- [CRATE] omega-regalloc/                     # Register allocation.
|   |   |-- [CRATE] omega-machine-optimization/         # Machine-level liveness, scheduling, peepholes, branch relaxation.
|   |   |-- [CRATE] omega-machine-emission/             # Final machine program to encoded bytes.
|   |   |-- instruction_set_architectures/
|   |   |   |-- [CRATE] omega-isa-aarch64/              # AArch64 instruction defs, encodings, lowering hooks.
|   |   |   |-- [CRATE] omega-isa-x86_64/               # x86_64 instruction defs, encodings, lowering hooks.
|   |   |   |-- [CRATE] omega-isa-riscv64/              # RISC-V 64 instruction defs, encodings, lowering hooks.
|   |   |   `-- [CRATE] omega-isa-wasm32/               # Wasm codegen surface where native image rules differ.
|   |   |
|   |   |-- object/
|   |   |   |-- [CRATE] omega-object-file/              # Shared object-file representation: sections, symbols, relocations.
|   |   |   |-- [CRATE] omega-object-file-planning/     # Builds section and symbol plans before object-file or image writing.
|   |   |   |-- [CRATE] omega-relocations/              # Builds relocation records over selected and machine instructions.
|   |   |   |-- [CRATE] omega-object-file-elf/          # ELF object-file writer.
|   |   |   |-- [CRATE] omega-object-file-macho/        # Mach-O object-file writer.
|   |   |   |-- [CRATE] omega-object-file-coff/         # COFF/PE object-file writer.
|   |   |   `-- [CRATE] omega-object-file-wasm/         # Wasm object-file writer.
|   |   |
|   |   |-- linker/
|   |   |   |-- [CRATE] omega-linker/                   # Compiler-owned linker driver and graph orchestration.
|   |   |   |-- [CRATE] omega-linker-base/              # Shared symbol resolution, relocation, gc, comdat rules.
|   |   |   |-- [CRATE] omega-linker-elf/               # ELF executable/shared-object linking.
|   |   |   |-- [CRATE] omega-linker-macho/             # Mach-O executable/dylib linking.
|   |   |   |-- [CRATE] omega-linker-pe/                # PE/COFF executable/dll linking.
|   |   |   `-- [CRATE] omega-linker-wasm/              # Wasm final module linking and import/export shaping.
|   |   |
|   |   `-- images/
|   |       |-- [CRATE] omega-image/                    # Shared final image data model and fixup helpers.
|   |       |-- [CRATE] omega-image-emission/           # Selects the final executable image writer for a target.
|   |       |-- [CRATE] omega-image-elf/                # Final ELF image layout, program headers, loaders.
|   |       |-- [CRATE] omega-image-macho/              # Final Mach-O image layout, load commands, fixups.
|   |       |-- [CRATE] omega-image-pe/                 # Final PE image layout, directories, imports, relocations.
|   |       `-- [CRATE] omega-image-wasm/               # Final Wasm module packaging.
|   |
|   |-- runtime/
|   |   |-- [CRATE] omega-runtime-core/                 # Shared runtime entry contracts and compiler intrinsics.
|   |   |-- [CRATE] omega-runtime-memory/               # Allocator/runtime memory surfaces if language needs them.
|   |   |-- [CRATE] omega-runtime-unwind/               # Panic/failure/unwind or abort-mode runtime surface.
|   |   |-- [CRATE] omega-runtime-host/                 # Boundary host-call shims and platform bridge contracts.
|   |   `-- startup/
|   |       |-- [CRATE] omega-startup-macos/            # Process entry, startup runtime replacement, platform bootstrap.
|   |       |-- [CRATE] omega-startup-linux/            # Process entry, startup runtime replacement, platform bootstrap.
|   |       |-- [CRATE] omega-startup-windows/          # Process entry, startup runtime replacement, platform bootstrap.
|   |       `-- [CRATE] omega-startup-wasm/             # Wasm start/export bootstrap.
|   |
|   |-- orchestration/
|   |   |-- [CRATE] omega-queries/                      # Incremental/query engine and cache keys.
|   |   |-- [CRATE] omega-artifacts/                    # Phase artifact data and text/binary dumping.
|   |   |-- [CRATE] omega-session/                      # Compilation session, options, build graph, worker pools.
|   |   |-- [CRATE] omega-backend-pipeline/             # Backend phase sequencing at the orchestration edge.
|   |   `-- [CRATE] omega-compiler/                     # Top-level check/build API used by cli/lsp/tests.
|   |
|   `-- tool_support/
|       |-- [CRATE] omega-ide/                          # Semantic tokens, completion, hover, go-to-def support.
|       `-- [CRATE] omega-program-documentation/        # Documentation view of Omega programs for cli/lsp/doc tooling.
|
|-- omega/
|   |-- language/
|   |   |-- core/                                       # Always-available language package.
|   |   |-- alloc/                                      # Allocation and owned collection surfaces.
|   |   `-- std/                                        # Higher-level standard package surface.
|   |
|   `-- host/
|       |-- contracts/                                  # Cross-platform boundary and authority contracts.
|       |-- standard/                                   # Default host boundary and authority bundle.
|       `-- targets/
|           |-- darwin/
|           |-- linux/
|           |-- windows/
|           `-- wasm/
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

- `apps/` crates stay thin. They parse user intent and call compiler or IDE
  services.
- `orchestration/` sequences phases, owns sessions/options/artifacts, and keeps
  source loading coherent.
- `orchestration/` must not become the home for semantic checks or backend
  lowering.

### Source And Packages

- `foundation/` stays dependency-light. If it needs semantic or target details,
  it is in the wrong layer.
- Source-preserving syntax data belongs in `representations/`; source-to-syntax
  transforms belong in `pipeline/`. Do not add a `frontend/` layer unless a
  concrete formatter/lossless-CST subsystem earns its own home.
- `packages/` owns manifests, package graphs, and loading. It does not own
  language semantics.
- Source discovery/loading should remain an orchestration subsystem, not parser
  responsibility.

### Semantic Ownership

- `semantics/` owns language meaning: names, types, effects, contracts, proofs,
  domains, borrow checking, invariants, and validation.
- `omega-borrow` answers who may read or mutate.
- `omega-invariants` answers what remains true.
- `omega-contracts` answers what a callable requires or promises.
- `omega-proof` discharges obligations.
- `omega-graph` stays language/proof-facing; do not bury state-machine reasoning
  in backend crates.

### Representations And Pipeline

- `representations/` owns durable IR data structures and arena storage.
- `pipeline/` crates transform one representation into the next.
- Pipeline crates may depend on input and output representations, but should not
  become owners of shared helper structures.
- Long-lived representation boundaries should remain explicit: syntax,
  symbol-resolved, typed, checked, state graph, control flow, abstract
  operations, target operations, assigned target operations, machine
  instructions, and object/image data.
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
- `omega-machine-instructions-to-object-file` owns final instruction bytes,
  sections, symbols, and relocations.
- `backend/instruction_set_architectures/*` owns ISA definitions and encodings.
  Shared lowering policy belongs in shared backend crates.
- `omega-calling-conventions` owns ABI value passing rules.
- `omega-platform-interface` owns ABI-facing host/platform surfaces.
- `backend/object/*`, `backend/linker/*`, and `backend/images/*` should stay
  separate. Object writing, linking, and final image layout are different jobs.

### Runtime And Boundaries

- `runtime/startup/*` owns process entry and startup-runtime replacement logic.
- `omega/runtime` and `omega/host` model runtime and host boundary surfaces, not
  random backend shortcuts.
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
