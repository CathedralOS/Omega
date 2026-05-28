# Omega

Omega is an experimental systems programming language centered around explicit
state, proof-carrying behavior, capability-aware boundaries, and data-oriented
execution.

The bet is pretty direct: state machines should not be a framework pattern
hidden inside a branch-heavy language. They should be the shape of the program.
Machines own data, states perform work, and transitions describe control-flow
handoff in a form the compiler can inspect, prove, and eventually optimize hard.

Omega is not trying to be a safer C with prettier syntax, or a Rust clone with
state machines bolted on afterward. The language is trying to make the scary
systems-programming nouns first-class: places, values, facts, loans, moves,
drops, calls, transitions, effects, authority flow, and boundary edges.

Current status: Omega is very early, but no longer purely theoretical. The
compiler can parse/check all current samples, emit small native macOS ARM64 CLI
programs as direct executable images, and writes phase artifacts for every
compiler stage. The native path is still intentionally narrow; when a feature is
not supported, the compiler should say so instead of pretending.

Tiny current program:

```omega
use omega::language::std::console;

data Main {
    console: Console;
}

machine Main::main(&mut self) {
    self.console.write_line("Hello, Omega.");
    self.console.exit_process(0);
}
```

## Language Direction

Omega is chasing a few connected ideas:

- State machines are syntax, not library objects. `machine`, `state`, and
  `transition` give control flow a durable graph shape instead of burying it in
  arbitrary branches.
- Proof is part of normal compilation. Contracts, domains, bounded values,
  borrow facts, slice bounds, termination claims, and transition obligations are
  meant to be checked before the backend gets to emit bytes.
- Authority should flow through values. Effects stay coarse and readable, while
  capabilities are tracked through values, domains, provenance, and boundary
  calls so package reports can say what code accepts, uses, derives, stores,
  returns, releases, or acquires.
- Core collections are proof surfaces. Arrays, vectors, slices, strings, and
  string views should expose browsable operators and measures such as
  `Slice::Length`, while pointer/descriptor machinery stays behind explicit
  compiler/runtime boundaries.
- Data layout matters. Omega should bias toward owned data, dense arenas,
  predictable access, SIMD-friendly transforms, and state graphs that can be
  optimized because their semantics are visible.
- Native output is a first-class goal. The compiler is growing its own path from
  source to machine code, object data, linking, and final platform images
  instead of treating executable construction as mysterious external glue.

The long-term pitch is ambitious on purpose: write programs as explicit state
evolution, let the compiler challenge the facts, and then lower the surviving
program into tight native code.

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

Build the smallest CLI sample as a direct Linux ARM64 ELF image:

```bash
cargo run -p omega-cli -- --target linux_arm64 samples/cli_mvp/main.omg
docker run --rm --platform linux/arm64 -v "$PWD:/work" -w /work alpine:3.20 ./samples/cli_mvp/build/omega-program
```

Check the richer samples:

```bash
cargo run -p omega-cli -- --check samples/dungeon_crawler_cli/main.omg
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

## Current Native Status

The native path currently supports a small but real subset:

- macOS ARM64 direct executable image emission.
- Linux ARM64 direct static ELF executable emission for tiny syscall-only programs.
- Host calls for stdout, stdin read buffers, and process exit.
- Unconditional state chains.
- Simple nested machine continuations.
- Constant integer assignment into host-call arguments.
- Static guarded transition selection for compile-time-known enum-style values.
- Static record/array/field text lowering for simple sample data.
- Enough runtime dispatch, storage, text building, and host calls to plan the dungeon crawler sample up to native emission.

Current known limitation:

- `console.read_line` on macOS is blocked before emission because the old direct Darwin stdin syscall path can produce unsafe, unkillable test binaries. The next runtime milestone is explicit line discipline through a compiler-owned buffer or a libSystem-backed host binding.
- Linux ARM64 direct ELF currently targets the small CLI path first. More runtime dispatch coverage should move over once the direct image writer grows beyond the initial syscall proof.

Targets without a direct image writer fail the executable emission phase instead of falling back to an object-shaped bridge.

## Architecture
See [wiki/architecture/architecture.md](wiki/architecture/architecture.md) for a complete breakdown of the compiler architecture and pipeline.

## Samples And Canaries

Samples are language pressure tests. They may be pseudocode-ish if the language is still being shaped.
Each sample is a copyable mini-project with its own `.gitignore`; local compiler output belongs in the ignored `build/` directory beside the entrypoint.

Current samples:

- `samples/cli_mvp/`: hello-world style CLI program.
- `samples/dungeon_crawler_cli/`: richer console game pressure test covering generation, events, combat, inventory, and runtime dispatch.

Canaries are not samples. They isolate one compiler capability at a time.
They live under `canaries/pass/<feature>/main.omg` when the compiler should accept them and `canaries/fail/<feature>/main.omg` plus `expected.txt` when the compiler should reject them.
Future executable behavior canaries live under `canaries/run/<feature>/` with small input/output expectation files.

Canary names should describe the compiler behavior being pinned down, not the sample that exposed it. A dungeon crawler blocker should become a feature canary such as `runtime_text_builder`, not `dungeon_step_04`.

Generated canary `build/` directories are ignored. Permanent expectations belong in small checked-in files, not preserved build artifacts.

## Bundled Omega Packages

Imports beginning with `omega::` resolve to bundled Omega source packages under `omega/`.

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
- Do not wrap handles in `Option` just to model absence. The zero handle is the absence state; `Option<Handle<T>>` needs a semantic reason beyond “maybe missing.”
- Arena handles must be generational. Freed or stale handles resolve to dummy entries, not reused storage.
- Symbols are handle-first. String names are debug/export/import metadata, not durable identity inside semantic or native compiler layers.
- Source text is a frontend concern. Beyond resolution, source-backed names are technical debt unless they are literal program strings, diagnostics/debug metadata, or final-image import/export payload.
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
cargo test -p omega-compiler checks_passing_canaries
cargo test -p omega-compiler rejects_failing_canaries
```

## Design Notes

The language is moving quickly. The best current design references are:

- [Omega Language Guide](wiki/language_guide/README.md)
- [Architecture](wiki/architecture/architecture.md)
