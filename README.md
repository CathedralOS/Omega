# Omega

Omega is a systems programming language with zero-cost abstractions and no unsafe code, while still achieving C and Rust-like speeds. All code is modeled by data-oriented state machines, borrow-checked memory access, proof-carrying behavior, and capability-aware boundaries.

In other words:
- No deadlocks within the selected checked concurrency model; opaque external
  waits remain explicit assumptions or rejected boundaries.
- Termination is verified where promised — iteration is explicit state transitions; call cycles must prove termination; a `terminates` claim is enforced transitively.
- No stack overflows.
- No out-of-bounds indexing.
- No divide-by-zero.
- No unsafe memory access.
- Inline assembly is allowed when **provably** safe.
- Provable instruction/CPU budgets.
- Full transparency over system effects, such as filesystem or network access. Ban libraries that use any capabilities that seem dangerous.
- No panics in external libraries that have no reason to panic.

Omega is designed to be safe enough to control an airplane, and powers the [Cathedral](https://github.com/CathedralOS/Cathedral) operating system.

## Language Direction

Omega is chasing a few connected ideas:

- State machines are syntax, not library objects. `machine`, `state`, and `transition` give control flow a durable graph shape instead of burying it in arbitrary branches.
- Proof is part of normal compilation. Contracts, domains, bounded values, borrow facts, slice bounds, termination claims, and transition obligations are meant to be checked before the backend gets to emit bytes.
- Authority should flow through values. Effects stay coarse and readable, while capabilities are tracked through values, domains, provenance, and boundary calls so package reports can say what code accepts, uses, derives, stores, returns, releases, or acquires.
- Core collections are proof surfaces. Arrays, vectors, slices, strings, and string views should expose browsable operators and measures such as `Slice::Length`, while pointer/descriptor machinery stays behind explicit compiler/runtime boundaries.
- Data layout matters. Omega should bias toward owned data, dense arenas, predictable access, SIMD-friendly transforms, and state graphs that can be optimized because their semantics are visible.
- Native output is a first-class goal. The compiler is growing its own path from source to machine code, object data, linking, and final platform images instead of treating executable construction as mysterious external glue.

The long-term pitch is ambitious on purpose: write programs as explicit state evolution, let the compiler challenge the facts, and then lower the surviving program into tight native code.

## Building

Run the full Rust workspace verification:

```bash
cargo test
```

If many small Rust crates each pause for seconds before parsing, inspect the
derived Cargo cache before changing compiler or test architecture. A long-lived
`target/debug/deps` with hundreds of thousands of stale hashed artifacts makes
rustc rescan that directory for every crate. `cargo clean` removes only
rebuildable `target/` output and restores a compact cache; the next build is
cold.

Check the smallest CLI sample:

```bash
cargo run -p omega-cli -- --check samples/cli/basics/cli_mvp/main.omg
```

Build the smallest CLI sample on macOS ARM64:

```bash
cargo run -p omega-cli -- --target macos_arm64 samples/cli/basics/cli_mvp/main.omg
./samples/cli/basics/cli_mvp/build/omega-program
```

Build the smallest CLI sample as a direct Linux ARM64 ELF image:

```bash
cargo run -p omega-cli -- --target linux_arm64 samples/cli/basics/cli_mvp/main.omg
docker run --rm --platform linux/arm64 -v "$PWD:/work" -w /work alpine:3.20 ./samples/cli/basics/cli_mvp/build/omega-program
```

Check the richer samples:

```bash
cargo run -p omega-cli -- --check samples/cli/games/dungeon_crawler_cli/main.omg
```

Compile/check writes ignored phase artifacts under a `build/` directory next to the entrypoint unless `--build-dir <dir>` is provided.

Important artifact files:

- `00_timings.txt`: phase timing report.
- `01_sources.txt`: discovered source files.
- `02_ast.txt`: parsed source item summary.
- `03_resolve.txt`: imports, definitions, references.
- `04_types.txt`: type surface and effects.
- `05_typed_program.txt`: lowered compiler representation.
- `06_validation.txt`: semantic validation summary.
- `07_graph.txt`: source and lowered state graph.
- `08_proof.txt`: proof surface and obligations.
- `09_backend_plan.txt`: target, host ABI, calls, data, instructions, and image planning.
- `10_boundary.html`: boundary contracts and unchecked obligations.
- `12_emission.txt`: whether native emission is currently possible.
- `13_emitted_output.txt`: emitted native output information.
- `14_finalization.txt`: executable finalization and permission stamping for directly emitted images.

These are outputs of the current Rust development command, not required
bootstrap artifacts. Human-only reports and HTML views may be disabled or
removed from default validation paths when they impose measurable cost; the
hosted product closure includes only tooling the compiler executable imports.

## Current Native Status

The native path runs real programs on macOS ARM64, Windows x64, and Linux
(x64 and ARM64 ELF), all as directly emitted executable images:

- Runtime dispatch over machine/state graphs, including nested machine calls, value-position calls, and guarded multi-arm transitions with payload-binding case arms.
- Integer arithmetic across widths and signedness (including division, shifts, min/max), f32/f64 arithmetic and comparisons, and width-honest casts — all verified against a reference interpreter as a differential oracle (exit code and stdout must match exactly).
- Console host calls on every target: stdout, stderr, line-disciplined stdin (CRLF-correct), and process exit. The full `dungeon_crawler_cli` sample runs its scripted loop byte-identically to the interpreter.
- Slices and fat descriptors: element reads/writes through views, subslicing
  (`items[1..]`), descriptor materialization, and runtime text building.
- Case payload construction, tag dispatch, membership tests (`in`),
  synthesized structural equality, and plan-generated `compact_binary`
  encoders with byte-exact LEB128 output.

The general implementation queue and its acceptance checks live in
[`TASKS.md`](TASKS.md). Optimizer architecture and its dedicated execution queue
live in
[`optimizer_architecture.md`](wiki/design_briefs/optimizer_architecture.md) and
[`TASKS_OPTIMIZER.md`](TASKS_OPTIMIZER.md). Completed limitations are removed
rather than retained as status history.

Targets without a direct image writer fail the executable emission phase instead of falling back to an object-shaped bridge.

## Architecture

Psi operates on Omega files and owns the target-neutral pipeline from parsing
through one canonical terminal representation, including reference
interpretation. Omega consumes terminal Psi for provider installation,
optimization, and native lowering. The current Rust pipeline predates that cut
and is being migrated; `StateGraph` and `ControlFlowPlan` are not the public
portable format.

See [wiki/architecture/architecture.md](wiki/architecture/architecture.md) for a complete breakdown of the compiler architecture and pipeline.

The bootstrap architecture has the language-capability progression Alpha → Beta → Gamma → Delta → Epsilon → Omega. Alpha is raw tape execution; Beta is its textual assembly. The Epsilon-written compiler closure `D` produces the first full Omega compiler `omega₀`, which compiles the Omega-written closure `C` into production `omega`. Its active queue lives in
[`TASKS_BOOTSTRAP.md`](TASKS_BOOTSTRAP.md), while the canonical ownership map
lives in
[`repository_structure.md`](wiki/architecture/bootstrap_chain/repository_structure.md).
The fixed rung order does not freeze the present feature surfaces; the
whole-chain audit and debloat method is defined in
[`bootstrap_minimization.md`](wiki/design_briefs/bootstrap_minimization.md).
The literal Epsilon v1 contract and the incidental ordinary-Omega surface used by
the compiler source are defined and kept distinct in
[`compiler_source_profile.md`](wiki/architecture/bootstrap_chain/compiler_source_profile.md).
[`source/omega/README.md`](source/omega/README.md) describes the product-source side;
the proof kernel is Alpha-owned checker infrastructure, not another language rung.

## Samples And Language Cases

Samples are language pressure tests. They may be pseudocode-ish if the language is still being shaped.
Each sample is a copyable mini-project with its own `.gitignore`; local compiler output belongs in the ignored `build/` directory beside the entrypoint.

Current samples:

- `samples/cli/`: console and terminal-oriented programs grouped by domain, such as `basics/`, `games/`, `systems/`, and `probes/`.
- `samples/gui/`: windowed host/UI experiments, including the software-rendered calculator.
- `samples/uefi/`: firmware-targeted samples.

Language cases are not samples. They isolate one compiler capability at a time.
They live under `tests/omega/pass/<feature>/main.omg` when the compiler should accept them and `tests/omega/fail/<feature>/main.omg` plus `expected.txt` when the compiler should reject them.
Executable behavior cases live under `tests/omega/run/<feature>/` with small input/output expectation files.

Case names should describe the compiler behavior being pinned down, not the sample that exposed it. A dungeon crawler blocker should become a focused case such as `runtime_text_builder`, not `dungeon_step_04`.

Generated case `build/` directories are ignored. Permanent expectations belong in small checked-in files, not preserved build artifacts.

## Bundled Omega Packages

Imports beginning with `omega::` resolve to bundled Omega source packages under
`source/library/`.

Package paths can resolve to either `name.omg` or `name/mod.omg`, so larger packages such as `omega::host::targets::windows` can live in folders and shard their contracts by domain.

Set `OMEGA_LIBRARY_ROOT` to point at a different bundled library root when testing an installed or alternate toolchain layout.

## [READONLY] Coding Conventions

- Use real words in code. Prefer `character`, `statement`, `expression`, and `arguments` over `ch`, `stmt`, `expr`, and `args`.
- Avoid names that only make sense to compiler insiders. `pipeline` is better than `driver`; `expression` is better than `expr`.
- Keep compiler stages honest. Parse syntax, lower representation, validate semantics, plan native execution, then emit bytes.
- Keep sample coverage out of the shipped CLI. Tests and dev harnesses may discover `samples/`, but user-facing compiler behavior stays generic.
- Prefer small checkpoint commits after working improvements.
- Samples should reveal language pressure, not hide it in giant `main` files.
- Prefer arena-backed compiler data. Contiguous storage and small handles beat a pile of tiny heap allocations.
- Lowered representations should prefer `Handle<T>` and `HandleSpan<T>` over owned `Vec<T>` fields for repeated child lists.
- `Vec<T>` is fine for parser output, temporary builders, and local scratch data. It should not become the default long-lived representation shape.
- Prefer arena/vector-backed symbol tables over local hash maps. Dense lookups should collapse toward ids/handles as phases mature; hash maps need a specific sparsity or boundary reason.
- Prefer parent-owned `HandleSpan` child ranges for symbol lookup. Linear sibling scans over `HierarchyArena` child ranges are the default because real scopes are usually small and cache-friendly; global hash maps are an optimization for measured pathological scopes, not the baseline design.
- Use paged arenas for shared or eventually-parallel compiler data where growth should not move existing pages or require locking one giant `Vec`.
- Paged arenas use generational handles so reclaimed page storage cannot resurrect stale references.
- Do not use `RefCell` as an ownership escape hatch. Runtime borrow checking is not a substitute for clear compiler-phase ownership.
- Prefer ZII (Zero-is-initialization). Null handles (index 0) resolve to dummy arena entries instead of optionals and literal nulls.
- Do not wrap handles in `Optional` just to model absence. The zero handle is the absence state; `Optional<Handle<T>>` needs a semantic reason beyond “maybe missing.”
- Arena handles must be generational. Freed or stale handles resolve to dummy entries, not reused storage.
- Symbols are handle-first. String names are debug/export/import metadata, not durable identity inside semantic or native compiler layers.
- Source text is source-loading, diagnostic, and debug payload. Beyond resolution, source-backed names are technical debt unless they are literal program strings, diagnostics/debug metadata, or final-image import/export payload.
- Use stable handles when data needs references across phases; use redirect tables only when arena contents need reordering.
- Comments should explain non-obvious intent. Do not add “doing X unlike Rust” commentary unless the contrast changes implementation.

## Useful Commands

Run tests:

```bash
cargo test
```

Check a sample:

```bash
cargo run -p omega-cli -- --check samples/cli/basics/cli_mvp/main.omg
```

Compile a sample on macOS ARM64:

```bash
cargo run -p omega-cli -- --target macos_arm64 samples/cli/basics/cli_mvp/main.omg
```

Run focused compiler acceptance groups:

```bash
cargo test -p omega-compiler --test canary_suite entry_and_abi::pass_canaries_compile
cargo test -p omega-compiler --test canary_suite proof_and_float_suites::fail_canaries_reject_with_expected_diagnostic_fragment
```

## Design Notes

The language is moving quickly. The best current design references are:

- [Omega Language Guide](wiki/language_guide/language_guide.md)
- [Architecture](wiki/architecture/architecture.md)
