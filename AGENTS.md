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

Omega is a proof-checked systems language whose programs are data-oriented
state machines. This repository holds the language, its Rust reference
compiler, its Omega-written product compiler, and the Alpha-to-Omega bootstrap
chain.

**The goal is a compiler that ships.** Take a sample program end to end, not
a file or a stage ([the sample loop](#the-sample-loop)). Demonstrate compiler work through real programs and
relevant invalid controls; `omega run <program.omg>` must produce the authored
result. Squalr and Cathedral expose general compiler gaps, not special cases
to recognize or applications to rewrite around missing language support.
Iteration performance and the test environment are also first-class outcomes
when a measured workflow obstructs development. They need a concrete acceptance
and stopping point, not an open-ended optimization or testing campaign. A step
that makes the sample-to-native loop slow is a workflow defect: measure where
the time goes and remove the cause rather than waiting on it or working around it.

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

## Law of completion

Our superhuman advantage is carrying the entire difficult task end to end.
Difficulty is a reason to investigate and persist, not to abandon the work,
lose confidence without evidence, or redefine done. Confidence is demonstrated
by working behavior, not by declaring success.

We are capable of taking an uncompilable sample through every necessary compiler
layer to a compilable, executable, validated program landed on main. The agent
owns that entire transformation, not just the first repair. Treat the hard
remaining steps as engineering to finish, not evidence that the task is beyond
reach.

Follow the river to its end; do not carve every possible branch along the way.
Working paths reveal the design. Build and validate what this journey needs;
adjacent journeys will reveal their own needs when undertaken.

Unfinished work loses the context that made it tractable when the session
doing it stops. The session that starts a piece of work therefore carries it
through acceptance, integration, required review, authorized landing and
remote verification on main. A checkpoint, component pass, archive, JSON
report or handoff document preserves evidence; none completes the work.
There are no handoffs: nobody else is waiting to finish it.

The working session finishes the entire path, including the necessary
compiler, library and prerequisite repairs. Do not manufacture dependencies on
other agents by dividing work into stages or waiting for another session to
finish missing functionality. Reuse available repairs when practical;
otherwise continue the necessary work in the isolated worktree. Overlapping
edits and merge conflicts are expected and reconciled at rebase.

When the user authorizes end-to-end publication, the session lands the
completed path through the normal landing protocol; returning a partial
candidate for someone else to finish is not a substitute. Do not publish
half-finished functionality or an unfinished workflow. Read-only
investigations remain read-only.

Persistence does not authorize fabricated results, narrowed customer acceptance,
permission changes, custody bypasses or weakened review and validation gates.
Raise genuine external authorization or environment blockers precisely, keep
the work marked unfinished, and continue all other feasible engineering. A
user stop or exhausted runtime budget preserves the work for a later
continuation; it does not turn unfinished work into completion.

## The sample loop

Compiler work is one loop, and its unit is a sample: a runnable program under
`samples/` whose header documents `Expected exit` or `Expected output contains`.

1. **Take a failing sample.** `python3 tools/sample_queue.py` lists the
   samples that fail on this host in the latest sample-gate run, smallest
   first, with each one's refusal; run `python3 tools/sample_native_gate.py`
   with `--programs-only --find-work` first when no recent run exists; the full release
   gate still includes probes and controls. A sample the user names comes first. A
   sample that cannot run until the owner decides something says so in its
   README, citing the `OWNER_QUESTIONS.md` entry; the queue lists it apart
   from the work.
2. **Make it run.** Done means the sample compiles and runs natively with its
   authored result on main, with every compiler, library and fixture repair
   it needed landed with it, and its invalid controls still rejecting.
   Nothing short of that is done: a stage, checkpoint, diagnosis or board edit
   is progress, not completion.
3. **Keep it small enough to finish.** A piece of work is sized to finish in
   one session. When a sample is too large to trace (its size hides the
   failing construct, or its first failure leads into an unrelated second
   gap), cut it down: write a smaller sample that isolates the failing
   construct with its own oracle, make that run, land it, then return to the
   large one. Large programs advance as a ladder of small runnable samples,
   never as stage tasks.
4. **When it already works, go harder.** A sample that passes on its first
   run teaches nothing. Take the next failing sample, or write a harder one at
   the language frontier: a construct, combination or scale no passing sample
   uses yet. Its failure is the next work.

The repair inside the loop is ordinary compiler engineering and follows every
rule below: generalize the operation, check mechanical variants, pin the
behavior with fixtures under `tests/omega/`, and keep main green.

A change that makes the language reject source it used to accept migrates
every sample and fixture it breaks in the same change. Run the sample gate on
the affected samples before landing; a sample left failing by a language
change becomes someone else's queue entry with no compiler work in it.

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
The product has one visible route composed from two driver facades: Psi produces
Terminal Psi, then Omega realizes it. These domain folders do not create another
driver, and consumers do not assemble stages behind either facade.

Durable IRs stay in independent `compilation/representations/` packages grouped by actual
pipeline program family. Shared vocabulary, target/ABI descriptions, planning
records and standalone evidence belong to their foundation, checking or backend
owners; they are not extra IR stages. Shared foundations and utilities stay
independently owned. Do not absorb them into producing
stages or the application crate; the owner rejected that consolidation on
2026-09-26.

## Conventions and workflow

The rules that touch every edit:

- Data and code are independent: named implementations satisfy traits; data
  does not. Trait data types are ordinary explicit parameters. There is no
  conformance subject, `Self` type binding, or implicit `self` value. `dyn`
  selects an implementation at runtime; data arguments stay separate. Authored
  implementations declare member machines inline; reuse is an ordinary call,
  not a `Trait::member = Existing::machine;` reference row. Follow
  [named implementations](wiki/spec/language/conformances.md#data-and-code-are-independent)
  and remove legacy support rather than preserving it for old tests.
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
- The sample gate is the work queue; the boards are not. `TASKS.md` records
  only what the sample loop cannot: a prerequisite outside any sample (an
  owner decision, an unavailable environment), release work with its own
  acceptance, and a regression that could not be repaired where it was found.
  `TASKS_BOOTSTRAP.md` and `TASKS_OPTIMIZER.md` keep the bootstrap chain and
  optimizer work, which have their own acceptance. An item exists only while
  it names unfinished work and is deleted when its acceptance passes; state
  its current frontier once, replacing the paragraph rather than appending
  history. Owner decisions go to `OWNER_QUESTIONS.md`. Nobody owns the boards;
  reconcile concurrent edits at rebase.
- Windows and macOS are both supported development hosts; shared tooling
  needs a documented usable entrypoint on each — a `.ps1` alone is not
  macOS support; either document `pwsh` as an explicit prerequisite or
  provide the portable route. Omega is the intended implementation language
  for tooling; existing Python/shell implementations are removed when Omega
  replaces them.
- Language fixtures name the compiler behavior they pin
  (`tests/omega/{pass,fail,run}/<behavior>/`), not the sample that exposed
  it; `samples/` are language pressure tests: runnable programs whose header
  documents an `Expected exit` or `Expected output contains` oracle. A
  check-only probe that binds no program entry pins one behavior and belongs
  in the fixture tree. Existing projects and separate invalid controls retain
  their [discovery roles](samples/README.md#project-roles-and-discovery). Samples
  may evolve through use into complete programs or reusable kernels; a stable
  contract does not freeze implementation. One owner extracts an approved kernel
  into `source/library` and migrates its consumers together, preserving oracles
  and negative coverage. Do not create another permanent hierarchy or stale
  inventory. A Rust test of one crate stays beside that crate.
- Write direct prose: state the idea in literal words.
- Read the matching skill before starting: `.agents/skills/` — `advance` for
  the sample loop, `rust-systems-programmer` for Rust implementation and review,
  `architecture-cleanup`, `task-board-cleanup`, `local-swarm`, `devin-swarm`,
  `codex-swarm`, `cursor-swarm`.

Deliberate configuration — do not "fix" it: `clippy.toml` thresholds are
raised and dev/test profiles use `debug = 0`. Tests use `opt-level = 1`;
ordinary dev builds are unoptimized except `proof-admission` and the `sha2`
dependency, which use `opt-level = 3`. Check `Cargo.toml` and session overrides before interpreting
compiler timings. Use `CARGO_PROFILE_DEV_OPT_LEVEL=1` for an optimized dev
compiler comparison without changing repository defaults. Debug information
can be restored with `CARGO_PROFILE_DEV_DEBUG=2` or `CARGO_PROFILE_TEST_DEBUG=2`;
`CARGO_PROFILE_TEST_OPT_LEVEL=0` disables test optimization for a session.
`.gitattributes` forces LF because canonical source and evidence identities
are byte-sensitive.

## Scope and escalation

- The session that starts work finishes it. A claim records what a session
  is doing so other sessions can see it; it grants nothing and blocks nothing.
  Parked work is available to any session once its prerequisite is met.
- A sample's work includes the complete application and the compiler repairs
  it needs to run; carry it through source, native/Terminal execution, invalid
  controls and publication. Several failing samples with one demonstrated root
  cause are one piece of work; shared files or pipeline stages are not a
  reason to group. Never divide a sample into stages for different agents. Only an actual external prerequisite (an owner decision, an
  unavailable environment) gates work.
- Keep a short record in the claim note: sample, acceptance, current failure,
  next necessary action and prerequisites. Reread it after context compaction
  and before changing direction. A checkpoint is not completion. A user stop
  or exhausted budget preserves the frontier without claiming success.
- A newly discovered obstacle belongs to the work that found it: fix it
  rather than filing a task for someone else. Do not split work or mine
  tasks to keep agents busy; narrow parallel slices build duplicate routes for
  the same operation. When the queue is empty, write a harder sample (step 4
  of the sample loop).
- Overlap is expected and needs no permission. A Git conflict is a design
  signal: two sessions are changing one decision point, so reconcile them into
  one implementation at rebase. Conflicts only show textual overlap; semantic
  breakage lands without one, which is why main must stay green on every host
  (below).
- Main stays green on every supported host. No machine runs all three OSes,
  so each host with a native golden runs `python tools/native_watch.py` on a
  schedule ([testing](tools/testing.md#corpus-outcome-gate)). Whoever sees a
  regression bisects it to its first bad commit and repairs it before landing
  more capability work. Record it in `TASKS.md` only when it cannot be
  repaired in that session, and say exactly what prevents the repair.
- After landing a sample, an agent may take a bounded
  iteration-performance or test-environment outcome based on an experienced
  obstruction. Record the workflow and adequate target before repair; return
  to product work when it meets that target. No incidental profiling detours.
- Report which samples now run, not activity or commit counts. Missing implementation
  and file overlap are work to resolve, not blockers. An actual missing owner
  decision, inaccessible environment or external dependency may pause that one
  piece of work; record it and continue everything else. Unrelated busywork is
  not a substitute.
- Language/architecture decisions go in `OWNER_QUESTIONS.md`, referenced from
  the affected sample's README; engineering decisions stay with the session doing
  the work. Only the owner answers, closes or rewrites an owner question. A
  session that believes the specification already answers one reports its
  citations to the user and changes nothing: not the entry, not the
  specification, not the boards. A guide example, a lower-level contract or the
  Rust compiler's behavior never ratifies a language rule. Gates pass on real behavior. The accepted bootstrap
  chain changes only through its proofs.

## Sessions and subagents

Before launching a new swarm or creating a new worker, successfully fetch remote
`main` and record its exact commit. Fast-forward a clean checkout on `main`;
preserve dirty or non-main checkouts and create new worktrees from the fetched
commit. Read `AGENTS.md` and the matching skills from that updated source before
selecting or dispatching work. A failed fetch stops the new launch; a cached
`origin/main` is never a fallback. Each independent cloud executor repeats this
preflight before implementation. Existing outcomes retain their worktrees and
unpublished work, integrating fetched main at a safe checkpoint outside an
active qualification capture. Offline plans and dry runs are previews, not a
completed launch preflight.

- Give a session or subagent a complete piece of work, never a stage: the
  sample that must run, its oracle, and that it lands its own result.
  State directions directly; never imply or insinuate them. The session
  carries authorized work through implementation, testing, review, normal
  main landing and remote verification, and keeps working until it is
  complete.
- Dot-created Omega cloud sessions and their native workers default to
  **GPT 6.1 Sol, medium reasoning** (`gpt-6.1-sol`, `medium`), unless the user
  specifies another model or effort. Select them through the runtime's exposed
  launch fields; a prompt alone cannot change a model. Before launching or
  resuming cloud work, follow the [cloud launch preflight](.agents/skills/dot-swarm/references/cloud-launch.md).
  A single advance remains one assignment; it does not authorize a swarm.
- A requested Cursor cloud wave follows
  [cursor-swarm](.agents/skills/cursor-swarm/SKILL.md). Each worker is a
  Cursor Task subagent with its own VM and branch, started from fetched
  remote main. Select its model through the exposed Task fields.
- Parallel sessions take distinct samples, chosen by root cause, not by
  file. Shared implementation paths are fine; each session works in its
  own worktree and reconciles at rebase. Reuse another session's repair when
  practical; never wait for permission to touch files.
- Read-only review or scouting does not authorize writes or publication. A
  user-required independent review remains required; no other approval or
  reviewer gate exists.
- Sessions record progress in their claim note
  (`python tools/claims.py note --ticket`) and their final report. Nobody
  reconciles another session's work on its behalf.
- Continue implementation when the design answers the question; when it does
  not, put the exact missing decision to the project owner and keep working
  on everything else.

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
| `compiler` | Product compilation coordination and reports in `omega-rust/omega/compilation/driver/`, the source-free native driver in `omega-rust/omega/compilation/native-production/`, or an inseparable compiler contract change spanning Psi and Omega. A coordinator call-site adjustment accompanying a stage fix keeps the stage's lane. |
| `build` | The product's Build activation in `omega-rust/omega/compilation/build-evaluation/`, plus product build declarations such as `source/omega/build.omg`. Provider planning and trust admission under `realization/` use `omega`; CLI policy-file handling uses `cli`. Repository build commands and CI use `repo`. |
| `packages` | Package acquisition, graphs, review, admission, installation, and update workflows in `omega-rust/omega/packages/`. Package command wiring accompanying those changes keeps `packages`. |
| `cli` | Command parsing, flags, help, and command dispatch in `omega-rust/omega/application/cli/src/`, plus the corresponding product entrypoint. The command's underlying compiler or package behavior uses its owning lane. |
| `tooling` | Shipped developer tools and shared host support in `omega-rust/omega/foundation/`, including artifact file writing and measurements. Semantic trust records and admission under `realization/trust-model/` use `omega`. |
| `library` | Bundled packages in `source/library/`, including `core`, `alloc`, `std`, and ordinary domain packages. A compiler semantic fix exercised by library code keeps its compiler lane. |
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
  distinctions. Optional optimization specialization requires a correct general
  path. See [compositional lowering](omega-rust/pipeline.md#compositional-lowering).
  A repair is accepted only when mechanical variants of its program work too
  (through a local, field, parameter or call result; in another state,
  machine or target body; an equivalent spelling). A failing variant means the
  repair matched a shape; generalize the operation instead of adding the
  variant.
- **Verify at trust boundaries, not after every stage.** Foreign package review
  and untrusted build-script execution verify; a stage never re-validates,
  replays or rebuilds output the same compile just produced. Verifiers stay
  exercised by the corpus runner and tests, off the default compile path. See
  [verification placement](omega-rust/pipeline.md#verification-placement).
- **Never push to `main` directly.** Reserve and publish through
  `tools/landing.py` ([tools/landing.md](tools/landing.md)); history is
  linear — rebase onto `main` and fast-forward, never merge
  (`git config pull.rebase true`). Once a publish completes, remove the
  worktree and delete the candidate branch — a landed lane is dead
  bookkeeping, not a record. Land only finished work: every commit builds on
  its own, because main is bisected one commit at a time, and no commit says
  its own validation or review is still pending. `landing.py` refuses both.
- Before editing shared paths, check `python tools/claims.py status` and
  record a claim for your work ([tools/claims](tools/claims.md)) so other
  sessions can see it. Claims never grant or block work. If coordination is
  unavailable or times out, report it and continue in an isolated worktree
  with the intended scope recorded locally. Do not wait repeatedly, treat an
  outage as a conflict, or claim registration succeeded. Path overlaps are
  informational; reconcile overlapping changes at rebase. Existing tickets
  remain valid; refresh old clients that reject path overlap. Publication
  still uses the landing queue and never overwrites incoming work.
- Keep sample coverage out of the shipped CLI.

## Design references

- [Omega Language Guide](wiki/language_guide/language_guide.md)
- [Compiler ownership and pipeline](omega-rust/pipeline.md)
- [Documentation index](wiki/README.md)
- [Terminal Psi product contract](wiki/spec/terminal-psi/product.md)
- [Optimization phases](wiki/spec/build/optimizations.md#phase-and-product-boundaries)
- [Rust compiler completion](wiki/drafts/reference/rust_compiler_completion.md)
