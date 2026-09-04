# CLAUDE.md

Mannered prose substitutes metaphor and flourish for direct statement. Instead of "a parameter worth varying," the mannered writer produces "a dial worth turning." Instead of "this point still matters," they write "this point earns its keep." The phrases exist to display the writer, not to convey the idea, and readers can tell. That is why mannered prose irritates: it makes the reader work harder so the writer can perform. It is also imprecise. Metaphors drag in connotations the writer did not choose and cannot control. The fix is to say what you mean. When a literal phrase is available, use it.

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

Omega is a proof-carrying systems language whose programs are data-oriented
state machines. This repository holds the language, its Rust reference
compiler, its Omega-written product compiler, and the Alpha-to-Omega bootstrap
chain.

## Commands

The toolchain is pinned in `rust-toolchain.toml`; `rustup` selects it. Baseline
gates for a fresh checkout:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p omega-architecture-test --all-targets
cargo check --workspace --all-targets
cargo test --workspace --lib
```

`cargo test --workspace --lib` is the platform-portable subset: all library
tests, no target-specific executable/runtime legs. Platform integration tests
are separate and must report an explicit skip when the host cannot run them.

### Driving the compiler

The CLI package is **`omega`** (binary `omega`). The README's
`cargo run -p omega-cli` is stale; that package does not exist.

```bash
cargo run -p omega -- --check samples/cli/basics/cli_mvp/main.omg
```

Full surface (`omega-rust/omega/src/command.rs`):

```text
omega [--check] [--accept-admissions] [--output-only] [--package-root-policy <file>]
      [--build-dir <dir>] [--target <name>] [--disable-optimization <ExactName>]... <root.omg>
omega run [--both] [--keep] [--target <name>] <root.omg>
omega inspect-terminal --machine <qualified> [--target <name>] <root.omg>
omega audit source --kind <local|git> <locator> [--rev <rev>]
omega refresh-samples [samples-dir]
```

Compiling writes numbered phase artifacts (`00_timings.txt` through
`14_finalization.txt`) into an ignored `build/` beside the entrypoint, or
`--build-dir`. **That directory is the primary debugging surface** — read
`06_validation.txt`, `08_proof.txt`, `09_backend_plan.txt`, and
`12_emission.txt` before instrumenting compiler code.

`OMEGA_LIBRARY_ROOT` overrides the bundled `source/library/` root when testing
an alternate toolchain layout.

### Running one test

```bash
cargo test -p omega-compiler --test canary_suite entry_and_abi::pass_canaries_compile
```

```bash
cargo test -p omega-compiler --test canary_suite proof_and_float_suites::fail_canaries_reject_with_expected_diagnostic_fragment
```

`canary_suite` is the umbrella target driving the `tests/omega/{pass,fail,run}`
corpus; its submodules live in `tests/canary_suite/`. Filter by module or
feature name to run a single group.

### Bootstrap gates

Bootstrap gates are `sh` scripts, not Cargo tests, and self-skip when `python3`
is absent:

```bash
sh tools/bootstrap/check-chain-hygiene.sh
```

```bash
sh tests/bootstrap/alpha-beta-edge.sh --edge
```

The first is the single repository-topology gate. The second is the currently
closed bootstrap floor. There is deliberately no wrapper that pretends to run
the whole chain.

### Slow builds

If small crates each pause for seconds before parsing, inspect `target/` before
changing test or compiler architecture: a long-lived `target/debug/deps` with
hundreds of thousands of stale hashed artifacts makes rustc rescan it per
crate. `cargo clean` fixes it.

## Architecture

Full breakdown: [architecture.md](wiki/architecture/architecture.md) and
[repository_layout.md](wiki/architecture/repository_layout.md).

### The Psi/Omega ownership firewall

This is the load-bearing split, and the most common way to put code in the
wrong place:

- **Psi** operates on Omega source and owns everything target-neutral: lexing,
  parsing, resolution, typing, checking, proof, and production of **Terminal
  Psi**.
- **Omega** consumes Terminal Psi and owns provider selection, optimization,
  target realization, ABI, native emission, and execution machinery.
- Target backends own only unavoidable ISA, ABI, object-format, and relocation
  detail.
- Cathedral (the downstream OS) owns OS data structures, policies, protocols,
  and lifecycle. Do not model page tables, schedulers, or drivers as
  compiler-owned Rust types; if Cathedral cannot express something, name the
  missing general Omega primitive or mark the slice blocked.

Terminal Psi is the only portable boundary. `StateGraph` and `ControlFlowPlan`
predate that cut and are **not** the public portable format.

### Crate placement rule

Workspace crate names encode the pipeline. Within both halves:

- `foundation/` — shared vocabulary, arenas, symbols, diagnostics.
- `representations/` — durable IR structs.
- `pipeline/` — transforms only; crate names read literally as `X-to-Y`
  (`psi-source-files-to-tokens` → `psi-tokens-to-syntax-trees` →
  `psi-syntax-trees-to-symbol-resolved-trees` →
  `psi-symbol-resolved-trees-to-typed-trees` →
  `psi-typed-trees-to-checked-trees` → `psi-checked-trees-to-terminal`, then
  `omega-psi-to-abstract-operations` →
  `omega-abstract-operations-to-target-operations` →
  `omega-target-operations-to-selected-instructions` → image emission).
- `semantics/` — language meaning, validation, proof, interpreters.
- `backend/` — target, ABI, layout, object, linker, image.

Concepts stay visible across stages without being forced into one mega-IR: each
stage uses the form matching its resolution level while keeping stable links
back to the shared semantic spine. Coordinators stay boring — sequence typed
phases and stop. Do not add a crate until a module boundary has stopped moving.

`tests/architecture` (`omega-architecture-test`) enforces cross-crate
dependency direction and semantic shape. A wrong-direction dependency fails
there, not at `cargo check`.

### Source trees

- `omega-rust/` — the **Rust reference producer**. Working development
  compiler and differential comparator; explicitly not canonical, not a
  language rung, and it grants no authority.
- `source/psi/` + `source/omega/` — the Omega-written product compiler, split
  along the same firewall. This is the destination.
- `source/{alpha,beta,gamma,delta,epsilon}/` — the trust-minimizing bootstrap
  lattice Alpha → Beta → Gamma → Delta → Epsilon → Omega. Alpha is raw tape
  execution; Beta is the trusted imperative tape-assembly language; Gamma is
  the small typed functional evaluator; Delta authors the Epsilon compiler;
  Epsilon authors the first Omega compiler. Intermediate self-hosting is not a
  goal.
- `source/library/` — bundled Omega packages; `omega::` imports resolve here,
  as either `name.omg` or `name/mod.omg`.

Bootstrap scripts resolve cross-owner locations through the role manifest in
`tools/bootstrap/paths.sh`. **Never hard-code sibling-relative paths in a
bootstrap script.** Host scripts may invoke, stamp, compare, and report; they
may not parse, lower, manufacture semantic evidence, or decide trust.

## Repository conventions

The `[READONLY] Coding Conventions` section of [README.md](README.md) is
authoritative. The ones a default Rust instinct will violate:

- **Real words.** `character`, `statement`, `expression`, `arguments` — never
  `ch`, `stmt`, `expr`, `args`. `pipeline`, not `driver`.
- **Arena-backed, handle-first.** Lowered representations use `Handle<T>` and
  `HandleSpan<T>` for repeated child lists, not owned `Vec<T>`. `Vec<T>` is for
  parser output, temporary builders, and local scratch.
- **ZII.** The zero handle (index 0) resolves to a dummy arena entry and *is*
  the absence state. Do not wrap handles in `Optional` to model "maybe
  missing"; that needs a semantic reason beyond absence.
- **Generational handles.** Freed or stale handles resolve to dummy entries,
  never reused storage.
- **No `RefCell` as an ownership escape hatch.** Runtime borrow checking does
  not substitute for clear compiler-phase ownership.
- **Arenas over hash maps.** Linear sibling scans over `HierarchyArena` child
  ranges are the baseline for symbol lookup; hash maps need a measured sparsity
  or boundary reason.
- **Symbols are handle-first.** String names are debug/export/import metadata,
  not durable identity inside semantic or native layers.

Configuration that looks wrong but is deliberate:

- `clippy.toml` thresholds are raised on purpose (ownership and custody APIs
  return the original authority on failure; syntax and proof enums retain full
  structure). Do not "fix" a large error type or enum variant to satisfy a
  default lint.
- `debug = 0` in `[profile.dev]` and `[profile.test]` is intentional; opt back
  in per-session with `CARGO_PROFILE_TEST_DEBUG=2` or
  `CARGO_PROFILE_DEV_DEBUG=2`.
- `.gitattributes` forces LF because canonical source and evidence identities
  are byte-sensitive. Windows launchers (`.bat`, `.cmd`) keep CRLF.

## Tests and samples

- **Language cases** live in `tests/omega/pass/<feature>/main.omg`,
  `tests/omega/fail/<feature>/main.omg` plus `expected.txt`, and
  `tests/omega/run/<feature>/` with small input/output expectation files. Name
  a case for the **compiler behavior** it pins down, not the sample that
  exposed it: `runtime_text_builder`, not `dungeon_step_04`. Permanent
  expectations are small checked-in files, never preserved build artifacts.
- **Samples** (`samples/cli|gui|uefi/`) are language pressure tests, one
  copyable mini-project each. They may be pseudocode-ish while the language is
  being shaped, and should reveal pressure rather than hide it in a giant
  `main`. Sample coverage stays out of the shipped CLI.
- Rust tests whose subject is one crate stay beside that crate. The root
  `tests/` tree is only for repository-, language-, or multi-package-wide
  validation.

## Workflow

`TASKS.md`, `TASKS_BOOTSTRAP.md`, `TASKS_OPTIMIZER.md`, and
`TASKS_PACKAGE_MANAGER.md` are **execution boards, not changelogs**. A task
stays only while it names unfinished work, its owning code/design area, any
real blocker, and a concrete acceptance condition. Remove it when acceptance
passes — do not append landed substeps, version history, test counts, or
release notes. Completed limitations are deleted, not retained as status.

Owner decisions belong in `OWNER_QUESTIONS.md`, not on a board. Before starting
work, fetch `main` and inspect recent commits in that lane to avoid overlapping
an active change. Prefer small checkpoint commits after working improvements,
matching the terse imperative subject style in `git log`.

## Design references

- [Omega Language Guide](wiki/language_guide/language_guide.md)
- [Architecture](wiki/architecture/architecture.md)
- [Terminal Psi Architecture](wiki/architecture/pipeline/terminal_psi.md)
- [Rust Compiler Completion Contract](wiki/releases/rust_compiler_completion_contract.md)
