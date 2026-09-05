# AGENTS.md

This is the canonical instruction file for every coding agent working in this
repository. Agent-specific instruction files must point here rather than copy
these rules.

Omega is a proof-carrying systems language whose programs are data-oriented
state machines. This repository holds the language, its Rust reference
compiler, its Omega-written product compiler, and the Alpha-to-Omega bootstrap
chain.

## Commands

The Rust toolchain is pinned in `rust-toolchain.toml`; `rustup` selects it.

### Cargo wrapper

Use `mbx` in place of Cargo when available: `cargo test ...` becomes
`mbx test ...`, not `mbx cargo test ...`. If `mbx` is unavailable, use Cargo
without asking for permission.

Keep using `cargo fmt` and `cargo clean` directly; `mbx clean` has different
semantics. The examples below assume `mbx` is available.

Baseline gates for a fresh checkout:

```bash
cargo fmt --all -- --check
mbx clippy --workspace --all-targets -- -D warnings
mbx test -p omega-architecture-test --all-targets
mbx check --workspace --all-targets
mbx test --workspace --lib --no-fail-fast
```

`mbx test --workspace --lib --no-fail-fast` is the platform-portable subset:
all library tests, no target-specific executable/runtime legs. `--no-fail-fast`
is not optional. Without it the run stops at the first failing crate, which on a
Windows host is `omega-bounded-process` at position 7 of 110 lib targets, so 103
crates including every `psi_*` one never execute and the gate reports a stop
rather than a coverage gap. Platform integration tests
are separate and must report an explicit skip when the host cannot run them.

### Driving the compiler

The CLI package is **`omega`** (binary `omega`); there is no `omega-cli`
package.

```bash
mbx run -p omega -- --check samples/cli/basics/cli_mvp/main.omg
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
mbx test -p omega-compiler --test canary_suite entry_and_abi::pass_canaries_compile
```

```bash
mbx test -p omega-compiler --test canary_suite proof_and_float_suites::fail_canaries_reject_with_expected_diagnostic_fragment
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
  parsing, resolution, typing, checking, proof, selected target-neutral
  optimization, and production of **Terminal Psi**.
- **Omega** consumes Terminal Psi and owns provider selection, optimization of
  Omega-side abstract and physical representations, target realization, ABI,
  native emission, and execution machinery.
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
  Optimization stages are the exception: they normally consume and produce the
  same representation, so do not invent a `PreOptimized`/`PostOptimized` pair to
  satisfy the `X-to-Y` reading. See Optimization Phases below.
- `semantics/` — language meaning, validation, proof, interpreters.
- `backend/` — target, ABI, layout, object, linker, image.

Concepts stay visible across stages without being forced into one mega-IR: each
stage uses the form matching its resolution level while keeping stable links
back to the shared semantic spine. Coordinators stay boring — sequence typed
phases and stop. Do not add a crate until a module boundary has stopped moving.

Each program representation has one named root file beside `lib.rs`; the root
defines the current program and leads into subordinate concept-owned areas.
Organize those areas around the representation's actual control flow, values,
storage, calls, ownership, and evidence. Do not force an identical directory
template onto different representations or collect unrelated types in `model/`.
Pipeline crates own transformations and private working state, not public
program structs containing previous stage objects. Optimization history is
explicit evidence; it must not select a different downstream representation.

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
  chain Alpha → Beta → Gamma → Delta → Epsilon → Omega. Alpha is raw tape
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
an active change. Prefer small checkpoint commits after working improvements.

`wiki/architecture/bootstrap_chain/decisions.md` records decisions the owner has
ratified. A decision exists there because a human answered a question, so a
session never adds, amends, or supersedes an entry. When work meets one of D12's
escalation criteria, read D12 for the criterion's exact scope, then raise the
question in `OWNER_QUESTIONS.md` and leave the edge open until it is answered.

History on `main` stays linear. The Git for Windows system config sets
`pull.rebase` to `false`, which turns a `git pull` behind `origin/main` into a
merge commit; override it with `git config pull.rebase true`. Land a branch by
rebasing it onto `main` and fast-forwarding, not by merging.

Commit messages do not follow this repository's own `git log`. That history is
capitalized, imperative, and bodiless: it restates what the diff already shows
and drops the reasoning, which is the part no later reader can recover.

The subject is `lane: statement`. The lane names the area touched — `psi`,
`omega`, `delta`, `bootstrap`, `library`, `backend`, `docs`. The statement is
lowercase and declarative, and gives the resulting behavior rather than the
action taken. Around 68 characters, 85 at the outside. Two things landing
together join with `and`.

    delta: signed arithmetic traps at every overflow boundary

The body is prose paragraphs after a blank line, with numbers in place of
adjectives: `240.9 degrees against 194.8`, not `the phase varied`. Cover
whichever of these apply.

- What the previous behavior was and why it was wrong.
- Which alternatives were rejected, and why.
- Which constraint shaped the diff — the Psi/Omega firewall, a repository
  convention, an existing test, ZII.
- Which gates ran, named with their counts rather than the word `pass`, and
  what stayed byte-identical.
- What is known red, what was deliberately left undone, and — when one commit
  carries several changes — why the hunks could not be split.

Omit the body only when the subject already carries the full reasoning: a typo
fix, a rename, a mechanical revert.

## Prose

Mannered prose substitutes metaphor and flourish for direct statement. Instead
of "a parameter worth varying," the mannered writer produces "a dial worth
turning." Instead of "this point still matters," they write "this point earns
its keep." The phrases exist to display the writer, not to convey the idea, and
readers can tell. That is why mannered prose irritates: it makes the reader
work harder so the writer can perform. It is also imprecise. Metaphors drag in
connotations the writer did not choose and cannot control. The fix is to say
what you mean. When a literal phrase is available, use it.

## Design references

- [Omega Language Guide](wiki/language_guide/language_guide.md)
- [Architecture](wiki/architecture/architecture.md)
- [Terminal Psi Architecture](wiki/architecture/pipeline/terminal_psi.md)
- [Optimization Phases](wiki/architecture/pipeline/optimization_phases.md)
- [Rust Compiler Completion Contract](wiki/releases/rust_compiler_completion_contract.md)
