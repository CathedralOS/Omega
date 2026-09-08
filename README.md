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
