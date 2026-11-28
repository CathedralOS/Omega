# AGENTS.md

This is the canonical instruction file for every coding agent working in this
repository. Agent-specific instruction files must point here rather than copy
these rules. Repository skills are canonical in `.agents/skills/`; read and
edit them there. Pi and Codex discover that directory directly. For Claude
Code, create an ignored checkout-local link when `.claude/skills` is absent:
- Windows PowerShell: `New-Item -ItemType Directory -Force .claude; New-Item -ItemType Junction -Path .claude/skills -Target (Resolve-Path .agents/skills)`
- macOS/Linux shell: `mkdir -p .claude && ln -s ../.agents/skills .claude/skills`

**Skill invocation.** Use a repository skill only when the user explicitly asks for that skill in chat. This also applies to workers and to links between skills. Otherwise carry out the task using the project instructions and ordinary tools.

Omega is a proof-checked systems language whose programs are data-oriented
state machines:
- [source-rust/](source-rust/README.md) contains the Rust compiler, CLI, interpreter and package manager in the root Cargo workspace.
- `source/application/` and `source/subsystems/compiler/` — the Omega-written product compiler.
- `source/library/` — bundled `omega::` packages (`name.omg` or `name/mod.omg`).
- [`bootstrap/`](bootstrap/README.md) groups implementations by language. All routes share Alpha and Beta. For Gamma, Delta and Epsilon, compare interpretation, compilation to Alpha tape and compilation to native code. Host launchers, checks and optional proof research live in `tests/bootstrap/`. Record route experiments in [the development wiki](wiki/development/bootstrap/README.md).

**Public naming:** Follow [public names](wiki/README.md#public-names): the portable
IR, artifact, interpreter and CLI subsystem are **Psi**. **Terminal Psi** names
only the internal compiler stage/representation. Keep internal paths and wire
identifiers intact; do not export that stage label into help, reports or guides.
`omega run app.omg` always lowers the selected Product to Psi and interprets
its entry. `--target` selects the environment for that interpretation. To
verify a native executable, build it and launch the published executable
on its target host. [CLI contract](wiki/spec/build/embedding.md#standalone-interpretation)

For execution results, write "native/Psi agreement" or "Psi exit 0".

**Words this file uses.** You will copy the vocabulary you read into what you
write, so define a term the first time you use it in a report, and prefer the
plain wording over the shorthand:
- **Psi**: the portable, target-neutral form of a compiled program, and the
  interpreter that runs it. **Terminal Psi** is the last such form before
  machine code (internal name only; see public naming above).
- **Lowered-form checker**: the compiler stage that checks a program's proofs,
  ownership and contracts on its Terminal Psi. Its yes/no is the compiler's
  only verdict.
- **Fact**: something the compiler has proved about a value, such as
  `x >= 0`, that later code can rely on.
- **Positive / negative twin**: two test programs that differ in one input.
  The positive must compile and run; the negative must be refused with a
  specific error.
- **Hosted target**: one of the five operating-system/CPU pairs Omega must
  produce native programs for (listed below).
- **Release gate**: one of the six areas that must all be finished before the
  compiler ships (listed below).
- **Lane**: the area prefix of a commit title, such as `psi:` or `docs:`
  (see [commit naming](#commit-naming)).

**The goal is a complete compiler that ships.** Follow the work and review rules below and [Rust compiler completion](wiki/drafts/reference/rust_compiler_completion.md)
for the six release gates (`repository`, `source semantics`, `build and packages`,
`native matrix`, `diagnostics`, and `representative programs`) across all five
hosted targets (`linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`,
`windows_arm64`). The owner added `windows_arm64` on 2026-11-23. Programs can
bind `windows_arm64::ProgramEntry`; `TASKS.md` **WINDOWS-ARM64-TARGET** tracks
its system providers and the first run on a Windows ARM64 host.

Omega has one specification in [wiki/spec/](wiki/spec/). Describe
compiler-specific implementation status in the compiler's plan or layer
contracts.

**Zac and Jarod decide the specification.** When an owner directs closure of an
answered question, apply the ruling to the governing docs or specification,
update references and remove the entry from [OWNER_QUESTIONS.md](OWNER_QUESTIONS.md)
in the same PR. Otherwise agents read `wiki/spec/` and build what it says.
Escalate an unresolved change with the existing sentence, proposed text and reason.

The Psi wire encoding (numeric tags, field layouts and format markers) is the
compiler's file format, not specification. It lives beside the codec in
[encoding.md](source-rust/subsystems/compiler/psi/artifact/wire/encoding.md).
Update it in the same PR as the codec change it describes; its tag tables are
tested against the decoder by `mbx test -p terminal-wire --lib spec_tag_tables`.

## Bootstrap mandate

For bootstrap construction work, [bootstrap/MINIMIZATION.md](bootstrap/MINIMIZATION.md)
is the owner's governing mandate. It authorizes competing peer variants, changes
to topology and intermediate languages, compiled/interpreted/native experiments,
and explicit accelerators. Its architecture authority supersedes the fixed-design
and no-experiment instructions below for this scope. Formal bootstrap correctness
certificates do not gate construction; ordinary Omega safety and proof semantics
remain required. Use bootstrap compiler and Omega compiler as public names.
Record unresolved changes outside the mandate in
[OWNER_QUESTIONS_BOOTSTRAP.md](OWNER_QUESTIONS_BOOTSTRAP.md), not the product queue.

## Explain the change

Use an inverted pyramid in PR descriptions, commit bodies, progress/final
reports, explanatory documentation and comments. Write for a reader who knows
the project but not this subsystem: the first one or two sentences must explain
in ordinary language the specific problem, what changes and why it matters to
the person using or maintaining the software. Apply that clarity to titles too,
within the commit-lane rules.

Then supply the context or concrete example needed to understand the cause and
repair. Explain what an example demonstrates before its test values or syntax;
introduce technical terms as they become useful. A plain opening followed by
internal jargon still fails. Each paragraph should build on the last, so a
reader can stop at any level with a useful understanding. Comments start with
the local reason and constraint, using the surrounding code's context. Keep
small changes short rather than filling a template.

Three faults make agent writing hard to read. Check every sentence for them:
- **Hidden actor or action.** Make the thing that acts the subject and what it
  does the verb. Not "a refusal is produced upon checker invocation" but "the
  checker refuses the program".
- **Vague or invented wording.** Use the words a person on this project would
  say out loud. Avoid noun stacks ("loan restoration window obligation") and
  coined labels ("the entry boundary"); name the function, file or behavior.
- **Empty statements and buried points.** Put the point first and cut
  sentences that say nothing. Not "nothing else depends on anything but the
  result, so I'll report when it returns" but "I'll report when the test
  finishes."

When rewriting existing text, preserve its meaning and identifiers. The
[rewrite checker](.agents/skills/plain-writing/scripts/check_rewrite.py) checks identifiers and length.

Check claims against the actual submitted or landed diff and observed behavior;
inspect unclear code or examples instead of inventing a plausible benefit.
Rewrite the explanation when scope changes. Keep material limits beside the
result they qualify. Put validation and detailed provenance afterward, retaining
exact commands, source/host identities, failures and unverified cases. Historical
results remain tied to their recorded sources, not promoted to fresh validation.

End every PR description and commit body with one portable model-audit line
generated by `cargo xtask provenance`:
`Agent: <name> | Model: <model> | Effort: <effort> | Tokens: <in> in (<cached> cached), <out> out | Steps: <N> | Time: <duration> | Steers: <N>`,
where `<name>` is `Claude Code`, `Codex`, `ChatGPT`, `Cursor`, `Devin`, `Antigravity`,
`Dot`, or `Human`.
Several agents share each GitHub
account and GitHub squash merges preserve the PR body on `main`, so this line
records the exact model, reasoning effort, token cost, step count, active
duration, and human steers for every landed commit. Run `mbx xtask check-rules --body-file <path>` locally to validate the PR metadata. The required [admission workflow](.github/workflows/admission.yml) checks that the shared spec exists and that active code stays out of the archive.
The owner has authorized this line permanently for every agent: add it
automatically, never ask about it or report it as a pending step, and replace
any footer your harness adds (`Co-Authored-By`, "Generated with/by ...") with it.

A PR in a compiler lane (`psi`, `omega`, `backend`, `compiler`, `build`,
`packages`, `library`) also carries one line naming the release gate it serves
and what the change makes work:
`Gate: <release-gate> | <what the change makes work>`, where the name comes from
the release matrix in [Rust compiler completion](wiki/drafts/reference/rust_compiler_completion.md#release-matrix).

The same PRs carry one line naming the specification they implement, so a
reviewer can open the text the change claims to follow:
`Spec: <wiki/spec/...md[#anchor]>[, <page>...]`, or
`Spec: none | <why no page governs this>` when the spec is silent. A change that
knowingly departs from a page says so in its description, beside this line.

The same PRs carry one line saying what proved the change:
`Proof: <tests on the change and the area around it> | <clean, or what failed and what it turned out to be>`.

## Law of completion

When you take a program that does not compile, you own every fix it needs until
it compiles, runs, and is merged to `main`, not just the first fix. If it is
hard, investigate further; do not give up or change what "done" means.

**Outcome ownership experiment (owner-authorized, 2026-11-15).** We are testing
whether checkpoint merges let attention leave unfinished programs. Keep the
original program and its specified result as the assignment through those
merges. Use the pre-merge result to identify any remaining failure, including
a pre-existing failure in another stage, and keep working on that failure.
Small PRs remain useful checkpoints; they do not complete the assignment or
release its claim. If the session must stop, record the source revision,
original acceptance, current failure and next action in the existing task or PR,
and name the receiving session when one accepts the handoff. Without a receiver,
report the outcome as unfinished. Evaluate this experiment from session records:
time to the original result, repeated investigation and ownership gaps; a single
run can reveal loop failures but cannot establish a throughput improvement.

That includes fixes in other compiler stages, the bundled library, or anything
else the program depends on, all made in your own worktree. Do not split the
work into one task per stage, and do not wait for another session to supply a
missing piece. Writing up a bug does not fix it. The work is done when the
program runs.

The specification is the answer key. Read the page in `wiki/spec/` that covers
the feature and build what it says. Test what you changed, then merge it.

The compiler used to have a second checker that worked on the earlier,
tree-shaped form of the program. That checker has been removed. The
lowered-form checker is now the only one. If a program needs something that
checker cannot yet judge, add it to that checker (`TASKS.md`,
**LOWERED-FORM-CHECKER**). Until then the program is refused. Never weaken the
checker to make a program compile.

An open PR is unfinished work sitting in the way of everyone else's. If you
are the one looking at it, it is yours: finish it or close it.

### Decision-making framing for compiler completion

1. **Fix shared causes, not single programs:** Pick work by how many programs
   it makes work. When several programs fail for the same reason, fix the
   mechanism the spec describes so all of them work. Test the change and the
   small area around it; do not go looking for more before landing.
2. **When unsure, refuse:** If the compiler cannot prove something, it must
   reject the program, never accept it. Never make a program pass by weakening
   a proof requirement, by returning `true` when a lookup finds nothing
   (`None`), or by silently dropping something the compiler did not
   understand.
   Use checked providers and adapters for provider contracts. The
   [native borrow operations](wiki/spec/resources/native_borrows.md) require
   explicit Build admission for their specific reach. Treat any use of
   `Unsafe` in a library as malicious. Repair ordinary borrow, lifetime and
   contract errors in the program or checker.
3. **Track each field separately:** Record facts, borrows ("loans") and
   values that must be put back at the exact field path they apply to, such as
   `root.field` or one variant's payload. A write to `a.x` invalidates what is
   known about `a.x` and about `a` as a whole, but not about a separate field
   `a.y`.
4. **Test the behavior the change promises.** Use the owning layer's fixture
   for a layer contract. Exercise the consumer when the change affects a
   boundary. For a program whose expected behavior includes Terminal Psi and
   native execution, check the expected result independently on each engine,
   then compare termination, stdout, stderr and any outputs the test declares.
   Honor the required exit of the original program. Use
   `omega build --compare-tests build.omg` for registered tests and
   `mbx xtask sample-gate --filter <sample-path>` for repository samples.
   [Comparison contract](wiki/spec/build/testing.md#comparing-native-and-psi-tests)
5. **Board closure:** Delete completed bullets from `TASKS.md` outright (never
   rewrite into `"preserving / keeping / now passes"` history prose).
6. **Generalize instead of adding special cases:** If your fix needs a second
   copy of a data structure, an extra case, or a branch for one kind of
   location, the existing mechanism is too narrow. Widen that mechanism so one
   change fixes every program with the same cause, and do it in the same PR.
7. **Treat a repeating pattern as a broken contract:** Check every change
   against the spec and the layer contracts by default: `wiki/spec/`, each
   stage's README, [stage trust](wiki/drafts/designs/stage_trust.md) and
   [check on the lowered form](wiki/drafts/designs/check_on_lowered_form.md).
   When the same check, refusal or boilerplate appears dozens of times, first
   ask which contract it breaks. Usually a stage re-derives or re-checks a
   fact that another stage owns. Move the fact to its owner and delete the
   copies; don't compress them into a helper. Update both stages' READMEs in
   the same PR. For example, hundreds of "lost/changed its" refusals in stage
   05 `read/` mean typing does not publish what lowering needs.

## The sample loop

A language feature should work wherever a value can appear (a local, a field,
an argument, a return value, and so on) and on every hosted target, with a
negative twin. The lowered-form checker gives every one of those places the
same treatment: a fact belongs to a value, and a place only says which value it
holds ([check on the lowered form](wiki/drafts/designs/check_on_lowered_form.md)).
So there is no list of places to test one by one. A fix that works for one
place but needs new code for another is the too-narrow mechanism rule 6 above
describes. A `TASKS.md` item names an
acceptance only when it ends in a deletion (the old surface or path is gone);
otherwise its spec page and the running program decide when it is done.

A pull request proves the thing it built. Test the changed code, then a very
small area around it. If that is fine, land. If it is not, investigate that
failure; do not widen the run. There is no fixture corpus to run and no ledger:
the corpus under `tests/omega/` was deleted on the owner's ruling of 2026-11-13
([tests/README.md](_recycling/tests/README.md) says how to recover a fixture from
history), and nothing replaces it.

A program a test reads lives under its crate’s `tests/fixtures/<feature>/<tier>/<case>/`. Multiple owners share an input under the repository’s `tests/fixtures/`. The
test pins the exact result the program must produce (its output or its
error), which records what the program asks of the compiler. Do not make a test pass by changing the
program: a different fact, a looser bound or a different operation turns it
into a different test, and passing that one proves nothing about the original.

Tests follow the spec, not the current code (owner ruling, 2026-11-27). A
structural change is expected to break tests that pin internals: intermediate
tables, stage-to-stage forms, helper APIs, exact wording the spec doesn't fix,
or a mechanism the change deletes. Rewrite, merge or delete those tests in the
same PR. Never keep a design alive because a test reaches into it. History
keeps every old test if one is needed again. What a refactor must keep is the
spec-required behavior those tests stood for. That means a sample's output and
exit code, a refusal the spec names, native/Psi agreement and release-gate
contracts. Re-prove each on the new code, through the public surface where the
old test reached into internals. Name the tests rewritten or deleted, and why,
in the PR's `Proof:` line. Choose the change with better quality and yield
(less code, faster and leaner compiles, the same spec behavior), not the one
that breaks the fewest tests.

### No `.omg` file bloat

`samples/cli/facts/` is a **frozen finite set**, and `tests/omega/` is gone.
`cargo xtask check-rules` rejects any change that grows the number of `.omg`
files under `samples/cli/facts/` (moving them within it is allowed) and rejects any
new file under `tests/omega/`. Pin a repair with a test on the changed code and,
when it needs a program, at most a single focused positive/negative pair kept
beside that test.

### Priority and continuous execution

1. **Choose the outcome and root-cause cluster:** Honor explicit user scope
   first. Otherwise repair any unclaimed active `main` regression, then take:
   - First, unassigned items in `TASKS.md`'s `## Priority (human-set only)` section.
   - Second, wide-blast-radius ratified surface migrations (`NESTED-MACHINE-UNIFICATION`,
     `CANONICALIZE-SCALAR-RANGE-CONTRACTS`, `STATIC-MACHINE-MIGRATION`,
     `BUILD-OWNED-TARGET-SELECTION`, `BUILD-OWNED-NATIVE-BINDINGS`,
     `TOP-LEVEL-BOUNDARY-REQUIREMENTS`).
   - Third, remaining `TASKS.md` items and failing `samples/` programs.
2. **Keep the assigned outcome on the warm worktree:** Reproduce the failure,
   repair its cause, and test and review the change before merge. After merge,
   confirm publication with the [landing guide](wiki/development/landing.md#confirm-publication-and-continue).
   A merge alone calls for no compiler build or test rerun.
   When running with subagents, the coordinator chooses work from `TASKS.md`,
   acquires worktrees and registers claims. Each worker traces the failure,
   repairs and validates the change, and lands its PR in its own context window.
   Continue any unmet part of the original assignment on the warm worktree.
   An unfinished handoff preserves the original acceptance and names the
   receiving session when one has accepted it.
   Acquisition starts a stable model-audit interval automatically. Before a new
   outcome on a clean warm slot, run `mbx xtask worktree begin`; it changes only
   that slot's audit boundary, never its source, cache or pool lease. Keep one
   interval through checkpoints and rebases. Existing active slots adopt a
   provable checkout-local boundary once; ambiguous history refuses rather than
   charging an unrelated outcome. `provenance --full-session` is an explicit
   whole-session audit, not a workaround for missing interval identity.

### Requirements and evidence

Every repair must preserve the end-to-end contracts in [wiki/spec/](wiki/spec/)
and [the compiler ownership map](source-rust/README.md#where-code-belongs):
- **Establishment & composition:** Construct guards, entry contracts, calls, and
  joins establish and preserve static facts without post-hoc caller hacks;
  positive and negative twins differ only in the construct's own input or contract.
- **Identity, lifetime & soundness:** Snapshots and loans track exact storage
  versions; proof preserves ownership, authority, and termination obligations
  while respecting the boundary between erased math and executable values.
- **Typed-to-Terminal Psi (lowering and checking within stage 01):** Lower and check constructs without loss in
  canonical target-neutral Terminal Psi (`PSITERM\0`), materializing explicit
  affine cleanup, restoration windows, crash routes, and ranking edges.
- **Native realization & execution:** Lower Terminal Psi through ABI calling
  plans, instruction selection, and ELF/Mach-O/PE emission so native execution
  agrees with Psi on the tested observations.

## Commands

`mbx` ([Mr Boxington](https://github.com/jdx/mr-boxington)) is **mandatory** for
all Rust builds, Clippy checks, and test runs. Direct `cargo build`, `cargo test`,
`cargo clippy`, or `cargo run` fallback is forbidden. Use the qualified
checksum-capable adapter pinned in `tools/mbx-version`.

For a Linux cloud baseline on current main, run
`bash tools/cloud_baseline.sh <command> [args...]`. It fetches main and uses
that revision's mbx in a separate checkout. For images older than the launcher,
use the [cloud startup command](wiki/development/testing.md#run-a-linux-cloud-baseline-on-current-main).

If mbx is missing or unqualified for the current checkout, install or update it
from the checkout root:

- **Linux / macOS / Cloud containers:**
  ```bash
  bash tools/agent_setup.sh && mbx xtask doctor
  ```
- **Windows PowerShell:**
  ```powershell
  & (Join-Path (git --exec-path) '../../../bin/bash.exe') tools/xtask/bootstrap-mbx.sh --install-dir "$HOME/.cargo/bin"
  mbx setup --yes; mbx xtask doctor
  ```

See [wiki/development/testing.md](wiki/development/testing.md) for full details. Always close stdin
with `< /dev/null` on non-interactive `omega` invocations so
background processes never hang on open stdin:

```bash
# Let mbx place the worktree target so it can rebuild it after collection.
CARGO_PROFILE_DEV_OPT_LEVEL=1 mbx build -p omega < /dev/null

# Run the tests on the code you changed, then the small area around it:
mbx test -p <crate> --test <test> < /dev/null

# Run focused sample native + Psi gate:
mbx xtask sample-gate --filter <sample_path> < /dev/null

# Run repository rules, formatting, lint, and boundary gates. Nothing lints for
# you before merge, so lint every crate you touched before you push:
mbx xtask check-rules --base origin/main --title "<lane>: <subject>"
mbx xtask fmt --check --base origin/main
mbx clippy -p <crate> --all-targets -- -D warnings
mbx xtask check-boundary
```

The pre-push format check covers changed Rust lines in workspace packages, including new files. Run `mbx xtask fmt --check` for the full workspace format check.

<a id="heavy-checks-run-on-the-lan-vms"></a>
**Heavy checks run on the LAN VMs, never on a GitHub runner.** Run
full-workspace tests and Clippy on a LAN VM under
`~/omega-agent/bin/vm-heavy` (`omega-linux` for the Linux gate); `mbx xtask vm`
starts such a job and shows its progress
([how](wiki/development/build_slot.md#on-the-test-vms)). The required
`admission` job stays light: it compiles and tests no Rust code, checks that
the shared specification exists and runs
`python3 -B tools/ci/check_recycling.py` on the checkout.

Park retired implementations and code awaiting piecemeal migration in
the [repository-root archive](_recycling/README.md). Mirror the former owner's
repository-relative path beneath `_recycling/`. Restore each piece in its active
owner and wire callers there. Active Rust source and Cargo manifests must keep
implementation paths outside the archive. The cloud sessions' warm build is
published by hand from `omega-linux`
([warm build](wiki/development/testing.md#warm-build-for-cloud-sessions)).

When changing a shared Rust API or helper, search the whole repository for its
consumers, including tests, examples and forwarding helpers. Run the affected
crates' required all-target Clippy early, before costly runtime or performance
runs, and reuse that result for final validation unless relevant source changes.

Passing tests on the change and on the small area around it end pre-merge
testing. A failure there is an investigation of that failure, never a wider
run.

Builds, test runs and compiled programs from every local agent share one
machine. The [host build budget](wiki/development/build_slot.md) caps the total
load of all agents so the machine never reaches 100%: the pinned mbx turns the
host's mbx scheduler budget (`scheduler.reserve_cpus` in the owner's global mbx
configuration) into a hard CPU ceiling at the lowest operating-system priority
for every process it starts, so run builds, tests and the programs they start
through mbx, and worker prompts need no thread caps. `mbx xtask doctor` reports
the ceiling; until the host has that setting, put `MBX_SCHEDULER_CPUS` in every
worker prompt. On the test VMs, run one heavy job per machine
and never change the settings of a job that is already running; the remote shim
refuses a second build or test run while one is active.

## The Psi/Omega firewall

- The [source compiler](source-rust/subsystems/compiler/source-to-psi/) owns target-neutral language semantics from source tokens through checked Psi publication. [Psi](source-rust/subsystems/compiler/psi/) owns the portable program and its encoding. [The interpreter](source-rust/subsystems/interpreter/) executes an admitted program with its supplied services. Keep target ABIs and machine encodings in the native compiler.
- The [native compiler](source-rust/subsystems/compiler/psi-to-native/) consumes published Psi and owns target lowering, ABI calling plans, objects and images. [Build](source-rust/subsystems/compiler/build/) runs authored planning stages and requests compiler products. [The package manager](source-rust/subsystems/package-manager/) acquires and reviews the package graph. The [CLI](source-rust/application/) dispatches those operations.

In each Rust compiler half, `pipeline/` contains the numbered transformations
and `representations/` contains the forms they exchange. Follow source and
native stage ordering from the source [driver](source-rust/subsystems/compiler/source-to-psi/driver.rs) and native [driver](source-rust/subsystems/compiler/psi-to-native/driver/mod.rs). Compile-time
constant and call evaluation belongs to the source compiler. The native root
entry and private callables use the shared body driver. The former component
export scheduler is in the root archive. Restored exports also use the shared
body driver. Requested object verification consumes evidence from that emission.
[Source map](source-rust/subsystems/compiler/source-to-psi/README.md) · [Native map](source-rust/subsystems/compiler/psi-to-native/README.md)

## Conventions and workflow

- **Product and subsystem ownership:** Ship one `omega` binary with
  `omega <subsystem> <action>` routing. Folders and entry points expose those
  subsystem responsibilities; do not grow separate tools or hidden command
  drivers for work belonging to the product.
- **Representation-to-representation pipeline:** Stages consume their declared
  input representation and produce the next (`X-to-Y`), or transform the same
  representation (`Y-to-Y`). Carry needed semantic facts in that representation;
  do not bypass the pipeline with sidecars, callbacks into earlier stages or
  source re-analysis. Private stage scratch is not a second inter-stage product.
- **Cleanup priority within an assigned area:** Repair broken stage topology,
  ownership and representation handoffs before local polish in that area.
  This does not displace unrelated user-assigned work.
- **Compositional lowering ([compiler pipeline](source-rust/subsystems/compiler/README.md)):**
  Generalize the touched operation across all value placements and delete the
  superseded code path in the same commit.
- **Data-oriented storage:** Dense IDs index flat tables (`Vec` / slice), not
  `HashMap`/`BTreeMap` (`tools/ratchets/dense_id_maps.txt`). Derived data is computed
  once by its stage owner and passed forward, never re-parsed from source text
  (`tools/ratchets/source_text_reads.txt`) or cached in ad-hoc mutable maps (`tools/ratchets/cache_state.txt`).
  This applies to membership sets too, including collections whose key type is
  inferred. Validate arena ownership and handle generations before indexing by
  slot. Size scratch storage to the operation's actual domain: a query over a
  small selected family should use that family's prepared membership or a
  sorted slice, rather than allocate a table spanning every symbol on each query.
  Favor compact arena-backed records with valid zero/default states where their
  contracts permit (ZII), without treating zero bytes as arbitrary valid Rust
  values. Avoid per-element small allocations and reserve from known or cheaply
  estimated sizes. For sustained growth, consider paged arenas instead of moving
  a growing contiguous `Vec`; use concurrent paged storage when independent
  producers need it. Keep a bounded/pre-sized `Vec` when it fits the workload;
  neither paging nor concurrency is a goal by itself.
- **Verify at trust boundaries ([native pipeline](source-rust/subsystems/compiler/psi-to-native/README.md)):**
  Default compilation does not re-verify its own stage output. Verifiers run
  in test harnesses, at foreign package boundaries, and on explicit
  [validator requests](wiki/spec/build/command_line.md#independent-stage-validators).
- When retiring a variant, inspect the other variants and contracts served by
  each deleted helper and preserve their paths. Successful compilation does
  not prove that shared checks survived.

## Scope and escalation

- Keep `TASKS.md` strictly as a remaining-work board: delete finished bullets
  in the same commit as the repair. A ratified decision may add its
  implementation item.
- When a ruling or design decision changes how the compiler is built or how
  work is judged, the same PR updates or deletes every instruction that
  described the old way: this file, `.agents/skills/`, `TASKS.md` and the
  READMEs agents read. Search for the old terms before landing. Two
  instructions that disagree make agents pick one at random.
- When a genuine specification ambiguity blocks progress, record the exact
  question in `OWNER_QUESTIONS.md` and cite it in the relevant README rather
  than inventing unratified language semantics.
- Before you raise a question or report work as waiting on the owner, run
  `git fetch origin` and re-read `OWNER_QUESTIONS.md`,
  `OWNER_QUESTIONS_BOOTSTRAP.md` and the pull requests merged since you
  started. Rulings land while long sessions work; a question already answered
  on `origin/main` is not open.
- Repair obvious code-quality and architecture defects under the existing rules;
  they do not need owner sign-off. If a concrete defect reaches beyond the
  current repair (duplicated mechanisms, pipeline bypasses, unsound fallbacks,
  misleading tests or wasteful storage), record it in the
  [code repair queue](OWNER_CODE_CONCERNS.md) in the PR already being landed and
  keep working. Agents may take those repairs and remove verified resolved or
  stale entries. Keep plain implementation gaps in the existing `TASKS.md`
  outcome instead; escalate only genuine language or high-level architecture
  decisions, not routine refactors.

## Sessions and subagents

- **Coordinator and workers:** The coordinator chooses work and starts
  workers, each in its own worktree. You are trusted with your task end to
  end: review your own diff for soundness, regression coverage and cost, then
  squash-merge it when `admission` passes. Waiting for permission you already
  have only slows the project down. A coordinator may look at any diff; workers
  do not wait for it. A single-agent session works the same way.
- **Reactive wakeup only:** Never poll in loops, `sleep`, or schedule periodic
  timers waiting on subagents or background tasks; end the turn and let completion
  notifications wake the session.

## Commit naming

Use `lane: statement` (lowercase, declarative, max 85 characters, no trailing
period, no tool attribution or `Co-Authored-By` trailers). `cargo xtask check-rules`
reads the allowed lanes from this table:

| Lane | Responsibility and current path anchors |
| --- | --- |
| `alpha`, `beta`, `gamma`, `delta`, `epsilon` | The matching language under `bootstrap/<language>/`, its compilers and interpreters, plus the associated reference tooling and tests outside that directory. |
| `bootstrap-omega` | The Epsilon-written first Omega compiler D in `bootstrap/omega/compiler/` and its tests. Never shorten to `omega`. |
| `bootstrap` | Cross-rung reconstruction, trust-chain edges, artifact provenance, `tests/bootstrap/`, and `tools/bootstrap/`. |
| `psi` | Target-neutral source semantics and Psi publication in `source-rust/subsystems/compiler/source-to-psi/`, portable Psi in its sibling `psi/`, and Omega-written `source/subsystems/compiler/source_to_psi/`. |
| `omega` | Native planning, representations and verification in `source-rust/subsystems/compiler/psi-to-native/`, or Omega-written `source/subsystems/compiler/alpha/`. |
| `backend` | Native targets and ABIs under `psi-to-native/targets/`, and encoding, layout, objects and images in native stages 11–16. |
| `compiler` | The public compiler entry in `source-rust/subsystems/compiler/compiler.rs` and contracts spanning the two pipelines. |
| `build` | Project planning, Build execution and settings in `source-rust/subsystems/compiler/build/`, and authored `build.omg` declarations. |
| `packages` | Package acquisition, graphs, review and update in `source-rust/subsystems/package-manager/`. |
| `cli` | Command parsing, flags, help and dispatch in `source-rust/application/src/`. |
| `tooling` | Shipped developer tooling and shared host support in `source-rust/foundation/`. |
| `library` | Bundled packages in `source/library/` (`core`, `allocation`, `system`, etc.). |
| `samples` | Standalone sample content in `samples/`. A regression fixture for a compiler fix uses that compiler stage's lane. |
| `tests` | Shared test infrastructure, kept fixtures, or multi-owner gates. Stage-specific tests use their stage's lane. |
| `tools` | Repository maintenance tooling, CI/bootstrap helpers, ratchets, and `mbx xtask` (`tools/xtask/`). |
| `repo` | Repository workflow, `AGENTS.md`, `.agents/skills/`, CI, and workspace configuration. |
| `docs` | Explanatory documentation-only changes in `wiki/`, READMEs, or guides. |

## Hard rules

- **Isolated detached worktree pool (`.agents/worktree/worker-N`); never touch primary checkout; no local branches:**
  Claim an idle pool slot (`worker-1`, `worker-2`, …) via
  `cargo xtask worktree acquire <lane>-<topic>`, which resets an idle `worker-N`
  checkout to `origin/main` in place. Its fixed path lets later tasks reuse
  build outputs retained by mbx. In a cloud session the slot instead starts at
  the commit its restored build was published for; rebase onto `origin/main`
  only when the task needs something newer. Release finished slots with
  `cargo xtask worktree release` or `cargo xtask worktree clean`. Push via
  `git push origin HEAD:refs/heads/work/<lane>-<topic>`.
- Keep the `.omg` set under `samples/cli/facts/` frozen and keep the retired
  `tests/omega/` directory empty.
- **No direct pushes to `main` and no chasing `main`:** Land only through
  squash-merged PRs. Rebase only on actual conflicts or
  needed prerequisites.
- **Claims are informational (`cargo xtask claims`):** Register claims
  before editing shared paths. For work outside the registered paths, take a
  supplemental claim; `claims note` updates progress and the lease, not the
  path roster. Keep related claims through the original acceptance and
  reconcile overlapping edits at rebase.
- **Keep sample coverage out of the shipped CLI.**

## Design references

- [Omega Language Guide](wiki/language_guide/language_guide.md)
- [Compiler ownership and pipeline](source-rust/README.md#follow-a-compilation)
- [Documentation index](wiki/README.md)
- [Terminal Psi product contract](wiki/spec/terminal-psi/product.md)
- [Optimization phases](wiki/spec/build/optimizations.md#phase-and-product-boundaries)
- [Rust compiler completion](wiki/drafts/reference/rust_compiler_completion.md)
