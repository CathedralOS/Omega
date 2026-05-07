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

The repository is intentionally split into crates and sample folders. Names should explain the boundary. If a file name feels like compiler folklore, it probably needs another pass.

Legend:

- `[CRATE]` means a Cargo workspace package.
- Unprefixed folders are ordinary source/module boundaries inside a crate.

```text
Omega/
|-- Cargo.toml
|-- README.md
|-- omega-cli/
|   `-- [CRATE] omega-cli/                              # User-facing `omega` command.
|
|-- compiler/
|   |-- [CRATE] omega-core/                             # Cross-compiler foundations.
|   |   |-- arena/                                      # Arena, paged arena, handles, handle spans, free stack.
|   |   |-- diagnostics/                                # Diagnostic values and formatting.
|   |   `-- source/                                     # Source files, source map, resolver.
|   |
|   |-- [CRATE] omega-ast/                              # Parsed source tree structs.
|   |-- [CRATE] omega-lexer/                            # Source text to tokens.
|   |-- [CRATE] omega-parser/                           # Tokens to AST.
|   |-- [CRATE] omega-resolve/                          # Import/definition/reference reporting.
|   |-- [CRATE] omega-types/                            # Type surface and invariant constraint reporting.
|   |-- [CRATE] omega-graph/                            # Source-level machine/state graph reporting.
|   |-- [CRATE] omega-proof/                            # Source-level proof surface reporting.
|   |-- [CRATE] omega-native/                           # Source-level native surface reporting.
|
|-- omega/
|   `-- host/                                           # Bundled Omega host contracts and platform packages.
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
`-- wiki/                                               # Language design notes and guide drafts.
```

## Internal Placement Rules

These are the current rules of thumb. They are allowed to evolve, but the README should stay current when they do.

- `omega-cli` should stay thin. CLI parsing and invoking `omega_driver::check` or `omega_driver::compile` belongs there; compiler logic does not.
- `omega-driver::pipeline` owns orchestration and artifacts, not language semantics.
- `omega-driver::ir` owns lowered data structures, not parsing and not native details.
- `omega-driver::semantic` owns checks over lowered IR before native planning.
- `omega-driver::proof` owns proof obligations and proof checking.
- `omega-driver::native` owns the current native backend, but should be split aggressively as files grow.
- OS object file writers belong under `native/platform_object/`, not at the top level of native.
- Architecture instruction encoding belongs under `native/architecture/`.
- Host API/trust/platform-call lowering belongs in `native/abi.rs` and `native/host_calls.rs` for now, but should become more explicit as the host model matures.
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
