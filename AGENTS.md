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
- `bootstrap/{0_alpha,…,5_omega}/` — the trust-minimizing bootstrap chain (`bootstrap/proofs/` holds Gamma-written derivation checking; scripts resolve locations via `tools/bootstrap/paths.sh`).

**The goal is a complete compiler that ships.** Follow `.agents/skills/advance/SKILL.md`
(`/advance`) to execute work, `.agents/skills/coordinator-review/SKILL.md` to
review candidate diffs, and [Rust compiler completion](wiki/drafts/reference/rust_compiler_completion.md)
for the six release gates (`repository`, `source semantics`, `build and packages`,
`native matrix`, `diagnostics`, and `representative programs`) across all four
hosted targets (`linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`).

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

Check claims against the actual submitted or landed diff and observed behavior;
inspect unclear code or examples instead of inventing a plausible benefit.
Rewrite the explanation when scope changes. Keep material limits beside the
result they qualify. Put validation and detailed provenance afterward, retaining
exact commands, source/host identities, failures and unverified cases. Historical
results remain tied to their recorded sources, not promoted to fresh validation.

## Law of completion

We are capable of taking an uncompilable sample through every necessary compiler
layer to a compilable, executable, validated program landed on main. The agent
owns that entire transformation, not just the first repair. Difficulty is a
reason to investigate and persist, not to abandon the work or redefine done.

Follow the river to its end: carry the capability through every necessary
compiler, library, and prerequisite repair in your isolated worktree. Do not
divide work into stage-only tasks or wait for another session to finish missing
functionality. Checkpoints, archives, or JSON reports preserve evidence; none
completes the work.

Ownership of open PRs survives session changes. At startup or resumption, check
PRs you opened in earlier sessions against retained worktree and claim records;
keep each URL, submitted head, remaining failure/acceptance and next action in
its claim note. Carry authorized work through verified merge using the landing
roles below. Close a PR only for a recorded withdrawal or supersession, not age.

### Decision-making framing for compiler completion

1. **Drive the six release gates and sweep whole root-cause clusters, never 1-off wins:**
   When you fix a mechanism or migration gap, sweep and re-record **all**
   pinned regressions in `tests/omega/corpus_outcomes.txt`
   (`python3 tools/corpus_gate.py --list-pinned-regressions`) and failing
   programs in `samples/` (`python3 tools/sample_native_gate.py`) that share
   that root cause in the same PR.
2. **Fail-closed semantics over permissive fallbacks:** Never make a positive
   program pass by weakening a proof obligation, returning `true` when a
   lookup/rebase returns `None`, or silently erasing an unresolved construct.
3. **Exact projected-path scoping:** Track facts, loans, and restoration
   obligations at the exact projected place (`root.field` or case variant
   payload), invalidating overlapping writes without discarding provably
   disjoint sibling storage.
4. **Full pipeline carry-through (`Psi 05 -> 06 -> 08 -> Native`):** Carry every
   semantic repair through Checked Psi (`05`), Lowered/Terminal Psi (`06–08`),
   and native execution (`omega run --both`, exit 0).
5. **Monotonic board closure & clean golden records:** Delete completed bullets
   from `TASKS.md` outright (never rewrite into `"preserving / keeping / now
   passes"` history prose). Commit all code and fixture files before running
   `corpus_gate.py --record --filter <fixtures>` so `corpus_outcomes.txt`
   never records a dirty `@<sha>+wt` stamp, a `pass/`/`run/` row as `rejected`,
   or a `fail/` row as `unexpected`.

## The sample loop

When validating a language mechanism across value placements, you are carrying
one object through the compiler. Verify that same object across the nine
orthogonal transport places: `local`, `field`, `parameter`, `call_result`,
`match_join`, `const_name`, nested `machine` (`other_state/`), another
`machine` (`other_machine/`), and a `Build`-selected target implementation
(`target_body/`), with `ProgramEntry` bound for all four hosted targets and
passing `omega run --both` (exit 0) alongside its negative twin. The pinned
diagnostic, and the finite `tests/omega` facts fixture, is a photograph of that
object. Done means the compiler still holds that same object. Changing the fact,
the bound, or the operation so a place passes is repainting that photograph:
a different object, and a green check on it does not count.

### No new `samples/cli/facts` folder bloat

`samples/cli/facts/` and `tests/omega/{pass,fail,run}/facts/` are a **frozen
finite set**. `tools/check_rules.py` mechanically rejects adding any new `.omg`
files under `facts/` and caps new `.omg` files anywhere in the repository at
**at most 6 per PR** (`MAX_NEW_OMG_FILES = 6`). Pin repairs by flipping
existing failing rows in `tests/omega/corpus_outcomes.txt` and `samples/`, or
with at most a single focused positive/negative fixture pair under
`tests/omega/{pass,fail,run}/<behavior>/`.

### Priority and continuous execution

1. **Choose the outcome and root-cause cluster:** Honor explicit user scope
   first. Otherwise repair any unclaimed active `main` regression, then take:
   - First, unassigned items in `TASKS.md`'s `## Priority (human-set only)` section.
   - Second, wide-blast-radius ratified surface migrations (`NESTED-MACHINE-UNIFICATION`,
     `CANONICALIZE-SCALAR-RANGE-CONTRACTS`, `STATIC-MACHINE-MIGRATION`,
     `BUILD-OWNED-TARGET-SELECTION`, `BUILD-OWNED-NATIVE-BINDINGS`,
     `TOP-LEVEL-BOUNDARY-REQUIREMENTS`) and the top failing diagnostic clusters
     in `tests/omega/corpus_outcomes.txt`.
   - Third, remaining `TASKS.md` items and failing `samples/` programs.
2. **Execute `/advance` and chain on the warm worktree:** Follow the 5-step
   workflow in [.agents/skills/advance/SKILL.md](.agents/skills/advance/SKILL.md).
   When running with subagents, the coordinator runs Step 1 (triage from
   `TASKS.md` and `tests/omega/corpus_outcomes.txt`, worktree creation, and
   claim registration) and dispatches workers immediately so each worker owns
   Steps 2–5 (all `omega-rust/` source reading, root-cause tracing, repair,
   validation, and draft PR) in its own context window. After landing or
   handing off a slice, continue the next requirement of the same capability on
   the warm worktree.

### Requirements and evidence

Every repair must preserve the end-to-end contracts in [wiki/spec/README.md](wiki/spec/README.md)
and [omega-rust/pipeline.md](omega-rust/pipeline.md):
- **Establishment & composition:** Construct guards, entry contracts, calls, and
  joins establish and preserve static facts without post-hoc caller hacks;
  positive and negative twins differ only in the construct's own input or contract.
- **Identity, lifetime & soundness:** Snapshots and loans track exact storage
  versions; proof preserves ownership, authority, and termination obligations
  while respecting the boundary between erased math and executable values.
- **Checked-to-Terminal Psi (`Psi 06–08`):** Lower constructs without loss into
  canonical target-neutral Terminal Psi (`PSITERM\0`), materializing explicit
  affine cleanup, restoration windows, crash routes, and ranking edges.
- **Native realization & execution:** Lower Terminal Psi through ABI calling
  plans, instruction selection, and ELF/Mach-O/PE emission so native execution
  agrees with Terminal Psi (`omega run --both`).

## Commands

See [tools/testing.md](tools/testing.md) for full details. Always close stdin
with `< /dev/null` on non-interactive `omega` / `corpus_runner` invocations so
background processes never hang on open stdin:

```bash
# Build warm optimized development compiler in worktree target/:
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx build -p omega < /dev/null

# Run focused corpus verification & record (only on a clean committed worktree):
python3 tools/corpus_gate.py --filter <fixtures> < /dev/null
python3 tools/corpus_gate.py --record --filter <fixtures> < /dev/null

# Run focused sample native + Terminal Psi gate:
python3 tools/sample_native_gate.py --filter <sample_path> < /dev/null

# Run repository rules, formatting, and lint gates:
python3 tools/check_rules.py --base origin/main --title "<lane>: <subject>"
python3 tools/fmt.py --check
```

Use `cargo` when `mbx` is unavailable. Never run an unfiltered corpus sweep
locally (`corpus_gate.py` enforces a 200-fixture local selection budget).

## The Psi/Omega firewall

- **`psi/` (`omega-rust/psi/`, `source/psi/`)** owns target-neutral language
  semantics from source tokens through canonical Terminal Psi (`PSITERM\0`) and
  the Terminal Psi interpreter. `psi/` must never import `omega/` crates, target
  ABIs, or machine encodings.
- **`omega/` (`omega-rust/omega/`, `source/omega/`)** consumes Checked/Terminal
  Psi and owns build orchestration, package management, target lowering, ABI
  calling plans, object/image emission, and the CLI driver.

## Conventions and workflow

- **Compositional lowering ([pipeline.md](omega-rust/pipeline.md#compositional-lowering)):**
  Generalize the touched operation across all value placements and delete the
  superseded code path in the same commit.
- **Data-oriented storage:** Dense IDs index flat tables (`Vec` / slice), not
  `HashMap`/`BTreeMap` (`tools/dense_id_maps.txt`). Derived data is computed
  once by its stage owner and passed forward, never re-parsed from source text
  (`tools/source_text_reads.txt`) or cached in ad-hoc mutable maps (`tools/cache_state.txt`).
- **Verify at trust boundaries ([pipeline.md](omega-rust/pipeline.md#verification-placement)):**
  Stages do not re-verify output the same compile just produced; verifiers run
  in corpus/test harnesses and at foreign package boundaries.

## Scope and escalation

- Keep `TASKS.md` strictly as a remaining-work board: delete finished bullets
  monotonically in the same commit as the repair.
- When a genuine specification ambiguity blocks progress, record the exact
  question in `OWNER_QUESTIONS.md` and cite it in the relevant README rather
  than inventing unratified language semantics.

## Sessions and subagents

- **Division of labor (Coordinator vs. Worker):**
  - **Coordinator owns triage and review; workers own investigation and implementation:**
    Subagents exist so each capability outcome or diagnostic cluster gets its
    own isolated worktree and clean context window for compiler reading and
    root-cause tracing. When running with subagents, the coordinator selects
    outcomes from `TASKS.md` and `tests/omega/corpus_outcomes.txt` (by board
    item and diagnostic cluster, without reading `omega-rust/` source files to
    pre-diagnose the bug), creates the worktrees and claims, and dispatches
    workers immediately. Workers execute Steps 2–5 of `/advance`, push their
    branch, open a **draft** PR (`gh pr create --draft`), and stop
    (`candidate_ready`). Workers never mark a PR ready or run `tools/landing.py`
    themselves. The coordinator audits `gh pr diff <pr>` with
    [.agents/skills/coordinator-review/SKILL.md](.agents/skills/coordinator-review/SKILL.md)
    before running `python3 tools/landing.py land <pr> --expected-head <sha>`.
  - **Standalone single-agent session:** One agent executes Steps 1–5 of
    `/advance`, self-reviews against `coordinator-review`, and runs
    `python3 tools/landing.py land <pr> --expected-head <sha>`.
- **Reactive wakeup only:** Never poll in loops, `sleep`, or schedule periodic
  timers waiting on subagents or background tasks; end the turn and let completion
  notifications wake the session.

## Commit naming

Use `lane: statement` (lowercase, declarative, max 85 characters, no trailing
period, no tool attribution or `Co-Authored-By` trailers). `check_rules.py`
reads the allowed lanes from this table:

| Lane | Responsibility and current path anchors |
| --- | --- |
| `alpha`, `beta`, `gamma`, `delta`, `epsilon` | The corresponding language rung in `bootstrap/<rung>/`, including its compiler/evaluator, Rust reference tooling, and rung-specific tests. |
| `bootstrap-omega` | The Epsilon-written first Omega compiler D in `bootstrap/5_omega/` and its tests. Never shorten to `omega`. |
| `bootstrap` | Cross-rung reconstruction, trust-chain edges, artifact provenance, `tests/bootstrap/`, and `tools/bootstrap/`. |
| `psi` | Target-neutral source semantics through Terminal Psi in `omega-rust/psi/` or `source/psi/`. |
| `omega` | The Terminal-Psi-consuming compiler stage in `omega-rust/omega/compilation/{representations,pipeline,realization}/` or `source/omega/`. |
| `backend` | Target, ISA, ABI, object/image encoding, layout, and execution primitives in `omega-rust/omega/compilation/backend/`. |
| `compiler` | Product compilation coordination in `omega-rust/omega/compilation/compiler/` and `native-production/`, or an inseparable contract change spanning Psi and Omega. |
| `build` | Build system in `omega-rust/omega/build-system/` (`orchestration/`, `build-settings/`) and `build.omg` declarations. |
| `packages` | Package acquisition, graphs, review, admission, and update workflows in `omega-rust/omega/packages/`. |
| `cli` | Command parsing, flags, help, and dispatch in `omega-rust/omega/application/cli/src/`. |
| `tooling` | Shipped developer tools and shared host support in `omega-rust/omega/foundation/`. |
| `library` | Bundled packages in `source/library/` (`core`, `alloc`, `std`, etc.). |
| `samples` | Standalone sample content in `samples/`. A regression fixture for a compiler fix uses that compiler stage's lane. |
| `tests` | Shared test infrastructure, corpus registration, or multi-owner gates. Stage-specific tests use their stage's lane. |
| `tools` | Repository utilities in `tools/`. |
| `repo` | Repository workflow, `AGENTS.md`, `.agents/skills/`, CI, and workspace configuration. |
| `docs` | Explanatory documentation-only changes in `wiki/`, READMEs, or guides. |

## Hard rules

- **Isolated detached worktrees only; never touch primary checkout; no local branches:**
  Work exclusively in `.agents/worktree/<short-name>` created via
  `git worktree add --detach .agents/worktree/<short-name> origin/main`. Push
  via `git push origin HEAD:refs/heads/work/<short-name>`.
- **No `facts/` bloat (`MAX_NEW_OMG_FILES = 6`):** Never add `.omg` files under
  `samples/cli/facts/` or `tests/omega/{pass,fail,run}/facts/`, and never add
  more than 6 `.omg` files in a PR.
- **No direct pushes to `main` and no chasing `main`:** Land only through
  reviewed PRs and `tools/landing.py`. Rebase only on actual conflicts or
  needed prerequisites.
- **Claims are informational (`python3 tools/claims.py`):** Register claims
  before editing shared paths; reconcile overlapping edits at rebase.
- **Keep sample coverage out of the shipped CLI.**

## Design references

- [Omega Language Guide](wiki/language_guide/language_guide.md)
- [Compiler ownership and pipeline](omega-rust/pipeline.md)
- [Documentation index](wiki/README.md)
- [Terminal Psi product contract](wiki/spec/terminal-psi/product.md)
- [Optimization phases](wiki/spec/build/optimizations.md#phase-and-product-boundaries)
- [Rust compiler completion](wiki/drafts/reference/rust_compiler_completion.md)
