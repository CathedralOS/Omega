# AGENTS.md

This is the canonical instruction file for every coding agent working in this
repository. Agent-specific instruction files must point here rather than copy
these rules. Repository skills are canonical in `.agents/skills/`; read and
edit them there. Pi and Codex discover that directory directly. Do not
maintain a second copy. For Claude Code, create an ignored checkout-local
link to the same directory:

- Windows PowerShell: `New-Item -ItemType Directory -Force .claude`, then
  `New-Item -ItemType Junction -Path .claude/skills -Target (Resolve-Path .agents/skills)`.
- macOS shell: `mkdir -p .claude`, then `ln -s ../.agents/skills .claude/skills`.

Create the link only when `.claude/skills` is absent; inspect an existing path
before replacing it. Repeat in a new checkout if using Claude there. The Windows
junction avoids administrator or Developer Mode requirements; it is local, not a
tracked symlink that Git may check out as a plain text file.

Omega is a proof-carrying systems language whose programs are data-oriented
state machines. This repository holds the language, its Rust reference
compiler, its Omega-written product compiler, and the Alpha-to-Omega bootstrap
chain.

**The goal is a compiler that works.** The measure is one straight line:
`omega run <program.omg>` on a supported host compiles the program and
executes it, and the run produces the program's authored result. Today that
line is not usable end-to-end. Work counts when a real program that
previously failed to compile or run now compiles and runs correctly. Pick
the red program nearest that line and make it run. A slot with no such
program to claim idles.

- `omega-rust/` — the Rust reference producer: the working development
  compiler and differential comparator. Not canonical, not a language rung.
- `source/psi/` + `source/omega/` — the Omega-written product compiler, split
  along the same firewall. This is the destination.
- `source/library/` — bundled `omega::` packages (`name.omg` or `name/mod.omg`).
- `bootstrap/{0_alpha,…,5_omega}/` — the trust-minimizing bootstrap chain;
  `bootstrap/proofs/` holds Gamma-written derivation checking beside the
  rungs. Bootstrap scripts resolve cross-owner locations through
  `tools/bootstrap/paths.sh` — never hard-code sibling-relative paths — and
  may invoke, stamp, compare, and report but never parse, lower, manufacture
  semantic evidence, or decide trust.

## Commands

The Rust toolchain is pinned in `rust-toolchain.toml`; `rustup` selects it.

- Use `mbx` in place of Cargo for compile and test commands: `cargo check`
  becomes `mbx check`, `cargo test` becomes `mbx nextest run` (needs
  cargo-nextest 0.9.140 or newer). If `mbx` is unavailable, use Cargo without
  asking. Nextest does not run doctests; use `mbx test --doc`.
- Keep using `cargo fmt` and `cargo clean` directly — `mbx clean` differs.
  Whole-workspace formatting is `python tools/fmt.py`: `cargo fmt --all`
  exceeds the Windows command-line limit at this workspace's size.
- Compile a program: `mbx run -p omega -- --check <root.omg>`; run it:
  `mbx run -p omega -- run <root.omg>`. The CLI package is `omega` (binary
  `omega`) — there is no `omega-cli` package. The full flag surface:
  [omega-rust/omega/application/cli/README.md](omega-rust/omega/application/cli/README.md#command-surface).
- The compiler's end-to-end test is the corpus gate:
  `python tools/corpus_gate.py --filter <group/name fragment>`; a crate's own
  Rust tests run by package and name filter
  (`mbx nextest run -p <crate> <filter>`).
- Validation policy — what a change must run, baseline gates, corpus and
  bootstrap gates, test selection, slow builds — lives in
  [tools/testing.md](tools/testing.md). Read it before running more than the
  affected crate's check.

## The Psi/Omega firewall

The load-bearing split, and the most common way to put code in the wrong
place:

- **Psi** owns everything target-neutral on Omega source: lexing, parsing,
  resolution, typing, checking, proof, target-neutral optimization, and
  production of **Terminal Psi**.
- **Omega** consumes Terminal Psi and owns provider selection, Omega-side
  optimization, target realization, ABI, native emission, and execution.
- Target backends own only unavoidable ISA, ABI, object-format, and
  relocation detail. Cathedral (the downstream OS) owns OS structures — do
  not model page tables, schedulers, or drivers as compiler-owned types.

Terminal Psi is the only portable boundary. Targets are data, not control
flow: Psi checks every target-scoped machine body in every compilation, and
`--target` narrows what Omega realizes, never what Psi checks. The full
architecture contract — deferred Psi audit policy, crate placement,
discoverability, compositional lowering — lives in
[omega-rust/pipeline.md](omega-rust/pipeline.md).

Psi groups compilation machinery under `compilation/`, execution engines under
`interpreter/`, and shared primitives under `foundation/`. Compile-time admission
and result elaboration belong to `compilation/compile-time/`, not an interpreter.
The product still has one driver; these domain folders do not create another.

Durable IRs stay in independent `compilation/representations/` packages grouped by actual
pipeline program family. Shared vocabulary, target/ABI descriptions, planning
records and standalone evidence belong to their foundation, checking or backend
owners; they are not extra IR stages. Shared foundations and utilities stay
independently owned. Do not absorb them into producing
stages or the application crate; the owner rejected that consolidation on
2026-09-26.

## Conventions and workflow

The rules that touch every edit:

- Real words in code (`character`, `expression`, `arguments` — not `ch`,
  `expr`, `args`). Names must read outside compiler insiders: `pipeline`
  over `driver`.
- Arena-backed data with generational handles over scattered heap objects;
  ZII — the zero handle is the absence state, so do not wrap handles in
  `Optional`; no `RefCell` as an ownership escape. Durable lowered children
  use `Handle<T>`/`HandleSpan<T>`; `Vec` is construction scratch, not the
  durable shape. Arena/vector-backed symbol tables over local hash maps; a
  map keyed by compiler-owned objects is a review trigger for missing
  representation structure. Symbols are handle-first — string names are
  debug/export payload, not identity. Source text stays in loading,
  diagnostic, and debug payload roles.
- Entrypoints stay prominent: `main.rs` owns startup and visible dispatch
  to real subsystems. Split at responsibility boundaries.
- Comments explain non-obvious intent; `lib.rs` headers carry the reasoning
  signatures cannot — mechanism, justification, honest boundaries,
  provenance.
- Commit subjects are `lane: statement` — pick the lane by changed
  responsibility per [commit naming](#commit-naming) below.
- `TASKS.md`, `TASKS_BOOTSTRAP.md`, `TASKS_OPTIMIZER.md` are execution
  boards, not changelogs: an item exists only while it names unfinished work
  and is deleted when acceptance passes. An open item states its current
  frontier once — replace the paragraph, do not append run history. New
  items carry `(new-scope)` or `(split-of:<parent>)` on the first line.
  Owner decisions go to `OWNER_QUESTIONS.md`, not the boards.
  Task lists are shared: claim individual items and implementation paths,
  never a whole board file. Update only the relevant items and reconcile
  concurrent board edits during landing; nobody owns the task list.
- Windows and macOS are both supported development hosts; shared tooling
  needs a documented usable entrypoint on each — a `.ps1` alone is not
  macOS support; either document `pwsh` as an explicit prerequisite or
  provide the portable route. Omega is the intended implementation language
  for tooling; existing Python/shell implementations are removed when Omega
  replaces them.
- Language fixtures name the compiler behavior they pin
  (`tests/omega/{pass,fail,run}/<behavior>/`), not the sample that exposed
  it; `samples/` are language pressure tests; a Rust test of one crate stays
  beside that crate.
- Write direct prose: state the idea in literal words.
- Read the matching skill before starting: `.agents/skills/` — `advance` for
  board work, `rust-systems-programmer` for Rust implementation and review,
  `architecture-cleanup`, `task-board-cleanup`, `local-swarm`, `devin-swarm`,
  `codex-swarm`.

Deliberate configuration — do not "fix" it: `clippy.toml` thresholds are
raised and dev/test profiles use `debug = 0`. Tests use `opt-level = 1`;
ordinary dev builds are unoptimized except `proof-admission`, which uses
`opt-level = 3`. Check `Cargo.toml` and session overrides before interpreting
compiler timings. Use `CARGO_PROFILE_DEV_OPT_LEVEL=1` for an optimized dev
compiler comparison without changing repository defaults. Debug information
can be restored with `CARGO_PROFILE_DEV_DEBUG=2` or `CARGO_PROFILE_TEST_DEBUG=2`;
`CARGO_PROFILE_TEST_OPT_LEVEL=0` disables test optimization for a session.
`.gitattributes` forces LF because canonical source and evidence identities
are byte-sensitive.

## Scope and escalation

- Before a milestone, name the program it will get running and the
  dependencies still missing. Every change traces to a program that runs
  because of it.
- A session ends in one of two states: a program runs that did not, or the
  session is blocked on a named dependency and idle. Report which.
- Language/architecture decisions go in `OWNER_QUESTIONS.md` with the
  dependency referenced on the owning task; engineering decisions stay with
  the implementer. Gates pass on real behavior. The accepted bootstrap
  chain changes only through its proofs.

## Delegation

- Dot-created Omega cloud sessions and their native workers default to
  **GPT 6.1 Sol, medium reasoning** (`gpt-6.1-sol`, `medium`), unless the user
  specifies another model or effort. Select them through the runtime's exposed
  launch fields; a prompt alone cannot change a model. Before launching or
  resuming cloud work, follow the [cloud launch preflight](.agents/skills/dot-swarm/references/cloud-launch.md).
  A single advance remains one assignment; it does not authorize a swarm.
- One edit owner per change; parallelize only independent work.
  Assignments carry the objective, governing design links, ownership
  boundary, dependencies, and the acceptance command.
- Workers report through `python tools/claims.py note --ticket` and the
  session report; the coordinator integrates its workers' board updates,
  without claiming the board or blocking other sessions' updates. A slice
  that reaches a sibling-owned implementation path stops there and reports it.
- Continue implementation when the design answers the question; hand off
  the exact missing dependency when it does not.

## Commit naming

Use `lane: statement`. Choose the lane from the changed responsibility
below — not the repository name, implementation language, or a prefix
copied from Git history. Omega names the project, a compiler stage, and a
bootstrap implementation; those meanings stay distinct in subjects.

| Lane | Responsibility and current path anchors |
| --- | --- |
| `alpha`, `beta`, `gamma`, `delta`, `epsilon` | The corresponding language rung in `bootstrap/<rung>/`, including its compiler/evaluator, Rust reference tooling, and rung-specific tests. Use the actual rung name. |
| `bootstrap-omega` | The Epsilon-written first Omega compiler D in `bootstrap/5_omega/` and tests specifically of that implementation. Never shorten this to `omega`. |
| `bootstrap` | Cross-rung reconstruction, trust-chain edges, artifact provenance, and chain hygiene; includes `tests/bootstrap/` and shared `tools/bootstrap/` orchestration. A helper specific to one rung uses that rung's lane. |
| `psi` | Target-neutral source semantics through Terminal Psi: parsing, resolution, typing, checking, proof, interpretation, and Psi optimization in `omega-rust/psi/` or `source/psi/`. |
| `omega` | The Terminal-Psi-consuming compiler stage: Omega representations, transforms, optimization, and realization semantics in `omega-rust/omega/compilation/{representations,pipeline,realization}/` or corresponding `source/omega/` implementation. Never a project-wide default. |
| `backend` | Target, ISA, ABI, object/image encoding, layout, and execution primitives in `omega-rust/omega/compilation/backend/` and their product-source equivalents. Transform policy in `pipeline/` remains `omega`. |
| `compiler` | Product compilation coordination and reports in `omega-rust/omega/compilation/driver/`, or an inseparable compiler contract change spanning Psi and Omega. A coordinator call-site adjustment accompanying a stage fix keeps the stage's lane. |
| `build` | The product's Build activation in `omega-rust/omega/compilation/build-evaluation/`, plus product build declarations such as `source/omega/build.omg`. Provider planning and trust admission under `realization/` use `omega`; CLI policy-file handling uses `cli`. Repository build commands and CI use `repo`. |
| `packages` | Package acquisition, graphs, review, admission, installation, and update workflows in `omega-rust/omega/packages/`. Package command wiring accompanying those changes keeps `packages`. |
| `cli` | Command parsing, flags, help, and command dispatch in `omega-rust/omega/application/cli/src/`, plus the corresponding product entrypoint. The command's underlying compiler or package behavior uses its owning lane. |
| `tooling` | Shipped developer tools and shared host support in `omega-rust/omega/foundation/`, including artifact file writing and measurements. Semantic trust records and admission under `realization/trust-model/` use `omega`. |
| `library` | Bundled packages in `source/library/`, including `core`, `alloc`, and `std`, and their Rust reference implementations in `omega-rust/library/`. A compiler semantic fix exercised by library code keeps its compiler lane. |
| `samples` | Standalone sample/example content and presentation in `samples/` or example directories. A regression fixture for an implementation fix keeps that implementation's lane. |
| `tests` | Shared test infrastructure, corpus registration, fixtures, or architecture gates spanning multiple owners. Tests for one responsibility use that responsibility's lane, even in a test-only commit. |
| `tools` | Repository maintenance/development utilities in `tools/`, including landing reservations and sample-refresh scripts. Bootstrap tools follow the rung/chain rules above; shipped tooling uses `tooling`. |
| `repo` | Repository-wide workflow, agent instructions/skills, CI, workspace/dependency configuration, formatting policy, and Git settings. Includes standalone policy changes in `AGENTS.md` and `.claude/`. |
| `docs` | Explanatory documentation-only changes in `wiki/`, README files, or guides. Executable agent instructions use `repo`; documentation accompanying an implementation change keeps its implementation lane. |

Apply these selection rules in order:

1. Read the diff and identify the responsibility whose behavior or contract
   changed. The more specific table row wins over an enclosing directory:
   `omega-rust/omega/packages/package-manager/` is `packages`, not `omega` or `repo`.
2. Supporting tests, fixtures, documentation, task-board updates, and
   mechanical call-site edits inherit that responsibility's lane.
   `tests/omega/` names the language corpus; a typing regression there is
   `psi`, an encoding regression is `backend`. `TASKS*.md` and
   `OWNER_QUESTIONS.md` follow their subject; changes to the board workflow
   itself use `repo`.
3. For independently useful changes in different lanes, split the commits.
   For one inseparable change, use the responsibility whose contract
   required the other edits. File count does not decide the lane; do not
   invent combined prefixes such as `psi/omega` or use `repo` merely
   because many files changed.
4. New files in an existing responsibility use its existing lane. Only a
   new responsibility absent from this table needs a new lane: choose a
   short lowercase area name, document its scope here in the same change,
   and explain its ownership in the body. Do not create synonyms for
   listed lanes.

The statement is lowercase and declarative: name the resulting behavior or
structure. Aim for about 68 characters for the whole subject; 85 is the
maximum. No trailing period, ticket-only title, or bare `update`. No second
change-type prefix such as `feat:` or `fix:`.

The body is prose paragraphs after a blank line, with numbers in place of
adjectives. Cover what applies: what the previous behavior was and why it
was wrong; which alternatives were rejected; which constraint shaped the
diff; which gates ran, named with counts rather than the word `pass`; what
is known red or deliberately left undone. Omit the body only when the
subject carries the full reasoning (typo, rename, mechanical revert).

Commit messages carry no tool attribution: no "Generated with ..." line, no
`Co-Authored-By` trailer naming an agent or vendor.

## Hard rules

- **Do not add required feature-combination special cases. Clean as you go.**
  Generalize the touched operation or representation and remove its superseded
  path in the same change; do not land another source-shape recognizer, fallback
  or mode with cleanup deferred to a task. Preserve genuine semantic and ABI
  distinctions and independent verification. Optional optimization specialization
  requires a correct general path. See [compositional lowering](omega-rust/pipeline.md#compositional-lowering).
- **Never push to `main` directly.** Reserve and publish through
  `tools/landing.py` ([tools/landing.md](tools/landing.md)); history is
  linear — rebase onto `main` and fast-forward, never merge
  (`git config pull.rebase true`). Once a publish completes, remove the
  worktree and delete the candidate branch — a landed lane is dead
  bookkeeping, not a record.
- Before editing shared paths, check `python tools/claims.py status` for
  live session assignments and attempt to claim the work ([tools/claims](tools/claims.md)).
  Claims are advisory, not permission to develop. If coordination is unavailable
  or times out, report it and continue in an isolated worktree with the intended
  scope recorded locally. Do not wait repeatedly, treat an outage as a known
  conflict, or claim registration succeeded. Known live overlaps still require
  coordination or owner permission. Publication still uses the landing queue.
- Keep sample coverage out of the shipped CLI.

## Design references

- [Omega Language Guide](wiki/language_guide/language_guide.md)
- [Compiler ownership and pipeline](omega-rust/pipeline.md)
- [Documentation index](wiki/README.md)
- [Terminal Psi product contract](wiki/spec/terminal-psi/product.md)
- [Optimization phases](wiki/spec/build/optimizations.md#phase-and-product-boundaries)
- [Rust compiler completion](wiki/drafts/reference/rust_compiler_completion.md)
