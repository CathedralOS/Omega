# Omega

Omega is an experimental systems programming language centered around explicit state, provable behavior, and data-oriented execution.

The core bet is that state machines should not be a library pattern bolted onto a normal branch-heavy language. They should be the shape of the program. Machines own data, states perform straight-line work, and trailing transition arrows describe the possible handoffs.

Very early sketch:

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

Omega is not stable, specified, or trying to look finished. This repository is where the language, compiler, samples, and design notes are being forced into concrete form.

## Language Direction

Omega is currently exploring a few strong ideas:

- State machines are first-class citizens, not framework objects.
- `machine main` with `state entry` is the process entry point.
- States should be branch-free by design: no hidden `if`/`else` control flow inside state bodies.
- Transitions live at the end of states as ordered `-> target` edges.
- A bare arrow is unconditional; `when` adds the guard.
- `-> self;` re-enters the current state.
- Nested machine flow can be expressed as `-> child.entry -> continuation;`.
- Commands perform explicit work but do not imply return-value control flow.
- Data flow should prefer explicit owned data and `mut` parameters over invisible stacks or ambient state.
- Platform boundaries are explicit and stub-friendly.

Longer term, Omega wants compile-time proof integration. TLA+ style transition checks are a design goal, not a decoration. The compiler should eventually be able to derive a formal transition model from source, challenge invariants and liveness properties, and only then lower the program.

Performance is part of the language design too. Omega should bias toward dense data, predictable access, SIMD-friendly transforms, and state graphs that can be optimized aggressively. One experimental direction is patching control flow for hot transitions instead of carrying permanent branches, though that needs serious proof, safety, portability, and debugging constraints before it becomes real.

## Compiler Status

The Rust compiler is intentionally small, but it now has real pipeline seams:

- lexer with spans and structured errors
- parser for top-level items, machines, platforms, states, command calls, and transition arrows
- AST-to-IR lowering
- semantic validation for entry point, receivers, platform commands, and transition targets

Omega does not currently emit a native binary. That is deliberate: the old C-host path was removed rather than letting a throwaway transpiler shape the language.

The current MVP can check all samples through the real front-end pipeline:

```bash
cargo run -p omega-cli -- --check samples/cli_mvp/main.omg
cargo run -p omega-cli -- --check samples/dungeon_crawler_cli/main.omg
cargo run -p omega-cli -- --check samples/point_and_click/main.omg
```

Native binary emission should come from a real Omega backend or execution model, not from pretending C is the architecture.

## Repository Layout

- `omega-core/`: shared compiler foundations such as arenas, diagnostics, source files, and spans
- `omega-syntax/`: lexer, parser, tokens, and AST
- `omega-compiler/`: IR, semantic validation, native planning, and compile driver
- `omega-cli/`: command-line entry point, with binary name `omega`
- `samples/`: small Omega programs used to pressure-test language ideas
- `wiki/`: evolving design notes for semantics, proof shape, and state-machine behavior

Notable samples:

- `samples/cli_mvp/`: smallest console program, no input
- `samples/dungeon_crawler_cli/`: console input/output, nested machine flow, data-driven rooms
- `samples/point_and_click/`: windowed game sketch, room ownership, render loop boundaries

## Development Philosophy

Omega is allowed to be strange, but the codebase should not be sloppy.

Some repo-level taste:
- Use real words in code. Prefer `character`, `statement`, `expression`, and `arguments` over `ch`, `stmt`, `expr`, and `args`.
- Keep compiler stages honest. Parse syntax, lower meaning, validate semantics, then emit code.
- Prefer small checkpoint commits after a working improvement.
- Samples should reveal language pressure, not hide it in giant `main` files.
- Prefer arena-backed compiler data. Contiguous storage and small handles beat a pile of tiny heap allocations.
- Prefer arena/vector-backed symbol tables over local hash maps. Names should collapse toward ids/handles as phases mature.
- Use paged arenas for shared or eventually-parallel compiler data where growth should not move existing pages or require locking one giant `Vec`.
- Paged arenas use generational handles so reclaimed page storage cannot resurrect stale references.
- Do not use `RefCell` as an ownership escape hatch. Runtime borrow checking is not a substitute for clear compiler-phase ownership.
- Prefer ZII (Zero-is-initialization). Null handles (index 0) instead of optionals and literal nulls.
- Use stable handles when data needs references across phases; use redirect tables only when arena contents need reordering.

If a name or abstraction makes the compiler feel like a high-school project or a PL paper cosplay, it probably needs another pass.

## Useful Commands

Run tests:

```bash
cargo test
```

Check the CLI MVP sample:

```bash
cargo run -p omega-cli -- --check samples/cli_mvp/main.omg
```

Check richer samples:

```bash
cargo run -p omega-cli -- --check samples/dungeon_crawler_cli/main.omg
cargo run -p omega-cli -- --check samples/point_and_click/main.omg
```

## Design Notes

The language is moving quickly. The best current design references are:

- [Language Vision](wiki/language-vision.md)
- [State And Transition Model](wiki/state-transition-model.md)
- [Omega Language Guide](wiki/language_guide/README.md)
