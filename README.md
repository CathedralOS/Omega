# Omega

Omega is an experimental systems programming language centered around explicit state, provable behavior, and data-oriented execution.

The core bet is that state machines should not be a library pattern bolted onto a normal branch-heavy language. They should be the shape of the program. Machines own data, states perform straight-line work, and trailing transition arrows describe the possible handoffs.

Very early sketch:

```omega
use platform::console;

machine main {
    contains console: Console;

    state Main {
        console.WriteLine("Hello, Omega.");
        console.ExitProcess(0);
    }
}
```

Omega is not stable, specified, or trying to look finished. This repository is where the language, compiler, samples, and design notes are being forced into concrete form.

## Language Direction

Omega is currently exploring a few strong ideas:

- State machines are first-class citizens, not framework objects.
- `machine main` with `state Main` is the process entry point.
- States should be branch-free by design: no hidden `if`/`else` control flow inside state bodies.
- Transitions live at the end of states as ordered `-> Target` edges.
- A bare arrow is unconditional; `when` adds the guard.
- `-> self;` re-enters the current state.
- Nested machine flow can be expressed as `-> child.Main -> Continuation;`.
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
- temporary C-host backend for the smallest CLI executable path

The current MVP can compile `samples/cli_mvp/main.omg` into a native executable through the host C compiler:

```bash
cargo run -p omega -- samples/cli_mvp/main.omg
./target/omega/main
```

Expected output:

```text
Hello, Omega.
```

The C-host backend is scaffolding, not the endgame. It exists so we can keep making executable progress while the language semantics and IR harden.

## Repository Layout

- `omega/`: Rust compiler crate and command-line entry point
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
- Do not add fake layers. A placeholder architecture is worse than no architecture.
- Keep compiler stages honest. Parse syntax, lower meaning, validate semantics, then emit code.
- Prefer small checkpoint commits after a working improvement.
- Samples should reveal language pressure, not hide it in giant `main` files.
- Avoid C++-style ambient magic. Ownership, mutation, platform boundaries, and control flow should be visible.
- Write code for the human who has to debug it at 2 AM.

If a name or abstraction makes the compiler feel like a high-school project or a PL paper cosplay, it probably needs another pass.

## Useful Commands

Run tests:

```bash
cargo test
```

Build the CLI MVP sample:

```bash
cargo run -p omega -- samples/cli_mvp/main.omg
```

Run the generated executable:

```bash
./target/omega/main
```

## Design Notes

The language is moving quickly. The best current design references are:

- [Language Vision](wiki/language-vision.md)
- [State And Transition Model](wiki/state-transition-model.md)
