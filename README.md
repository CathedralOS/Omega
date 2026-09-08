# Omega

Omega is a proof-carrying systems language whose programs are data-oriented
state machines. This repository contains its language contracts, Rust reference
compiler, Omega-written product compiler, and trust-minimizing bootstrap chain.
The language design is broader than the compiler's implemented support.

Start with the [language guide](wiki/language_guide/language_guide.md) for
examples and the [specification index](wiki/README.md#current-specification-subjects)
for current contracts.

## Language Direction

State transitions make control flow explicit. Ownership and loans govern memory
access; contracts and proof obligations describe valid operations; capabilities
carry authority across boundaries. Layouts and target realization connect those
semantics to systems programming without making physical representation the
meaning of a value.

Safety, termination, concurrency and resource guarantees have explicit scopes
and assumptions. A logical-work bound is not a host CPU-time measurement, and
an external boundary needs its own admitted contract. See the specification
rather than treating these goals as unconditional implementation guarantees.

## Building

Install Rust through `rustup`; [rust-toolchain.toml](rust-toolchain.toml) pins
the compiler, formatter and linter. Use `mbx` for compiling commands when
available, or substitute `cargo`. Keep `cargo fmt` and `cargo clean` direct.

To request checking of the smallest CLI sample:

```bash
mbx run -p omega -- --check samples/cli/basics/cli_mvp/main.omg
```

Checking is not evidence of native execution. The
[CLI sample instructions](samples/cli/basics/cli_mvp/README.md) describe its
current boundary and host-specific build/run commands. Do not execute an old
image after a failed compilation.

Compiler observations normally go into an ignored `build/` beside the
entrypoint; `--build-dir <dir>` overrides that location. Available reports
depend on the requested product and observation policy. `--output-only`
suppresses auxiliary reports, not checking or required admission. See
[compiler products and observations](omega-rust/omega/compiler/compiler/README.md#product-boundaries-and-observations)
for the owning contract.

## Current Native Status

The native route consumes verified Terminal Psi and rejects unsupported
constructs; it has no checked-tree or source-shaped fallback. A parser,
interpreter or individual backend test does not establish end-to-end native
support for a sample or target. The
[completion plan](wiki/drafts/rust_compiler_completion.md) owns the required
acceptance matrix, and [native realization](omega-rust/omega/compiler/native-realization/README.md)
owns the implementation boundary.

Unfinished work belongs on [the compiler board](TASKS.md) and
[optimizer board](TASKS_OPTIMIZER.md), not a parallel capability ledger here.

## Architecture

[Psi](omega-rust/psi/README.md) owns source semantics through Terminal Psi.
Omega consumes that portable product for provider selection, optimization,
target realization, ABI and native emission. Target backends own unavoidable
ISA, ABI, object-format and relocation details. The
[connected pipeline](omega-rust/pipeline.md) maps transformations to code owners.

- [omega-rust/](omega-rust/README.md) is the Rust reference producer and
  differential comparator, not a canonical language rung or source of authority.
- [source/](source/README.md) holds the Omega-written product compiler.
- [bootstrap/](bootstrap/README.md) holds Alpha → Beta → Gamma → Delta →
  Epsilon → Omega and its separate proof tools.

Gamma is the small typed scalar/effect functional language with a direct Beta
evaluator. Delta authors the Epsilon evaluator; Epsilon authors the first Omega
compiler, D. D builds the Omega-written compiler, which rebuilds that same
product source. Intermediate self-hosting is not a goal. The
[bootstrap contract](bootstrap/CONTRACT.md) defines the source subjects and
required evidence; [whole-chain minimization](bootstrap/MINIMIZATION.md)
governs implementation choices.

## Samples And Language Cases

Samples are copyable language pressure tests, not a blanket support claim.
They may contain intended syntax that the compiler does not yet implement.
Browse [CLI](samples/cli), [GUI](samples/gui), or [UEFI](samples/uefi) examples;
each project's ignored `build/` owns its generated output.

Language cases isolate compiler behavior in `tests/omega/pass/<feature>/`,
`fail/<feature>/` and `run/<feature>/`. Name cases for that behavior, not the
sample that exposed it. Keep permanent expectations small and checked in;
generated build artifacts are not expectations.

## Bundled Omega Packages

Imports beginning with `omega::` resolve beneath [source/library/](source/library),
as either `name.omg` or `name/mod.omg`. `OMEGA_LIBRARY_ROOT` selects an
alternate bundled library root for toolchain-layout testing.

## Useful Commands

[AGENTS.md](AGENTS.md#driving-the-compiler) documents the CLI and
[focused canary commands](AGENTS.md#running-one-test).
[Local testing](tools/testing.md) covers nextest installation, platform
integration and affected-test selection. Choose validation from the changed
behavior; a fresh worktree alone does not require a full baseline.

## Design Notes

- [Documentation index](wiki/README.md)
- [Language guide](wiki/language_guide/language_guide.md)
- [Language and toolchain specification](wiki/README.md#current-specification-subjects)
- [Compiler ownership and pipeline](omega-rust/pipeline.md)
- [Optimizer implementation](omega-rust/optimization.md)
