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
Windows host is `bounded-process` at position 7 of 110 lib targets, so 103
crates including every Psi one never execute and the gate reports a stop
rather than a coverage gap. Platform integration tests
are separate and must report an explicit skip when the host cannot run them.

### Developer platform support

Windows and macOS are supported development hosts; Zac works on macOS. Shared
build, test, maintenance, and landing workflows must have a documented usable
entrypoint on both. Do not assume PowerShell, Windows paths, drive letters, or
Windows-only executables are available on another developer's machine.

- Put shared behavior in one portable implementation. Prefer an existing Rust
  subcommand or a Python standard-library script when it fits the task. Thin
  `.ps1` and `.sh` launchers may call it; do not maintain independent copies of
  the protocol or business logic in each shell.
- A `.ps1` file is not sufficient evidence of macOS support. PowerShell 7 can be
  a cross-platform runtime, but `pwsh` must then be an explicit, documented
  prerequisite on both hosts, and the implementation must avoid Windows-only
  APIs. Do not silently require Zac to install PowerShell as the default answer
  to a missing macOS entrypoint. When adding or extending shared tooling,
  provide the portable route and document its runtime prerequisites.
- Document commands for both PowerShell and macOS's shell when their syntax
  differs: environment variables, quoting, line continuation, path handling,
  and exit-code checks. Use repository-relative paths and propagate failures.
  A shell wrapper that swallows a failing check is not equivalent behavior.
- Platform-specific tooling is appropriate for platform-specific work. Label
  its scope and document the corresponding host route or explicit limitation;
  do not make a Windows-only helper mandatory for an unrelated shared workflow.
- Validate shared behavior on Windows and macOS when those hosts are available,
  including failure paths. Record which host actually ran each check. A Windows
  pass or source inspection alone does not establish a macOS runtime pass; if
  macOS is unavailable, report that remaining validation explicitly.

For landing reservations, all host entrypoints must preserve the same Git
reference format, ownership checks, and atomic publication/release behavior.
A macOS route must interoperate with Windows publishers; bypassing reservations
with a direct push is not a portability workaround.

### Driving the compiler

The CLI package is **`omega`** (binary `omega`); there is no `omega-cli`
package.

```bash
mbx run -p omega -- --check samples/cli/basics/cli_mvp/main.omg
```

Full surface (`omega-rust/omega/src/command.rs`):

```text
omega [--check] [--offline] [--accept-admissions] [--output-only] [--package-root-policy <file>]
      [--build-dir <dir>] [--target <name>] [--disable-optimization <ExactName>]... <root.omg>
omega run [--both] [--keep] [--target <name>] <root.omg>
omega inspect-terminal --machine <qualified> [--target <name>] <root.omg>
omega audit source --kind <local|git> <locator> [--rev <rev>]
omega audit packages [--project <dir>] [--target <name>]... [--details] [--offline]
omega install <source> [--rev <revision>] [--package <declared-name>] [--as <alias>] [--target <name>]... [--project <dir>] [--offline]
omega update [package-or-alias...] [--to <revision>] [--target <name>]... [--project <dir>] [--offline]
omega <install|update> <--resume|--discard-review> [--project <dir>] [--offline]
omega refresh-samples [samples-dir]
```

`--offline` restricts package acquisition to local sources and cached recorded
Git pins. It does not refresh selectors or sandbox later program execution.
`run` and `inspect-terminal` do not support this flag.

Compiling writes numbered phase artifacts (`00_timings.txt` through
`14_finalization.txt`) into an ignored `build/` beside the entrypoint, or
`--build-dir`. **That directory is the primary debugging surface** — read
`06_validation.txt`, `08_proof.txt`, `09_backend_plan.txt`, and
`12_emission.txt` before instrumenting compiler code.

`OMEGA_LIBRARY_ROOT` overrides the bundled `source/library/` root when testing
an alternate toolchain layout.

### Running one test

```bash
mbx test -p compiler --test canary_suite entry_and_abi::pass_canaries_compile
```

```bash
mbx test -p compiler --test canary_suite proof_and_float_suites::fail_canaries_reject_with_expected_diagnostic_fragment
```

`canary_suite` is the umbrella target driving the `tests/omega/{pass,fail,run}`
corpus; its submodules live in `tests/canary_suite/`. Filter by module or
feature name to run a single group.

The two tests above also select corpus members from
`OMEGA_PASS_CANARY_FILTER` and `OMEGA_FAIL_CANARY_FILTER`: comma-separated
trimmed substrings matched against the `group/name` path, so `wire/` takes a
whole group. Unset runs the whole corpus, and a value matching nothing fails
the test rather than passing empty. Every other test in the target ignores
both. `OMEGA_CANARY_JOBS` overrides the outer worker count, which defaults to
host parallelism capped at 12 and must be a positive integer.

```bash
OMEGA_PASS_CANARY_FILTER=nested_parameter_receiver_call \
  mbx test -p compiler --test canary_suite entry_and_abi::pass_canaries_compile
```

`canary_suite` is not in the baseline gates above and a full run is currently
red, so a filtered run scoped to what you changed is how a failure gets
attributed.

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

Internal package and folder names omit the enclosing `omega-` or `psi-`
namespace. Keep the shipped `omega` package name. Cargo names are unique across
the workspace; use descriptive ownership names rather than duplicate generic
names (Psi's `semantic-vocabulary` and `flow-effects`, for example).

- `foundation/` — shared vocabulary, arenas, symbols, diagnostics.
- `representations/` — durable IR structs.
- `pipeline/` — transforms only; crate names read literally as `X-to-Y`
  (`source-files-to-tokens` → `tokens-to-syntax-trees` →
  `syntax-trees-to-symbol-resolved-trees` →
  `symbol-resolved-trees-to-typed-trees` →
  `typed-trees-to-checked-trees` → `checked-trees-to-lowered-psi` →
  `lowered-psi-to-lowered-psi` → `lowered-psi-to-terminal-psi`, then
  `terminal-psi-to-abstract-operations` →
  `abstract-operations-to-target-operations` →
  `target-operations-to-selected-instructions` →
  `selected-instructions-to-selected-instructions` →
  `selected-instructions-to-register-homes` → image emission).
  Optimization stages use literal `X-to-X` names: for example,
  `abstract-operations-to-abstract-operations`. They consume and produce
  the same representation; do not invent a `PreOptimized`/`PostOptimized` pair.
  The folders must expose the connected `X-to-Y`, `Y-to-Y`, `Y-to-Z` sequence,
  not merely name individually plausible calculations. See Optimization Phases.
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
- `bootstrap/{alpha,beta,gamma,delta,epsilon}/` — the trust-minimizing bootstrap
  chain Alpha → Beta → Gamma → Delta → Epsilon → Omega. Alpha is raw tape
  execution; Beta is the trusted imperative tape-assembly language; Gamma is
  the small typed functional evaluator; Delta authors the Epsilon compiler;
  Epsilon authors the first Omega compiler. Intermediate self-hosting is not a
  goal.
- `bootstrap/omega/` — the Epsilon-written first Omega compiler D. It is a
  bootstrap implementation, not the final Omega-owned product source.
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

`TASKS.md`, `TASKS_BOOTSTRAP.md`, and `TASKS_OPTIMIZER.md` are
**execution boards, not changelogs**. A task
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

Reserve only final integration and publication with `tools/landing.py`
(Python 3 and Git; no shell-specific runtime or third-party Python packages).
Develop and checkpoint in an isolated worktree first; `main` is the only shared
code branch. Join the shared FIFO queue when ready. Each promoted head has a
nonrenewable 180-second UTC lease, starting at promotion rather than claim.
Only that head may claim; use the command's local wait instead of an AI polling
loop. Expired heads are removed by the next coordinating client, which gives
the next ticket its own three minutes. Once reserved, rebase onto its returned base, run
the applicable checks locally, and publish the exact verified commit through
the command. All writers use this route rather than direct pushes to `main`.
An occupied reservation does not prevent development or reading incoming main.
Cancel a waiting ticket if no longer ready; release after a new or unexplained
gate failure before continuing implementation. Rejoining starts at the tail.
Owner-authorized baseline exceptions may proceed only with documented evidence
that the same failures occur without the change; retain the tested revisions,
commands, and results in the checkpoint. Do not relabel new failures as baseline.
Never extend a head lease or bypass the exact-reference publication checks.
Expiry is automatic; early cancellation/recovery still requires the exact
ticket/claim and the owner's direction.
See [landing](tools/landing.md) for commands,
observed-owner recovery, and handling an uncertain network result. This protocol
coordinates publishing across machines; it does not assign work ownership.

### Commit naming

Use `lane: statement`. Choose the lane from the changed responsibility below,
not the repository name, implementation language, or a prefix copied from Git
history. Omega names the whole project, a compiler stage, and a bootstrap
implementation; those meanings must stay distinct in commit subjects.

| Lane | Responsibility and current path anchors |
| --- | --- |
| `alpha`, `beta`, `gamma`, `delta`, `epsilon` | The corresponding language rung in `bootstrap/<rung>/`, including its compiler/evaluator, Rust reference tooling, and rung-specific tests. Use the actual rung name. |
| `bootstrap-omega` | The Epsilon-written first Omega compiler D in `bootstrap/omega/` and tests specifically of that implementation. Never shorten this to `omega`. |
| `bootstrap` | Cross-rung reconstruction, trust-chain edges, artifact provenance, and chain hygiene; includes `tests/bootstrap/` and shared `tools/bootstrap/` orchestration. A helper specific to one rung uses that rung's lane. |
| `psi` | Target-neutral source semantics through Terminal Psi: parsing, resolution, typing, checking, proof, interpretation, and Psi optimization in `omega-rust/psi/` or `source/psi/`. |
| `omega` | The Terminal-Psi-consuming compiler stage: Omega representations, transforms, optimization, and realization semantics in `omega-rust/omega/{representations,pipeline,semantics}/` or corresponding `source/omega/` implementation. Never a project-wide default. |
| `backend` | Target, ISA, ABI, object/image encoding, layout, and execution primitives in `omega-rust/omega/backend/` and their product-source equivalents. Transform policy in `pipeline/` remains `omega`. |
| `compiler` | Product compilation coordination and reports in `omega-rust/omega/compiler/`, or an inseparable compiler contract change spanning Psi and Omega. A coordinator call-site adjustment accompanying a stage fix keeps the stage's lane. |
| `build` | The product's build evaluation, composition, provider planning, deployment, and trust ledger in `omega-rust/omega/build/`, plus product build declarations such as `source/omega/build.omg`. Repository build commands and CI use `repo`. |
| `packages` | Package acquisition, graphs, review, admission, installation, and update workflows in `omega-rust/omega/packages/`. Package command wiring accompanying those changes keeps `packages`. |
| `cli` | Command parsing, flags, help, and command dispatch in `omega-rust/omega/src/`, plus the corresponding product entrypoint. The command's underlying compiler or package behavior uses its owning lane. |
| `tooling` | Shipped compiler support such as artifact views, profiles, language-server/docs-generator behavior, and host custody in `omega-rust/omega/tooling/`. |
| `library` | Bundled packages in `source/library/`, including `core`, `alloc`, and `std`. A compiler semantic fix exercised by library code keeps its compiler lane. |
| `samples` | Standalone sample/example content and presentation in `samples/` or example directories. A regression fixture for an implementation fix keeps that implementation's lane. |
| `tests` | Shared test infrastructure, corpus registration, fixtures, or architecture gates spanning multiple owners. Tests for one responsibility use that responsibility's lane, even in a test-only commit. |
| `tools` | Repository maintenance/development utilities in `tools/`, including landing reservations and sample-refresh scripts. Bootstrap tools follow the rung/chain rules above; shipped tooling uses `tooling`. |
| `repo` | Repository-wide workflow, agent instructions/skills, CI, workspace/dependency configuration, formatting policy, and Git settings. Includes standalone policy changes in `AGENTS.md`, `CLAUDE.md`, and `.claude/`. |
| `docs` | Explanatory documentation-only changes in `wiki/`, README files, or guides. Executable agent instructions use `repo`; documentation accompanying an implementation change keeps its implementation lane. |

Apply these selection rules in order:

1. Read the diff and identify the responsibility whose behavior or contract
   changed. The more specific table row wins over an enclosing directory:
   `omega-rust/omega/packages/` is `packages`, not `omega` or `repo`.
2. Supporting tests, fixtures, documentation, task-board updates, and mechanical
   call-site edits inherit that responsibility's lane. `tests/omega/` names the
   language corpus; a typing regression there is `psi`, an encoding regression
   is `backend`. `TASKS*.md` and `OWNER_QUESTIONS.md` follow their subject; changes
   to the board workflow itself use `repo`.
3. For independently useful changes in different lanes, split the commits.
   For one inseparable change, use the responsibility whose contract required
   the other edits. Use `compiler` for a joint Psi/Omega contract, `bootstrap`
   for a joint chain edge, `tests` for shared test machinery, and `repo` for
   repository-wide policy or mechanical maintenance. File count does not decide
   the lane. Explain the coupled areas in the body; do not invent combined
   prefixes such as `psi/omega` or use `repo` merely because many files changed.
4. New files in an existing responsibility use its existing lane. Only a new
   responsibility absent from this table needs a new lane: choose a short,
   lowercase area name, add its scope and overlap rules here in the same change,
   and explain its ownership in the body. Do not create synonyms for listed lanes.

The statement is lowercase and declarative: name the resulting behavior or
structure, rather than an instruction to the reader. Preserve case in literal
identifiers and proper names. Aim for about 68 characters for the entire subject;
85 is the maximum. Use no trailing period, ticket-only title, or generic verb
such as "update" without the concrete result. Use `and` for two coupled results;
put secondary detail in the body when the subject would exceed the limit.
Do not add a second change-type prefix such as `feat:` or `fix:`.

Examples of subject wording (not claims that these changes have landed):

    tools: landing reservations serialize integration and publication
    repo: commit lanes distinguish compiler stages from project tooling
    psi: index checks reject values outside the declared range
    backend: Windows import encoding preserves DLL casing
    packages: install selects declared Git workspace members
    bootstrap-omega: source manifests include every compiler member
    delta: signed arithmetic traps at every overflow boundary

Before committing, compare the subject and body with `git diff --cached`: the
lane must match the staged responsibility and every claimed result must be
supported by that diff or observed validation. This convention applies to future
commits; it does not authorize rewriting existing history to rename subjects.

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
