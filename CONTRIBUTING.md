# Contributing

The full contributor contract for this repository: coding conventions,
platform support, test and sample policy, workflow, commit naming, and prose.
[AGENTS.md](AGENTS.md) is the canonical instruction file for coding agents and
keeps only the always-needed summary; this file holds the detail it links to.
A change that moves a rule between the two must update every inbound anchor.

## Repository conventions

These coding conventions are authoritative for the repository.
For Rust implementation, refactoring, and performance review, also read the
[rust-systems-programmer skill](.agents/skills/rust-systems-programmer/SKILL.md).
Use that exact local copy; these repository contracts take precedence.

- Use real words in code. Prefer `character`, `statement`, `expression`, and `arguments` over `ch`, `stmt`, `expr`, and `args`.
- Avoid names that only make sense to compiler insiders. `pipeline` is better than `driver`; `expression` is better than `expr`.
- Keep compiler stages honest. Parse syntax, lower representation, validate semantics, plan native execution, then emit bytes.
- Organize source by responsibility, not line-count or directory-depth quotas.
  `main.rs` owns application startup and visible dispatch to the real subsystems;
  it must not be a forwarding stub created to satisfy a size rule. Make the
  entrypoint prominent and supporting responsibilities easy to find beneath it:
  names and calls should lead the reader toward the operation they seek.
  Cohesive siblings are fine; neither a flat pile of unrelated handlers nor a
  staircase of forwarding modules makes ownership discoverable. Split at a
  genuine responsibility boundary, not to manufacture a small `main.rs`,
  `lib.rs`, or `mod.rs`. When adding or reorganizing code, walk the touched path
  from entrypoint through dispatch to work and result handling before handoff.
  Architecture checks enforce dependency and semantic boundaries, not file sizes
  or prescribed cosmetic splits.
- Keep sample coverage out of the shipped CLI. Tests and dev harnesses may discover `samples/`, but user-facing compiler behavior stays generic.
- Prefer small checkpoint commits after working improvements.
- Samples should reveal language pressure, not hide it in giant `main` files.
- Prefer arena-backed compiler data. Contiguous storage and small handles beat a pile of tiny heap allocations.
- Lowered representations should prefer `Handle<T>` and `HandleSpan<T>` over owned `Vec<T>` fields for repeated child lists.
- `Vec<T>` is fine for parser output, temporary builders, and local scratch data. It should not become the default long-lived representation shape.
- Prefer arena/vector-backed symbol tables over local hash maps. Dense lookups should collapse toward ids/handles as phases mature; hash maps need a specific sparsity or boundary reason.
- Before adding an index, ask whether ownership, dense handles, ordered spans, or the producing stage can carry the relationship directly. A map keyed by compiler-owned objects is a review trigger for missing representation structure, not automatically a defect. Eliminating repeated lookup is preferable to accelerating unnecessary lookup.
- Compare total construction, lookup, mutation, cloning, and retained-storage costs on relevant workloads. Beating a whole-arena scan alone does not establish the best framing. Hash maps remain valid for sparse, irregular associations; do not replace them with oversized sparse arrays, repeated sorting, or more bookkeeping merely to avoid hashing.
- Prefer parent-owned `HandleSpan` child ranges for symbol lookup. Linear sibling scans over `HierarchyArena` child ranges are the default because real scopes are usually small and cache-friendly; global hash maps are an optimization for measured pathological scopes, not the baseline design.
- Choose the work domain before tuning its loop: retain immutable inputs and useful selections, avoid rebuilding unchanged owners, and preserve independently produced results when consumers can use them. See the Rust skill's [storage and execution guidance](.agents/skills/rust-systems-programmer/references/storage-and-execution.md); this is not a mandate for RLE, nested vectors, caches, or threads.
- Use paged arenas where growth must preserve existing pages. Parallel readiness additionally needs independently owned mutation/output, stable handle identity and deterministic publication; paging alone does not remove shared append contention or final remapping costs.
- Paged arenas use generational handles so reclaimed page storage cannot resurrect stale references.
- Do not use `RefCell` as an ownership escape hatch. Runtime borrow checking is not a substitute for clear compiler-phase ownership.
- Prefer ZII (Zero-is-initialization). Null handles (index 0) resolve to dummy arena entries instead of optionals and literal nulls.
- Do not wrap handles in `Optional` just to model absence. The zero handle is the absence state; `Optional<Handle<T>>` needs a semantic reason beyond “maybe missing.”
- Arena handles must be generational. Freed or stale handles resolve to dummy entries, not reused storage.
- Symbols are handle-first. String names are debug/export/import metadata, not durable identity inside semantic or native compiler layers.
- Source text is source-loading, diagnostic, and debug payload. Beyond resolution, source-backed names are technical debt unless they are literal program strings, diagnostics/debug metadata, or final-image import/export payload.
- Use stable handles when data needs references across phases; use redirect tables only when arena contents need reordering.
- Comments should explain non-obvious intent. Do not add “doing X unlike Rust” commentary unless the contrast changes implementation.
- Crate `lib.rs` headers carry the reasoning signatures cannot: mechanism, justification, honest boundaries, and provenance. `omega-rust/omega/backend/images/image/src/lib.rs` and `omega-rust/psi/semantics/terminal-fixed-fuel/src/lib.rs` are the established exemplars.

Configuration that looks wrong but is deliberate:

- `clippy.toml` thresholds are raised on purpose (ownership and custody APIs
  return the original authority on failure; syntax and proof enums retain full
  structure). Do not "fix" a large error type or enum variant to satisfy a
  default lint.
- `debug = 0` in `[profile.dev]` and `[profile.test]` is intentional; opt back
  in per-session with `CARGO_PROFILE_TEST_DEBUG=2` or
  `CARGO_PROFILE_DEV_DEBUG=2`.
- `opt-level = 1` in `[profile.test]` is intentional: a test that compiles
  an Omega program checks the whole assembled program, and unoptimized that
  is 4.6 times slower (40.0 s against 8.6 s per program-compiling test) while the
  edit-loop rebuild stays around 13 s. Override per session with
  `CARGO_PROFILE_TEST_OPT_LEVEL=0` when stepping through test code.
- `.gitattributes` forces LF because canonical source and evidence identities
  are byte-sensitive. Windows launchers (`.bat`, `.cmd`) keep CRLF.


## Developer platform support

Windows and macOS are supported development hosts; Zac works on macOS. Shared
build, test, maintenance, and landing workflows must have a documented usable
entrypoint on both. Do not assume PowerShell, Windows paths, drive letters, or
Windows-only executables are available on another developer's machine.

Omega is the intended implementation language for repository tooling and test
orchestration. Existing Python, shell, and PowerShell implementations are
temporary support while the Omega compiler and required libraries become usable.
Keep their maintenance bounded; prefer deleting redundant scripts and reusing
existing operations over expanding a host-language framework. Migrate a workflow
when Omega can run it with the same observations and failure handling, and remove
the replaced implementation. The eventual host-specific surface is a few
optional compiler kickstart scripts and tests of actual platform behavior.
This direction does not require an unavailable Omega tool to bootstrap itself.

- Until that migration is practical, put shared behavior in one portable
  implementation using an existing Rust subcommand or Python entrypoint. Thin
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
An open item's evidence states the current frontier once: recording a newer
observation replaces the paragraph it supersedes. Do not append revision stamps,
run-by-run comparisons, passed-test totals, or a history of investigated causes.
Keep a reproduction, revision, or measured limit only when it changes the next
implementation decision. Detailed logs belong in run artifacts; landed results
belong in Git. Cite
revisions as published on `main` — landing rewrites worktree SHAs, so a
prerebase reference is unverifiable — and prefer symbol and test names over
line numbers, which drift.
When creating or refining an item for delegation, include enough rationale,
design links, dependencies, and scope to route it without rediscovering the
problem. Add only context that affects the assignment; no mandatory field
template or separate delegation board is needed. Tag an added item's provenance
on its first line — `(new-scope)` for newly discovered work or
`(split-of:<parent-item>)` when it decomposes an existing item — so board
growth stays measurable; items added before 2026-09-14 are untagged.

An implementation assignment is owned through its agreed customer acceptance,
including the necessary in-scope producer, consumer, fixture, and validation
changes. Finding the next failing stage is an intermediate result, not task
completion or a reason to create a task for another agent. Continue the repair
when the contract answers it. Respect active assignments and explicit scope
limits: coordinate a concrete dependency handoff without claiming the customer
is fixed or silently abandoning its integration ownership.

Workers report diagnoses and unlanded evidence through their claim notes and
session report, not standalone `board:` commits or PRs. Board-only publication
is reserved for an explicitly assigned audit, owner decision, or coordinator
consolidation; renaming its commit does not make an implementation handoff into
a delivered fix. A newly discovered item must preserve a real required outcome
outside the current assignment, not offload the remaining work of that assignment.
Before adding one, use the existing capability owner and consolidate duplicate
symptoms. Keep distinct acceptance cases, design rationale, and real blockers.

Owner decisions belong in `OWNER_QUESTIONS.md`, not on a board. Before starting
work, fetch `main` and inspect recent commits in that lane to avoid overlapping
an active change. Prefer small checkpoint commits after working improvements.

The [bootstrap contract](bootstrap/CONTRACT.md) and its linked language/proof
owners hold current requirements; Git holds the retired decision history.
Preserve ratified semantics. Correct obvious stale text, but raise genuinely
unsettled conflicts in `OWNER_QUESTIONS.md` and mark the affected specification
undetermined. Apply [owner escalation](bootstrap/MINIMIZATION.md#owner-escalation)
and leave an owner-blocked edge open until answered.

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
Confirmed unrelated failures follow Validation scope: retain the tested
revisions, commands, and attribution evidence in the checkpoint. Do not relabel
new or unexplained affected failures as baseline.
Never extend a head lease or bypass the exact-reference publication checks.
Expiry is automatic; early cancellation/recovery still requires the exact
ticket/claim and the owner's direction.
See [landing](tools/landing.md) for commands,
observed-owner recovery, and handling an uncertain network result. This protocol
coordinates publishing across machines; it does not assign work ownership.


### Scope checkpoints

Before each advancement milestone, name the exact customer, the behavior or
required evidence it will deliver, and the downstream dependencies still missing.
Read the owning minimization/design contract; a board item or passing helper
test is not by itself a reason to retain machinery. Test the premise itself:
does this obligation serve the long-term goal, is the stated dependency real,
and is the proposed implementation necessary? Do not merely optimize the way
an unjustified task is executed. For bootstrap work, apply
[whole-chain minimization](bootstrap/MINIMIZATION.md), counting
source, profiles, proofs, tests, and host plumbing together. Human-auditable
proof closure is a first-class customer alongside execution; the checker design
must earn its complexity too. Neither a runnable chain without required evidence
nor an expanding proof framework without a concrete edge satisfies that goal.

Before adding a subsystem, a new helper layer, or a workaround for a resource
limit, compare the simpler alternatives in commentary: deletion/deferment,
consolidation, or changing the underlying implementation/provision where the
owner permits it. Distinguish language semantics and ratified observation
contracts from private capacities. Do not invent measurements, weaken checking,
or change the trusted boundary to make an alternative appear cheaper.

At checkpoint handoff, state the customer's observed result before and after,
what actually reuses the implementation, what complexity was added or removed,
and what remains missing. Do not claim anticipated reuse as demonstrated reuse.
Assess whether the remaining plan still makes sense. After
two consecutive milestones that only add supporting machinery without advancing
customer behavior, reducing human audit burden, or establishing measured
feasibility/completion of a named proof obligation, pause implementation
before a third on that strategy: summarize the accumulated cost and assess
continuing, simplifying, or deferring. Successful helper tests do not reset this
checkpoint. Apply the same pause immediately when the required
customer or downstream contract is absent and work would become speculative.
This applies across commits, agent handoffs, and automatic goal continuations.

Pauses apply to a strategy; missing implementation does not make its customer
unavailable. Before resuming a paused strategy, assess the failed approach and
state the revised bounded path to customer acceptance. Another isolated helper
under a new name does not satisfy that assessment. Skip work that genuinely needs
an unanswered owner decision, unavailable prerequisite, or conflicting active
assignment, and continue elsewhere within the user's scope. Do not present a
known pause as a delivered advance or ask the user to choose ordinary engineering
steps. Retain customer continuity while a defensible path remains.

For unrestricted compiler advancement, prioritize the boards' immediate customer
outcomes over unrelated cleanup or helper work. A red customer command and a
repair spanning multiple stages are reasons to investigate, not reasons to pass
over the task. Name the concrete blocker or active assignment when bypassing an
otherwise eligible priority. Explicit user assignments still govern selection.

Actual owner language/architecture decisions belong in `OWNER_QUESTIONS.md`, with
the dependency referenced on the owning task; continue elsewhere while awaiting
the answer. Ask for scope/prioritization direction only when no actionable work
remains within the user's allowed scope, reporting the candidates and blockers,
or when the user's explicit restriction prevents proceeding. An unavailable
operational prerequisite may also require a stop with evidence. A pause does not
authorize deletion, abandoning required proofs, or rewriting settled decisions.

Bootstrap design exploration, including non-authoritative experimental
implementations, is delegated under [whole-chain minimization](bootstrap/MINIMIZATION.md).
Name the next compiler/checker customer, compare simpler alternatives, and count
the complete audit cost and displaced machinery. Do not seek approval merely
to run that comparison or preserve a general-purpose rung for hypothetical use.
Experiments do not change the accepted chain or discharge its proofs. Escalate
changes to the trust boundary or required assurances before relying on them;
experimental success alone is not authority to weaken either.


### Agent delegation

Use Astra for coordination and delegated work requiring judgment: repository
refactoring, architecture or ownership decisions, performance analysis, semantic
review, and skill evaluation or improvement. A bounded task or detailed prompt
does not make that work mechanical.

Use Luna only for straightforward execution after the decisions are settled:
applying an exact edit recipe, repetitive mechanical changes, or running already
specified checks. The assignment must leave no architectural, behavioral, or
tradeoff decisions to the worker and must include concrete acceptance checks.
If such decisions emerge, stop that worker's implementation and route the work
to Astra. A single bounded change can stay with one agent; parallelize independent
work with one edit owner per change.

Assignments need the objective and rationale, anchor files and governing design
links, ownership boundary, dependencies, acceptance command or artifact, and
concrete escalation conditions. Read the linked architectural reasoning before
crossing ownership boundaries; a file list is not sufficient. Investigate code
and specifications first, then escalate failed checks, uncovered invariants,
ambiguity, or scope expansion to the coordinator or a stronger model. Confidence
alone is not evidence. Preserve Zac's design authority and the existing
`OWNER_QUESTIONS.md` criteria; uncertainty does not automatically create an
owner question. Never invent semantics or validation evidence to satisfy a gate.

The coordinator owns dispatch, monitoring, result inspection, and integration.
Assign one integration owner per customer slice, including its outer acceptance
command; split independent dependencies only with explicit edit boundaries and a
handoff back to that owner. Before editing, check the claims registry
(`python tools/claims.py status`) for live session assignments and recent lane
commits, claim the chosen item and its owning paths through
[tools/claims](tools/claims.md), and announce the customer slice and owning
paths to the coordinator (or in the working conversation for an uncoordinated
local session). A live conflicting claim is a real assignment: resolve it
before editing the shared paths; continue read-only investigation or
independent work meanwhile. Do not require
a new global reservation or proof that no unseen session exists. Old wave exclusions and
idle worktrees are not proof of active ownership, and unclaimed work can still
collide — the claims registry is an advisory fence, not a lock. The landing
queue serializes
publication, not development, and does not prevent duplicate implementation.
Use actual agent tools and returned IDs before reporting workers as launched,
queued, or running; report tool failures as failures. Distinguish implementation
completed, verification-only completed, diagnosed blocker, and work continuing.
A diagnosis does not deliver an assigned fix: continue implementation when the
existing design answers it, or identify the exact missing dependency or decision.
Consolidate duplicate blockers in the owning board item and hand off concrete
findings. Follow the dispatch and review steps in
[advance](.agents/skills/advance/SKILL.md#delegate-a-bounded-assignment), alongside
the existing isolation, validation, and landing rules.

Cloud swarm waves are coordinated with [tools/swarm](tools/swarm/README.md):
the coordinator pre-assigns one board item per session in a wave manifest, and
each session is an ordinary `advance` invocation restricted to that item. Live
assignments are registered on `refs/coordination/omega-claims/main` through
[tools/claims](tools/claims.md): the launcher checks each manifest entry
against it, and each session claims its item before editing. Sessions publish
through the landing protocol with a `swarm-<wave>-<name>` owner
label, and receipts stay in ignored `build/swarm/`, never on boards. The wave's
summarized per-session outcomes land in
`tools/swarm/waves/<wave>.outcomes.json` (`report --save`).


## Commit naming

Use `lane: statement`. Choose the lane from the changed responsibility below,
not the repository name, implementation language, or a prefix copied from Git
history. Omega names the whole project, a compiler stage, and a bootstrap
implementation; those meanings must stay distinct in commit subjects.

| Lane | Responsibility and current path anchors |
| --- | --- |
| `alpha`, `beta`, `gamma`, `delta`, `epsilon` | The corresponding language rung in `bootstrap/<rung>/`, including its compiler/evaluator, Rust reference tooling, and rung-specific tests. Use the actual rung name. |
| `bootstrap-omega` | The Epsilon-written first Omega compiler D in `bootstrap/5_omega/` and tests specifically of that implementation. Never shorten this to `omega`. |
| `bootstrap` | Cross-rung reconstruction, trust-chain edges, artifact provenance, and chain hygiene; includes `tests/bootstrap/` and shared `tools/bootstrap/` orchestration. A helper specific to one rung uses that rung's lane. |
| `psi` | Target-neutral source semantics through Terminal Psi: parsing, resolution, typing, checking, proof, interpretation, and Psi optimization in `omega-rust/psi/` or `source/psi/`. |
| `omega` | The Terminal-Psi-consuming compiler stage: Omega representations, transforms, optimization, and realization semantics in `omega-rust/omega/{representations,pipeline,semantics}/` or corresponding `source/omega/` implementation. Never a project-wide default. |
| `backend` | Target, ISA, ABI, object/image encoding, layout, and execution primitives in `omega-rust/omega/backend/` and their product-source equivalents. Transform policy in `pipeline/` remains `omega`. |
| `compiler` | Product compilation coordination and reports in `omega-rust/omega/docs/compiler/`, or an inseparable compiler contract change spanning Psi and Omega. A coordinator call-site adjustment accompanying a stage fix keeps the stage's lane. |
| `build` | The product's build evaluation, composition, provider planning, deployment, and trust ledger in `omega-rust/omega/build/`, plus product build declarations such as `source/omega/build.omg`. Repository build commands and CI use `repo`. |
| `packages` | Package acquisition, graphs, review, admission, installation, and update workflows in `omega-rust/omega/packages/`. Package command wiring accompanying those changes keeps `packages`. |
| `cli` | Command parsing, flags, help, and command dispatch in `omega-rust/omega/src/`, plus the corresponding product entrypoint. The command's underlying compiler or package behavior uses its owning lane. |
| `tooling` | Shipped compiler support such as artifact views, profiles, language-server/docs-generator behavior, and host custody in `omega-rust/omega/tooling/`. |
| `library` | Bundled packages in `source/library/`, including `core`, `alloc`, and `std`. A compiler semantic fix exercised by library code keeps its compiler lane. |
| `samples` | Standalone sample/example content and presentation in `samples/` or example directories. A regression fixture for an implementation fix keeps that implementation's lane. |
| `tests` | Shared test infrastructure, corpus registration, fixtures, or architecture gates spanning multiple owners. Tests for one responsibility use that responsibility's lane, even in a test-only commit. |
| `tools` | Repository maintenance/development utilities in `tools/`, including landing reservations and sample-refresh scripts. Bootstrap tools follow the rung/chain rules above; shipped tooling uses `tooling`. |
| `repo` | Repository-wide workflow, agent instructions/skills, CI, workspace/dependency configuration, formatting policy, and Git settings. Includes standalone policy changes in `AGENTS.md`, `CONTRIBUTING.md`, `CLAUDE.md`, and `.claude/`. |
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

Commit messages carry no tool attribution: no "Generated with ..." line, no
`Co-Authored-By` trailer naming an agent or vendor, and no vendor links. The
recorded author is the person or service identity the environment is
configured to use; agent tooling does not append itself to history.


## Prose

Mannered prose substitutes metaphor and flourish for direct statement. Instead
of "a parameter worth varying," the mannered writer produces "a dial worth
turning." Instead of "this point still matters," they write "this point earns
its keep." The phrases exist to display the writer, not to convey the idea, and
readers can tell. That is why mannered prose irritates: it makes the reader
work harder so the writer can perform. It is also imprecise. Metaphors drag in
connotations the writer did not choose and cannot control. The fix is to say
what you mean. When a literal phrase is available, use it.
