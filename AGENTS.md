# AGENTS.md

This is the canonical instruction file for every coding agent working in this
repository. Agent-specific instruction files must point here rather than copy
these rules. Repository skills are canonical in `.agents/skills/`; read and
edit them there. Pi and Codex discover that directory directly. For Claude
Code, create an ignored checkout-local link when `.claude/skills` is absent:
- Windows PowerShell: `New-Item -ItemType Directory -Force .claude; New-Item -ItemType Junction -Path .claude/skills -Target (Resolve-Path .agents/skills)`
- macOS/Linux shell: `mkdir -p .claude && ln -s ../.agents/skills .claude/skills`

Omega is a proof-checked systems language whose programs are data-oriented
state machines:
- `omega-rust/` — the Rust reference producer and differential comparator.
- `source/psi/` + `source/omega/` — the Omega-written product compiler.
- `source/library/` — bundled `omega::` packages (`name.omg` or `name/mod.omg`).
- `bootstrap/interpreted/{0_alpha,…,5_omega}/` — the trust-minimizing bootstrap chain (`bootstrap/proofs/` holds Gamma-written derivation checking; scripts resolve locations via `tools/bootstrap/paths.sh`).

**Public naming:** Follow [public names](wiki/README.md#public-names): the portable
IR, artifact, interpreter and CLI subsystem are **Psi**. **Terminal Psi** names
only the internal compiler stage/representation. Keep internal paths and wire
identifiers intact; do not export that stage label into help, reports or guides.
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
- **Hosted target**: one of the four operating-system/CPU pairs Omega must
  produce native programs for (listed below).
- **Release gate**: one of the six areas that must all be finished before the
  compiler ships (listed below).
- **Lane**: the area prefix of a commit title, such as `psi:` or `docs:`
  (see [commit naming](#commit-naming)).

**The goal is a complete compiler that ships.** Follow `.agents/skills/advance/SKILL.md`
(`/advance`) to execute work, `.agents/skills/coordinator-review/SKILL.md` to
review candidate diffs, and [Rust compiler completion](wiki/drafts/reference/rust_compiler_completion.md)
for the six release gates (`repository`, `source semantics`, `build and packages`,
`native matrix`, `diagnostics`, and `representative programs`) across all four
hosted targets (`linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`).

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

To rewrite existing text in plain language, follow
[plain-writing](.agents/skills/plain-writing/SKILL.md); its script checks that
the rewrite kept every identifier and grew no longer.

Check claims against the actual submitted or landed diff and observed behavior;
inspect unclear code or examples instead of inventing a plausible benefit.
Rewrite the explanation when scope changes. Keep material limits beside the
result they qualify. Put validation and detailed provenance afterward, retaining
exact commands, source/host identities, failures and unverified cases. Historical
results remain tied to their recorded sources, not promoted to fresh validation.

End every PR description and commit body with one portable model-audit line
generated by `cargo xtask provenance`:
`Agent: <name> | Model: <model> | Effort: <effort> | Tokens: <in> in (<cached> cached), <out> out | Steps: <N> | Time: <duration> | Steers: <N>`,
where `<name>` is `Claude Code`, `Codex`, `Cursor`, `Devin`, `Antigravity`,
`Dot`, or `Human`.
Several agents share each GitHub
account and GitHub squash merges preserve the PR body on `main`, so this line
records the exact model, reasoning effort, token cost, step count, active
duration, and human steers for every landed commit. `cargo xtask check-rules --body-file` checks it on every PR.
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

That includes fixes in other compiler stages, the bundled library, or anything
else the program depends on, all made in your own worktree. Do not split the
work into one task per stage, and do not wait for another session to supply a
missing piece. Writing up a bug does not fix it. The work is done when the
program runs.

The specification is the answer key. Read the page in `wiki/spec/` that covers
the feature and build what it says. Test what you changed, then merge it;
`main` is tested again after every merge.

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
   An [assumed binding](wiki/spec/build/assumed_binding_contracts.md) lets a
   program skip a check by asserting that the check holds. Treat it as
   unavailable in ordinary Omega code. Never add one to silence a borrow or
   lifetime error, skip a proof or contract, work around a compiler
   limitation, or make a test pass; fix the program, contract, proof machinery
   or compiler instead. Being asked to "make it compile" does not permit one. A
   genuine exception needs a concrete reason the program must go beyond what
   the compiler can check, and explicit approval from the program's top-level
   (root) package.
3. **Track each field separately:** Record facts, borrows ("loans") and
   values that must be put back at the exact field path they apply to, such as
   `root.field` or one variant's payload. A write to `a.x` invalidates what is
   known about `a.x` and about `a` as a whole, but not about a separate field
   `a.y`.
4. **Carry every fix all the way to machine code:** A fix to what a program
   means must work in each later stage: the lowered-form checker, Terminal Psi,
   and the native program. Prove it with `omega run --both`, which runs the
   program in the Psi interpreter and natively and requires both to exit 0.
5. **Board closure:** Delete completed bullets from `TASKS.md` outright (never
   rewrite into `"preserving / keeping / now passes"` history prose).
6. **Generalize instead of adding special cases:** If your fix needs a second
   copy of a data structure, an extra case, or a branch for one kind of
   location, the existing mechanism is too narrow. Widen that mechanism so one
   change fixes every program with the same cause, and do it in the same PR.

## The sample loop

A language feature should work wherever a value can appear (a local, a field,
an argument, a return value, and so on) and on all four hosted targets, with a
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
([tests/README.md](tests/README.md) says how to recover a fixture from
history), and nothing replaces it.

A program a test reads lives beside that test, under `tests/fixtures/`. The
test pins the exact result the program must produce (its output or its
error), which records what the program asks of the compiler. Do not make a test pass by changing the
program: a different fact, a looser bound or a different operation turns it
into a different test, and passing that one proves nothing about the original.

### No `.omg` file bloat

`samples/cli/facts/` is a **frozen finite set**, and `tests/omega/` is gone.
`cargo xtask check-rules` rejects any change that grows the number of `.omg`
files under `samples/cli/facts/` (moving them within it is allowed), rejects any
new file under `tests/omega/`, and caps new
`.omg` files anywhere in the repository at **at most 6 per PR**
(`MAX_NEW_OMG_FILES = 6`). Pin a repair with a test on the changed code and,
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
2. **Execute `/advance` and chain on the warm worktree:** Follow the 5-step
   workflow in [.agents/skills/advance/SKILL.md](.agents/skills/advance/SKILL.md).
   When running with subagents, the coordinator runs Step 1 (triage from
   `TASKS.md`, worktree creation, and claim registration) and dispatches workers immediately so each worker owns
   Steps 2–5 (all `omega-rust/` source reading, root-cause tracing, repair,
   validation, and PR) in its own context window. After landing or
   handing off a slice, continue the next requirement of the same capability on
   the warm worktree.
   Acquisition starts a stable model-audit interval automatically. Before a new
   outcome on a clean warm slot, run `mbx xtask worktree begin`; it changes only
   that slot's audit boundary, never its source, cache or pool lease. Keep one
   interval through checkpoints and rebases. Existing active slots adopt a
   provable checkout-local boundary once; ambiguous history refuses rather than
   charging an unrelated outcome. `provenance --full-session` is an explicit
   whole-session audit, not a workaround for missing interval identity.

### Requirements and evidence

Every repair must preserve the end-to-end contracts in [wiki/spec/](wiki/spec/)
and [omega-rust/pipeline.md](omega-rust/pipeline.md):
- **Establishment & composition:** Construct guards, entry contracts, calls, and
  joins establish and preserve static facts without post-hoc caller hacks;
  positive and negative twins differ only in the construct's own input or contract.
- **Identity, lifetime & soundness:** Snapshots and loans track exact storage
  versions; proof preserves ownership, authority, and termination obligations
  while respecting the boundary between erased math and executable values.
- **Typed-to-Terminal Psi (`Psi 05–06`):** Lower and check constructs without loss in
  canonical target-neutral Terminal Psi (`PSITERM\0`), materializing explicit
  affine cleanup, restoration windows, crash routes, and ranking edges.
- **Native realization & execution:** Lower Terminal Psi through ABI calling
  plans, instruction selection, and ELF/Mach-O/PE emission so native execution
  agrees with Psi (`omega run --both`).

## Commands

`mbx` ([Mr Boxington](https://github.com/jdx/mr-boxington)) is **mandatory** for
all Rust builds, Clippy checks, and test runs. Direct `cargo build`, `cargo test`,
`cargo clippy`, or `cargo run` fallback is forbidden. If `mbx` is missing or older
than the version pinned in `tools/mbx-version`, install or update it
immediately before building (run these from the checkout root):

- **Linux / macOS / Cloud containers:**
  ```bash
  bash tools/agent_setup.sh && mbx xtask doctor
  ```
- **Windows PowerShell:**
  ```powershell
  $v = (Get-Content tools/mbx-version).Trim(); Invoke-WebRequest "https://github.com/jdx/mr-boxington/releases/download/v$v/mbx-x86_64-pc-windows-msvc.zip" -OutFile "$env:TEMP\mbx.zip"; Expand-Archive "$env:TEMP\mbx.zip" "$HOME\.cargo\bin" -Force; mbx setup --yes; mbx xtask doctor
  ```

See [tools/docs/testing.md](tools/docs/testing.md) for full details. Always close stdin
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
mbx xtask fmt --check
mbx clippy -p <crate> --all-targets -- -D warnings
mbx xtask check-boundary
```

The only pre-merge check is `admission` (rules and formatting, about a
minute); `main` is checked after every merge.

Passing tests on the change and on the small area around it end pre-merge
testing. A failure there is an investigation of that failure, never a wider
run.

Builds, test runs and compiled programs from every local agent share one
machine. Before the first build on a host you share with its owner or with
other agents, apply the [host build budget](tools/docs/build_slot.md): cap the total
load of all agents so the machine never reaches 100%, run at the lowest
operating-system priority on a workstation its owner is using, and write the
budget into every worker prompt. On the test VMs, run one heavy job per machine
and never change the settings of a job that is already running; the remote shim
refuses a second build or test run while one is active.

## The Psi/Omega firewall

- **`psi/` (`omega-rust/psi/`, `source/psi/`)** owns target-neutral language
  semantics from source tokens through canonical Terminal Psi (`PSITERM\0`) and
  the Psi interpreter. `psi/` must never import `omega/` crates, target
  ABIs, or machine encodings.
- **`omega/` (`omega-rust/omega/`, `source/omega/`)** consumes Checked/Terminal
  Psi and owns build orchestration, package management, target lowering, ABI
  calling plans, object/image emission, and the CLI driver.

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
- **Compositional lowering ([pipeline.md](omega-rust/pipeline.md#compositional-lowering)):**
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
- **Verify at trust boundaries ([pipeline.md](omega-rust/pipeline.md#verification-placement)):**
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
  end: review your own diff the way a careful reviewer would
  ([coordinator-review](.agents/skills/coordinator-review/SKILL.md)), then
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
| `alpha`, `beta`, `gamma`, `delta`, `epsilon` | The corresponding language rung in `bootstrap/interpreted/<N>_<rung>/`, including its compiler/evaluator, Rust reference tooling, and rung-specific tests. |
| `bootstrap-omega` | The Epsilon-written first Omega compiler D in `bootstrap/interpreted/5_omega/` and its tests. Never shorten to `omega`. |
| `bootstrap` | Cross-rung reconstruction, trust-chain edges, artifact provenance, `tests/bootstrap/`, and `tools/bootstrap/`. |
| `psi` | Target-neutral source semantics through Terminal Psi in `omega-rust/psi/` or `source/psi/`. |
| `omega` | The Terminal-Psi-consuming compiler stage in `omega-rust/omega/compilation/{representations,pipeline,realization}/` or `source/omega/`. |
| `backend` | Target, ISA, ABI, object/image encoding, layout, and execution primitives in `omega-rust/omega/compilation/backend/`. |
| `compiler` | Product compilation coordination in `omega-rust/omega/compilation/compiler/` and `native-compiler/`, or an inseparable contract change spanning Psi and Omega. |
| `build` | Build system in `omega-rust/omega/build-system/` (`orchestration/`, `build-settings/`) and `build.omg` declarations. |
| `packages` | Package acquisition, graphs, review, admission, and update workflows in `omega-rust/omega/packages/`. |
| `cli` | Command parsing, flags, help, and dispatch in `omega-rust/omega/application/src/`. |
| `tooling` | Shipped developer tooling and shared host support compiled into `omega` (`omega-rust/omega/foundation/`). |
| `library` | Bundled packages in `source/library/` (`core`, `alloc`, `std`, etc.). |
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
- **No `.omg` bloat (`MAX_NEW_OMG_FILES = 6`):** Never grow the `.omg` files
  under `samples/cli/facts/`, never add a file under `tests/omega/`, and never add
  more than 6 `.omg` files in a PR.
- **No direct pushes to `main` and no chasing `main`:** Land only through
  squash-merged PRs. Rebase only on actual conflicts or
  needed prerequisites.
- **Claims are informational (`cargo xtask claims`):** Register claims
  before editing shared paths; reconcile overlapping edits at rebase.
- **Keep sample coverage out of the shipped CLI.**

## Design references

- [Omega Language Guide](wiki/language_guide/language_guide.md)
- [Compiler ownership and pipeline](omega-rust/pipeline.md)
- [Documentation index](wiki/README.md)
- [Terminal Psi product contract](wiki/spec/terminal-psi/product.md)
- [Optimization phases](wiki/spec/build/optimizations.md#phase-and-product-boundaries)
- [Rust compiler completion](wiki/drafts/reference/rust_compiler_completion.md)
