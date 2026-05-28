# Repository Layout
The repository should grow toward a feature-first Rust workspace with strong layering and deliberately explicit crate names. The goal is not a tiny academic compiler layout. The goal is a production-grade toolchain layout that can carry Omega from language bring-up through multi-platform shipping without leaning on LLVM or native system linkers.

Architecture notes start at [Architecture](architecture.md), including the
pipeline document that explains how places, values, facts, loans, moves, drops,
calls, transitions, effects, and boundary edges should evolve across IRs.

Long-term design assumptions:

- The compiler owns its full native pipeline: parse, analyze, lower, optimize, select instructions, encode machine code, write object containers, resolve/link, and emit final platform images.
- All major executable formats are first-class: Mach-O, ELF, PE/COFF, and WebAssembly.
- The backend is shared where it should be shared, but architecture and platform boundaries stay obvious in the tree.
- The standard library, host contracts, startup/runtime, and calling-convention/platform ABI knowledge are versioned inside the workspace, not treated as mysterious external glue.

Current migration note: the old native bring-up bridge has been split apart. Domain logic now lives in explicit backend crates, while `compiler/orchestration/omega-backend-pipeline` owns the remaining phase sequencing. The long-term pressure stays the same: phase boundaries should become precise enough that temporary aggregate report surfaces disappear instead of ossifying.

Near-term compiler pipeline direction:

- `omega-compiler` should read like an obvious conveyor belt: load sources, lex, parse, discover imports, assemble syntax, resolve, typecheck, validate, plan backend, emit, optionally write outputs.
- Source discovery/loading is a subsystem inside `compiler/orchestration/omega-compiler/src/pipeline/source/`, not a responsibility smeared across lexer, parser, and compiler entrypoints.
- The source subsystem should own frontier management, canonical source identity, file contents, and compiler-local source storage.
- Lexer and parser should stay narrow: `TokenStream` in, syntax out. Import discovery happens from parsed per-file structure, not by giving the parser orchestration jobs.
- New crates should come later, only after these boundaries stop moving. For now, the right move is cleaner subsystems inside the orchestration crate.

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
|   |-- frontend/
|   |   |-- [CRATE] omega-concrete-syntax-tree/         # Comments and lossless parse nodes (CST).
|   |   |-- [CRATE] omega-syntax-trees/                 # Parsed source structure; expressions and child lists should be arena handles, not recursive boxes.
|   |   `-- [CRATE] omega-format/                       # Formatter and syntax-preserving rewrites.
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
|   |   |-- [CRATE] omega-effects/                      # Effect surface, mutation/host capability checking.
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
|       |-- contracts/                                  # Cross-platform boundary capability contracts.
|       |-- standard/                                   # Default host capability bundle.
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

## Internal Placement Rules

These are the current rules of thumb. They are allowed to evolve, but the README should stay current when they do.

- `omega-cli`, `omega-language-server`, and `omega-documentation-generator` stay thin. They parse user intent and call `omega-compiler` or `omega-ide`; they do not own compiler semantics.
- `foundation/` must stay dependency-light. If a crate there starts depending on semantic representations, machine representations, or target details, it is in the wrong layer.
- `frontend/` owns syntax definitions and source-preserving structure only. The durable per-file outputs of frontend work belong in `representations/`. Name resolution, type facts, and control-flow meaning belong in `semantics/`.
- `omega-syntax-trees` should be table-shaped, not a long-lived recursive heap tree. Recursive syntax edges should be `Handle<T>` and repeated children should be `HandleSpan<T>` so parser output does not normalize tiny allocations into the rest of the compiler.
- `packages/` owns manifests, dependency graphs, and source loading. It should not grow semantic rules for the language itself.
- `omega-symbol-resolved-trees` exports `SymbolResolvedTrees`, the first representation where source spelling has been disambiguated into symbol handles. Parser conveniences and concrete syntax trivia do not belong there; names are diagnostic payload, not semantic equality.
- `semantics/` proves and reports what the program means. `representations/` decides how that meaning is shaped for optimization and code generation.
- `omega-borrow` answers who may read or mutate. `omega-invariants` answers what remains true. `omega-contracts` answers what a callable or machine requires or promises. `omega-proof` is where those obligations are discharged.
- `omega-graph` and `omega-proof` stay semantic/proof-facing first. Do not bury language-level state-machine reasoning inside machine-code crates.
- `omega-symbol-resolved-trees`/`SymbolResolvedTrees`, `omega-typed-trees`, `omega-state-graph`, `omega-control-flow`, `omega-abstract-operations`, `omega-target-operations`, `omega-assigned-target-operations`, `omega-machine-instructions`, and `omega-object-file` are long-lived boundaries. Do not skip straight from source-shaped structures to ad hoc backend structs once the compiler grows. These cover the territory other compilers often split into HIR, MIR, target-independent abstract operations, target-aware low-level operations, assigned target operations, symbolic machine instructions, and final object-file emission.
- `representations/` owns the durable structs and arena data, including frontend products like source files, token streams, and syntax trees. `pipeline/` crates transform from one representation to the next, depend on both sides, and should not become owners of shared helper structures.
- `omega-control-flow-to-abstract-operations` is where the backend stops pretending control flow is already machine-shaped. This phase should produce target-independent abstract operations with infinite virtual registers, explicit values, and explicit effects.
- `omega-abstract-operations-to-target-operations` is where normalization, legalization, and instruction-shape lowering belong. This phase may depend on target, ISA, layout, and calling-convention facts, but it should still be a pure representation-to-representation transform: abstract operations in, target-aware operations out.
- `omega-target-operations-to-assigned-target-operations` is where register allocation, stack-slot assignment, spill insertion, and calling-convention placement become concrete. If a pass needs physical registers or fixed stack homes, it belongs here or immediately around it.
- `backend/instruction_set_architectures/*` owns architecture-specific instruction definitions and encoding. Shared lowering policy belongs in `omega-instruction-selection`, not duplicated per architecture unless the target really demands it.
- `omega-assigned-target-operations` is the post-assignment layer: target-aware operations whose registers, stack homes, and calling-convention placements are already decided, but which have not yet been converted into final machine instructions.
- `omega-machine-instructions` is Omega's final symbolic instruction layer: physical registers and stack homes may already be assigned, but labels, relocations, and object-file concerns are still symbolic and inspectable here.
- `omega-machine-instructions-to-object-file` is where final bytes and relocation-bearing object contents are born. No earlier representation should carry encoded instruction bytes as its primary truth. Final instruction encodings, section layout, relocation records, and symbol references belong here or immediately around it in backend-only passes.
- `omega-calling-conventions` owns ABI-level rules for how values cross state/function/platform boundaries: registers, stack slots, return locations, and callee/caller responsibilities.
- `omega-platform-interface` owns ABI-facing OS/platform call surfaces, imports, loader-visible symbols, and host integration facts.
- `backend/object/*` writes relocatable containers. `backend/linker/*` resolves symbols, applies relocations, strips dead sections, and builds final images. Do not blur object writing and linking together.
- `backend/images/*` owns final executable/shared-library layout rules. Platform image concerns should not leak upward into generic optimization crates.
- Because Omega does not rely on native system linkers, import tables, export tables, load commands, dynamic loader metadata, startup entry selection, and final fixups are first-class compiler responsibilities.
- `.o` emission is a compatibility/debug bridge, not the default long-term architecture. The compiler should move toward direct executable image construction from machine program data.
- Internal symbols should be handle-first, not string-first. A symbol's identity should come from a generational arena handle; parentage, kind, linkage, and optional debug/display strings are metadata. Names needed by final images, imports, exports, diagnostics, or debug info should be generated or retained at the edge, not propagated through every lowering layer.
- Symbol resolution should follow the symbol tree before reaching for a global resolver map. A symbol handle is identity; its parent chain and `HandleSpan` child range are the natural scope structure. `HierarchyArena` is the foundation shape for this: builders may patch exact child spans during construction, while published arenas are immutable and walk only the child range a parent owns. Resolving `self.player.inventory.drop_items` is a sequence of local sibling probes: find `player` under the current machine, find `inventory` under `Player`, then find `drop_items` under `Inventory`. This keeps lookups dense and scoped instead of maintaining a giant `(parent, string) -> symbol` table by default. If instrumentation later shows a parent with pathological child counts, add a sorted child range or parent-local side index for that scope only.
- Source text does not survive resolution as compiler identity. Tokens and abstract syntax may point at source files with `SourceSpan`, and the resolver may compare those spans against the symbol table while building handles. After resolution, program identity is `SymbolHandle` or a more specific typed handle. If later phases need spelling, they ask the symbol table or source map for diagnostics/debug output; they do not carry source strings as data-flow truth.
- User-defined string literals are payload, not identity, and may flow through typed program data into emission. Debug/display names are symbol metadata and may be omitted or stripped from release artifacts. Compiler-generated linker/platform names are target-edge payload and should be introduced near the image/linking layer, not propagated through semantic representations.
- `runtime/startup/*` owns entry bootstrap code and startup-runtime replacement logic. Keep process start rules out of random backend files.
- `omega-queries` and `omega-session` own orchestration, caching, and artifact production. They should call phases, not absorb the phase logic itself.
- Tests should not live in `lib.rs`; public crate roots should explain exports, not hide 2,000 lines of behavior.
- `mod.rs` and `lib.rs` should declare boundaries, not become implementation junk drawers.
