# Omega

Omega is an experimental systems programming language centered around explicit state, provable behavior, and data-oriented execution.

The central bet is simple: state machines should not be a framework pattern hidden inside a branch-heavy language. They should be the shape of the program. Machines own data, states perform work, and transitions describe control-flow handoff.

Current status: Omega is very early, but no longer purely theoretical. The compiler can parse/check all current samples, emit/link small native macOS ARM64 CLI programs, and writes phase artifacts for every compiler stage. The native path is still intentionally narrow; when a feature is not supported, the compiler should say so instead of pretending.

Tiny current program:

```omega
use platform::console;

machine main {
    contains console: Console;

    state entry {
        console.write_line("Hello, Omega.");
        console.exit_process(0);
    }
}
```

## Language Direction

Omega is exploring these core ideas:

- State machines are first-class citizens, not library objects.
- `machine main` with `state entry` is the process entry point.
- States prefer straight-line work. Branching belongs in ordered transition arrows.
- Transitions live at the end of states as `-> target` edges.
- A bare arrow target is unconditional; `when` adds a guard.
- `-> self` re-enters the current state.
- A trailing bare `->` marks explicit terminal/default completion when a transition table needs it.
- Nested machine flow can be expressed as `-> child.entry -> continuation`.
- Calls perform work, but do not imply hidden return-control semantics.
- Data flow should prefer explicit owned data and `mut` parameters over ambient state.
- Platform boundaries are explicit, trusted, and auditable.

Longer term, Omega wants compile-time proof integration. TLA+ style transition checks are a design goal, not decoration. The compiler should eventually derive formal transition models from source, challenge invariants and liveness properties, and only then lower the program.

Performance is also part of the language design. Omega should bias toward dense data, predictable access, SIMD-friendly transforms, and state graphs that can be optimized aggressively.

## Building

Run the full Rust workspace verification:

```bash
cargo test
```

Check the smallest CLI sample:

```bash
cargo run -p omega-cli -- --check samples/cli_mvp/main.omg
```

Build the smallest CLI sample on macOS ARM64:

```bash
cargo run -p omega-cli -- --target macos_arm64 samples/cli_mvp/main.omg
./samples/cli_mvp/build/omega-program
```

Check the richer samples:

```bash
cargo run -p omega-cli -- --check samples/dungeon_crawler_cli/main.omg
cargo run -p omega-cli -- --check samples/point_and_click/main.omg
```

Compile/check writes ignored phase artifacts under a `build/` directory next to the entrypoint unless `--build-dir <dir>` is provided.

Important artifact files:

- `00_timings.txt`: phase timing report.
- `01_sources.txt`: discovered source files.
- `02_ast.txt`: parsed source item summary.
- `03_resolve.txt`: imports, definitions, references.
- `04_types.txt`: type surface and effects.
- `05_driver_ir.txt`: lowered compiler IR.
- `06_validation.txt`: semantic validation summary.
- `07_graph.txt`: source and lowered state graph.
- `08_proof.txt`: proof surface and obligations.
- `09_native_plan.txt`: native target, host ABI, calls, data, instructions, object shape.
- `10_trust.txt`: trusted contracts and unchecked obligations.
- `11_emission_plan.txt`: whether native emission is currently possible.
- `12_emit.txt`: emitted object/container information.
- `13_link.txt`: linker invocation and result.

## Current Native Status

The native path currently supports a small but real subset:

- macOS ARM64 object emission.
- System linker invocation for runnable `omega-program` output.
- Host calls for stdout, stdin read buffers, and process exit.
- Unconditional state chains.
- Simple nested machine continuations.
- Constant integer assignment into host-call arguments.
- Static guarded transition selection for compile-time-known enum-style values.
- Static record/array/field text lowering for simple sample data.

Current known limitation:

- The dungeon crawler reaches parameterized helper-state guards such as `room.cell == cell` and stops there. The next native milestone is argument/parameter binding for scheduled state calls.

Targets without a real object writer still fall back to an Omega native object container so planned bytes remain inspectable.

## Workspace Shape

The repository should grow toward a feature-first Rust workspace with strong layering and deliberately explicit crate names. The goal is not a tiny academic compiler layout. The goal is a production-grade toolchain layout that can carry Omega from language bring-up through multi-platform shipping without leaning on LLVM or native system linkers.

Long-term design assumptions:

- The compiler owns its full native pipeline: parse, analyze, lower, optimize, select instructions, encode machine code, write object containers, resolve/link, and emit final platform images.
- All major executable formats are first-class: Mach-O, ELF, PE/COFF, and WebAssembly.
- The backend is shared where it should be shared, but architecture and platform boundaries stay obvious in the tree.
- The standard library, host contracts, startup/runtime, and platform ABI knowledge are versioned inside the workspace, not treated as mysterious external glue.

Legend:

- `[CRATE]` means a Cargo workspace package.
- Unprefixed folders are ordinary source/module boundaries inside a crate.

```text
Omega/
|-- Cargo.toml
|-- README.md
|-- apps/
|   |-- [CRATE] omega-cli/                              # User-facing `omega` command.
|   |-- [CRATE] omega-lsp/                              # Editor/language-service server.
|   `-- [CRATE] omega-doc/                              # Doc generation, package docs, symbol pages.
|
|-- compiler/
|   |-- foundation/
|   |   |-- [CRATE] omega-base/                         # Small shared primitives, ids, interners, utility traits.
|   |   |-- [CRATE] omega-arena/                        # Arena, paged arena, generational handles, handle spans.
|   |   |-- [CRATE] omega-span/                         # Source positions, file spans, expansion spans.
|   |   |-- [CRATE] omega-diagnostics/                  # Diagnostics, notes, labels, rendering, stable ids.
|   |   |-- [CRATE] omega-source/                       # Source files, source db, virtual paths, line maps.
|   |   |-- [CRATE] omega-vfs/                          # Real/fs overlay/package virtual filesystem.
|   |   |-- [CRATE] omega-intern/                       # String/symbol interning.
|   |   `-- [CRATE] omega-profiling/                    # Timings, phase counters, artifact metrics.
|   |
|   |-- frontend/
|   |   |-- [CRATE] omega-token/                        # Token definitions and trivia model.
|   |   |-- [CRATE] omega-lexer/                        # Source text to tokens.
|   |   |-- [CRATE] omega-cst/                          # Concrete syntax tree and lossless parse nodes.
|   |   |-- [CRATE] omega-parser/                       # Tokens to CST/AST.
|   |   |-- [CRATE] omega-ast/                          # Parsed source tree structs for semantic entry.
|   |   |-- [CRATE] omega-ast-lower/                    # AST to early semantic forms.
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
|   |   |-- [CRATE] omega-consteval/                    # Compile-time evaluation and folding.
|   |   |-- [CRATE] omega-graph/                        # Machine/state graph construction and validation.
|   |   |-- [CRATE] omega-proof/                        # Proof obligations, invariants, liveness hooks.
|   |   `-- [CRATE] omega-semantics/                    # Phase glue for semantic passes and canonical reports.
|   |
|   |-- intermediate-representations/
|   |   |-- [CRATE] omega-dataflow/                     # CFG/dataflow framework.
|   |   |-- [CRATE] omega-opt/                          # Machine-independent optimization passes.
|   |   |-- [CRATE] omega-hir/                          # High-level semantic IR.
|   |   |-- [CRATE] omega-lir/                          # Low-level target-aware IR before final encoding.
|   |   |-- [CRATE] omega-mir/                          # Mid-level IR after semantic lowering.
|   |   |-- [CRATE] omega-mir-build/                    # HIR to MIR lowering.
|   |   `-- [CRATE] omega-mono/                         # Monomorphization/specialization and code unit planning.
|   |
|   |-- backend/
|   |   |-- [CRATE] omega-target/                       # Target triples, cpu/features, os/env/object format matrix.
|   |   |-- [CRATE] omega-calling-contracts/            # Calling conventions, ABI, parameter passing, unwind contracts.
|   |   |-- [CRATE] omega-layout/                       # Type layout, alignments, field offsets, calling-convention records.
|   |   |-- [CRATE] omega-instruction-selection/        # Shared instruction selection framework.
|   |   |-- [CRATE] omega-regalloc/                     # Register allocation.
|   |   |-- [CRATE] omega-machine/                      # Machine function model, blocks, virtual/physical regs.
|   |   |-- instruction_set_architectures/
|   |   |   |-- [CRATE] omega-isa-aarch64/              # AArch64 instruction defs, encodings, lowering hooks.
|   |   |   |-- [CRATE] omega-isa-x86_64/               # x86_64 instruction defs, encodings, lowering hooks.
|   |   |   |-- [CRATE] omega-isa-riscv64/              # RISC-V 64 instruction defs, encodings, lowering hooks.
|   |   |   `-- [CRATE] omega-isa-wasm32/               # Wasm codegen surface where native image rules differ.
|   |   |
|   |   |-- object/
|   |   |   |-- [CRATE] omega-object/                   # Shared object model: sections, symbols, relocations.
|   |   |   |-- [CRATE] omega-object-elf/               # ELF object/container writer.
|   |   |   |-- [CRATE] omega-object-macho/             # Mach-O object/container writer.
|   |   |   |-- [CRATE] omega-object-coff/              # COFF/PE object/container writer.
|   |   |   `-- [CRATE] omega-object-wasm/              # Wasm module/object writer.
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
|   |       |-- [CRATE] omega-image-elf/                # Final ELF image layout, program headers, loaders.
|   |       |-- [CRATE] omega-image-macho/              # Final Mach-O image layout, load commands, fixups.
|   |       |-- [CRATE] omega-image-pe/                 # Final PE image layout, directories, imports, relocations.
|   |       `-- [CRATE] omega-image-wasm/               # Final Wasm module packaging.
|   |
|   |-- runtime/
|   |   |-- [CRATE] omega-runtime-core/                 # Shared runtime entry contracts and compiler intrinsics.
|   |   |-- [CRATE] omega-runtime-memory/               # Allocator/runtime memory surfaces if language needs them.
|   |   |-- [CRATE] omega-runtime-unwind/               # Panic/failure/unwind or abort-mode runtime surface.
|   |   |-- [CRATE] omega-runtime-host/                 # Trusted host-call shims and platform bridge contracts.
|   |   `-- startup/
|   |       |-- [CRATE] omega-startup-macos/            # Process entry, startup runtime replacement, platform bootstrap.
|   |       |-- [CRATE] omega-startup-linux/            # Process entry, startup runtime replacement, platform bootstrap.
|   |       |-- [CRATE] omega-startup-windows/          # Process entry, startup runtime replacement, platform bootstrap.
|   |       `-- [CRATE] omega-startup-wasm/             # Wasm start/export bootstrap.
|   |
|   |-- orchestration/
|   |   |-- [CRATE] omega-queries/                      # Incremental/query engine and cache keys.
|   |   |-- [CRATE] omega-artifacts/                    # Phase artifact models and text/binary dumping.
|   |   |-- [CRATE] omega-session/                      # Compilation session, options, build graph, worker pools.
|   |   `-- [CRATE] omega-compiler/                     # Top-level check/build API used by cli/lsp/tests.
|   |
|   `-- tool_support/
|       |-- [CRATE] omega-ide/                          # Semantic tokens, completion, hover, go-to-def support.
|       `-- [CRATE] omega-doc-model/                    # Shared doc extraction model for cli/lsp/doc tooling.
|
|-- omega/
|   |-- core/                                           # Language core package shipped with every toolchain.
|   |-- alloc/                                          # Allocation/data-structure package if language needs it.
|   |-- std/                                            # Higher-level standard package surface.
|   |-- host/                                           # Trusted host contracts and audited platform surfaces.
|   `-- platform/                                       # Platform-specific Omega packages and startup bindings.
|
|-- runtimes/
|   |-- startup_objects/                                # Compiler-owned startup/runtime support objects.
|   `-- platform/                                       # Link-time runtime assets and metadata by target family.
|
|-- samples/
|   |-- cli_mvp/                                        # Smallest console program.
|   |-- dungeon_crawler_cli/                            # Console input/output and room navigation pressure test.
|   `-- point_and_click/                                # Windowed game/state-machine sketch.
|
|-- canaries/
|   |-- pass/                                           # Tiny feature canaries expected to check.
|   `-- fail/                                           # Tiny negative canaries with expected diagnostics.
|
|-- tests/
|   |-- integration/                                    # End-to-end compiler tests.
|   |-- target_corpus/                                  # Per-target ABI/object/link/image tests.
|   `-- bootstrap/                                      # Future self-hosting/bootstrap tests.
|
`-- wiki/                                               # Language design notes, target notes, and guide drafts.
```

## Internal Placement Rules

These are the current rules of thumb. They are allowed to evolve, but the README should stay current when they do.

- `omega-cli`, `omega-lsp`, and `omega-doc` stay thin. They parse user intent and call `omega-compiler` or `omega-ide`; they do not own compiler semantics.
- `foundation/` must stay dependency-light. If a crate there starts depending on HIR, MIR, or target details, it is in the wrong layer.
- `frontend/` owns syntax and source-preserving structure only. Name resolution, type facts, and control-flow meaning belong in `semantics/`.
- `packages/` owns manifests, dependency graphs, and source loading. It should not grow semantic rules for the language itself.
- `omega-hir` is the first meaning-bearing IR. Parser conveniences and concrete syntax trivia do not belong there.
- `semantics/` proves and reports what the program means. `ir/` decides how that meaning is represented for optimization and code generation.
- `omega-graph` and `omega-proof` stay semantic/model-facing first. Do not bury language-level state-machine reasoning inside machine-code crates.
- `omega-mir` and `omega-lir` are long-lived boundaries. Do not skip straight from HIR to ad hoc backend structs once the compiler grows.
- `backend/isa/*` owns architecture-specific instruction definitions and encoding. Shared lowering policy belongs in `omega-isel`, not duplicated per ISA unless the target really demands it.
- `backend/object/*` writes relocatable containers. `backend/link/*` resolves symbols, applies relocations, strips dead sections, and builds final images. Do not blur object writing and linking together.
- `backend/images/*` owns final executable/shared-library layout rules. Platform image concerns should not leak upward into generic optimization crates.
- Because Omega does not rely on native system linkers, import tables, export tables, load commands, dynamic loader metadata, startup entry selection, and final fixups are first-class compiler responsibilities.
- `runtime/startup/*` owns entry bootstrap code and startup-runtime replacement logic. Keep process start rules out of random backend files.
- `omega-queries` and `omega-session` own orchestration, caching, and artifact production. They should call phases, not absorb the phase logic itself.
- Tests should not live in `lib.rs`; public crate roots should explain exports, not hide 2,000 lines of behavior.
- `mod.rs` and `lib.rs` should declare boundaries, not become implementation junk drawers.

## Samples And Canaries

Samples are language pressure tests. They may be pseudocode-ish if the language is still being shaped.

Current samples:

- `samples/cli_mvp/`: hello-world style CLI program.
- `samples/dungeon_crawler_cli/`: static room layout, console input, room info, movement commands.
- `samples/point_and_click/`: windowed game sketch with room ownership and render-loop boundaries.

Canaries are not samples. They isolate one compiler capability at a time.

Current feature canaries include:

- `state_transition_chain`
- `nested_machine_continuation`
- `owned_assignment_before_exit`
- `guarded_transition_dispatch`
- `mutable_output_host_call`
- `record_array_field_access`

## Bundled Omega Packages

Imports beginning with `omega::` resolve to bundled Omega source packages under `omega/`.

Package paths can resolve to either `name.omg` or `name/mod.omg`, so larger packages such as `omega::host::windows` can live in folders and shard their contracts by domain.

Set `OMEGA_LIBRARY_ROOT` to point at a different bundled library root when testing an installed or alternate toolchain layout.

## [READONLY] Coding Conventions

- Use real words in code. Prefer `character`, `statement`, `expression`, and `arguments` over `ch`, `stmt`, `expr`, and `args`.
- Avoid names that only make sense to compiler insiders. `pipeline` is better than `driver`; `expression` is better than `expr`.
- Keep compiler stages honest. Parse syntax, lower representation, validate semantics, plan native execution, then emit bytes.
- Keep sample coverage out of the shipped CLI. Tests and dev harnesses may discover `samples/`, but user-facing compiler behavior stays generic.
- Prefer small checkpoint commits after working improvements.
- Samples should reveal language pressure, not hide it in giant `main` files.
- Prefer arena-backed compiler data. Contiguous storage and small handles beat a pile of tiny heap allocations.
- Lowered IR should prefer `Handle<T>` and `HandleSpan<T>` over owned `Vec<T>` fields for repeated child lists.
- `Vec<T>` is fine for parser output, temporary builders, and local scratch data. It should not become the default long-lived IR shape.
- Prefer arena/vector-backed symbol tables over local hash maps. Names should collapse toward ids/handles as phases mature.
- Use paged arenas for shared or eventually-parallel compiler data where growth should not move existing pages or require locking one giant `Vec`.
- Paged arenas use generational handles so reclaimed page storage cannot resurrect stale references.
- Do not use `RefCell` as an ownership escape hatch. Runtime borrow checking is not a substitute for clear compiler-phase ownership.
- Prefer ZII (Zero-is-initialization). Null handles (index 0) resolve to dummy arena entries instead of optionals and literal nulls.
- Arena handles must be generational. Freed or stale handles resolve to dummy entries, not reused storage.
- Use stable handles when data needs references across phases; use redirect tables only when arena contents need reordering.
- Comments should explain non-obvious intent. Do not add “doing X unlike Rust” commentary unless the contrast changes implementation.

## Useful Commands

Run tests:

```bash
cargo test
```

Check a sample:

```bash
cargo run -p omega-cli -- --check samples/cli_mvp/main.omg
```

Compile a sample on macOS ARM64:

```bash
cargo run -p omega-cli -- --target macos_arm64 samples/cli_mvp/main.omg
```

Inspect canaries:

```bash
cargo test -p omega-driver checks_passing_canaries
cargo test -p omega-driver rejects_failing_canaries
```

## Design Notes

The language is moving quickly. The best current design references are:

- [Language Vision](wiki/language-vision.md)
- [State And Transition Model](wiki/state-transition-model.md)
- [Omega Language Guide](wiki/language_guide/README.md)
