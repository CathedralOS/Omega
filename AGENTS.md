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

**The goal is a compiler that ships.** Choose an unfinished capability or
release-gate blocker from `TASKS.md`, `tests/omega/corpus_outcomes.txt`, or
`samples/`, then carry its repair end to end ([the completion loop](#the-sample-loop)),
not a file or a stage. Demonstrate compiler work through real programs and
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

## Explain the change

Write for a reader who knows what the project does but has not read the diff
or worked on this subsystem.
Use an inverted pyramid in PR descriptions, commit bodies, progress/final
reports, explanatory documentation and comments. Start with the specific
situation and what changes for the person using or maintaining the software.
The first one or two sentences must make the problem and result clear in
ordinary language. Apply this to the title too, within the commit-lane rules.
Choose that opening from the behavior the change affects, not the names of
the mechanisms edited. Say plainly when the intended result is still unfinished.

An opening such as "Preserve post-call facts through returned scalar views"
requires subsystem knowledge. "Keep value information after calls" removes
some jargon but still leaves the reader to work out the benefit. Build the
explanation in layers, as in this example:

"A program can now subtract 9 after calling a helper that guarantees the number
is at least 9. The compiler used to forget that guarantee and reject the
calculation."

"The problem occurred when the program kept a reference to the number in an
array. Updating an array entry and updating the number it points to are
different operations, but the compiler was treating uncertainty about the
entry as uncertainty about the number. It now tracks those locations separately."

Only after that explanation does the reader have a reason to learn which
stored-reference paths and call-completion checks changed. A result such as
"the example now returns 73" needs the same treatment: explain what that value
demonstrates. For internal work, name the maintenance or development benefit
without inventing an end-user feature.

Each paragraph should answer a question the preceding text raises. Explain the
example before showing unfamiliar syntax, and the cause before naming the
mechanism that fixes it. A plain opening followed by a list of internal terms
still fails. Readers should be able to stop after a paragraph with a useful
understanding, rather than needing a later paragraph to decode it.

Simplify the explanation, not just the vocabulary. "Permission information" is
still opaque unless the reader learns who may read or write which data and for
how long. Once a term is explained, use its canonical name consistently. Source
comments begin with the local reason for the code, then the constraint that
must hold. For example: "A saved copy must keep pointing at the original data
when the caller replaces the variable." Then give any necessary details about
how that is maintained. Use the surrounding code's context; do not narrate
obvious operations or add a tutorial.

Put validation and detailed provenance after the explanation. Keep a limitation
beside any result it materially qualifies, and preserve exact evidence
identities, failures and unverified cases. Link long records or use details
sections. A small change may need only one paragraph; do not inflate it to fill
a template.

Check accuracy and clarity separately. Verify claims against the final diff
and observed behavior, inspecting unclear code instead of inventing a plausible
story. Then set the diff and session history aside and read the title and
opening alone. Can you explain what happens differently and why it matters
without repeating unexplained labels from the text? Check each following
paragraph the same way. Familiarity with the implementation can hide missing
explanations. Rewrite when scope changes; omit the session's chronology.

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

### Decision-making framing for compiler completion

Every agent decision serves one global destination
([rust_compiler_completion.md](wiki/drafts/reference/rust_compiler_completion.md)):
**recording all six release gates (repository, source semantics, build and
packages, native matrix, diagnostics, and representative programs) with
`tools/release/release_record.py` at one commit across all four hosted targets
(`linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`).**

Optimize every engineering and validation choice for **compiler completion**,
never for making a single turn or gate check appear green:

1. **Never settle for 1-off wins, stop after one passing test, or bloat `samples/cli/facts`:**
   Making a single fixture pass and stopping does not complete the compiler, and
   minting new 36-file `samples/cli/facts/<fact>/` folders bloats the corpus and
   slows every release gate. `samples/cli/facts/` is **frozen against new folder
   creation** (`tools/check_rules.py` rejects new folders there). Whenever you
   repair a mechanism:
   - Implement the general semantic, proof, and lowering rule across all value
     placements (`local`, `field`, `parameter`, `call_result`, `match_join`,
     `const_name`, nested `machine`, other `machine`, and `Build`-selected
     target implementation) in `omega-rust/`.
   - Sweep and re-record the **entire root-cause family** of pinned regressions in
     `tests/omega/corpus_outcomes.txt` (`--list-pinned-regressions`) and failing
     programs in `samples/` (`tools/sample_native_gate.py`) that share that
     mechanism, closing the whole cluster in one pass rather than one test at a
     time.
   - Pin any newly uncovered behavior with a focused positive and negative
     fixture pair under `tests/omega/{pass,fail,run}/<behavior>/` (never a new
     18-place fact matrix).
   - Immediately continue to the next open requirement of the `TASKS.md`
     capability or the next release-gate blocker in the same session. Do not stop
     after landing one witness or one PR unless the coordinator or user
     explicitly bounds the slice.
2. **Fail-closed semantics over positive-only shortcuts:** When an analysis
   (`Psi 05` place rebasing, borrow origins, write frames, or call-result
   candidates) cannot resolve an exact backing place or proof obligation, return
   `None` / `Err` (fail closed). Never add a fallback branch that treats an
   unresolved local or reference as its own backing storage or returns
   `Ok(Vec::new())` to unblock a positive test, and always scope taint or
   rejection checks to the exact projected path (`path.is_empty()` or
   prefix/suffix overlap) so sibling fields are not poisoned.
3. **Never `--record` a broken or dirty row:** Before opening a PR, inspect
   `git diff -- tests/omega/corpus_outcomes.txt`:
   - Every `pass/` and `run/` row in the diff must be `checked` or `built`
     (never record a `pass/` place as `rejected` because one place such as
     `target_body` had un-migrated syntax, a leftover `omega.lock`, or a missing
     `reaches` clause — fix the place first).
   - Every `fail/` row in the diff must be `rejected ... expected` (never leave
     `rejected ... unexpected` or `checked`; if a compiler repair legitimately
     sharpens a diagnostic on an existing `fail/` control, update its
     `expected.txt` to the true obligation first).
   - Always commit all code and fixture edits **before** running
     `python3 tools/corpus_gate.py --record --filter ...` so recorded `@<sha>`
     stamps never carry a dirty `+wt` suffix.
4. **Evolve framing when recurring failures appear:** Whenever
   [coordinator-review](.agents/skills/coordinator-review/SKILL.md) or
   [retrospect](.agents/skills/retrospect/SKILL.md) catches agents repeatedly
   making the same wrong tradeoff along the compiler-completion path, update the
   common framing here in `AGENTS.md` (and the execution steps in
   [advance](.agents/skills/advance/SKILL.md) or creation-time checks in
   `tools/check_rules.py`) while keeping platform swarm skills thin.

## The sample loop

Compiler advancement is driven by the **six release gates** in
[rust_compiler_completion.md](wiki/drafts/reference/rust_compiler_completion.md):
repository health, source semantics (`python3 tools/corpus_gate.py`), build and
packages, the 4-host native execution matrix (`python3 tools/corpus_gate.py --native`
on `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`), diagnostics
(`python3 tools/corpus_gate.py --filter fail/`), and representative programs
across all of `samples/` (`python3 tools/sample_native_gate.py`).

### No new `samples/cli/facts` folder bloat

`samples/cli/facts/` already holds 414 nine-place folders and
`tests/omega/{pass,fail}/facts/` holds over 11,500 fixtures. **Creating new
`samples/cli/facts/<folder>/` directories or new 18-fixture
`tests/omega/{pass,fail}/facts/<fact>_through_<place>` matrices is frozen** and
rejected by `tools/check_rules.py`. Existing fact folders remain as regression
coverage (and any incomplete existing place or broken row inside them may be
repaired in place), but new compiler work must never mint 36-file fact matrices.

Instead, every compiler repair must:
1. **Generalize the mechanism in `omega-rust/` across all value placements:**
   Operate on canonical semantic representations so the same construct, proof
   fact, loan, cleanup action, or ABI placement works regardless of whether the
   value arrives through a local, field, parameter, call result, `match` join,
   `const` name, nested `machine`, other `machine`, or `Build`-selected target
   implementation, from `Psi 01–05` checking through `Psi 06–08` Terminal Psi
   and `Omega 00–09` native codegen (`omega run --both`).
2. **Sweep the entire root-cause family:** Identify and re-record every pinned
   regression in `tests/omega/corpus_outcomes.txt` (`python3 tools/corpus_gate.py
   --list-pinned-regressions`) and every failing program in `samples/`
   (`python3 tools/sample_native_gate.py`) that shares the repaired mechanism.
3. **Pin new coverage with focused behavioral fixtures:** When a repair covers a
   gap not already pinned by an existing corpus regression, add a focused
   positive fixture and reject control under
   `tests/omega/{pass,fail,run}/<behavior>/` (2 fixtures, not 36).

### Priority and continuous execution

1. **Choose the outcome and its root-cause family:** Honor explicit user scope
   and retain the current capability assignment first; sessions with an active
   assignment never pivot to unrelated `main` regressions. For new automatic
   work without a current assignment, repair an unclaimed active `main`
   regression (where the newest completed `Post-merge smoke` run on `main`
   failed and no live claim covers it) before choosing an actionable outcome in
   this order:
   - First, take the first unassigned actionable item in `TASKS.md`'s
     `## Priority (human-set only)` section (in top-down order; only humans add,
     reorder, or move items into or out of that section).
   - Within and immediately after that section, prioritize ratified
     language-design and surface-syntax migrations (`NESTED-MACHINE-UNIFICATION`,
     `CANONICALIZE-SCALAR-RANGE-CONTRACTS`, `STATIC-MACHINE-MIGRATION`,
     `BUILD-OWNED-TARGET-SELECTION`, `BUILD-OWNED-NATIVE-BINDINGS`, and
     `TOP-LEVEL-BOUNDARY-REQUIREMENTS`) ordered by `.omg` blast radius.
   - Next, take remaining `TASKS.md` outcomes in top-down board order and
     cluster-sized families of pinned regressions in
     `tests/omega/corpus_outcomes.txt` (`--list-pinned-regressions`) and failing
     programs in `samples/` (`tools/sample_native_gate.py`).
   Reuse a warm `target/` producer with verified inputs; build only when those
   inputs changed or no valid producer exists.
2. **Make the full family run and land:** Done for a slice means the compiler
   repair is general across value placements and stages (`omega run --both`),
   the entire root-cause family in `tests/omega/corpus_outcomes.txt` and
   `samples/` is repaired and cleanly recorded (zero `+wt` stamps, zero
   `pass/` rows recorded as `rejected`, zero `fail/` rows recorded as
   `unexpected`), and any language change that rejects previously accepted
   syntax migrates every affected sample and fixture in the same commit.
3. **Update `TASKS.md` monotonically and continue immediately:** Delete each
   completed bullet or clause from `TASKS.md` outright so the board shrinks
   monotonically (never rewrite completed work into `"preserving / retaining /
   keeping / now passes"` prose), and delete the item when its full
   `Acceptance:` passes. Landing one slice never ends the session: immediately
   continue to the next open requirement of the capability or next release-gate
   blocker on the warm worktree until the assigned capability or wave is closed.

### Requirements and evidence

Every compiler repair and its fixtures must satisfy the accepted specification
requirements end to end:

- **Establishment:** construction, guards, entry contracts, completed calls or
  checked theorem evidence on the construct under test justify introducing the
  fact; an unproved goal does not establish itself, and a post-hoc caller
  equality guard (`transition x == 100`) after the construct completes does not
  substitute for the construct establishing or preserving the static fact
  ([invocation contracts](wiki/spec/language/state_contracts.md),
  [proof contracts](wiki/spec/proofs/contracts.md),
  [domains](wiki/spec/language/domains.md),
  [dependent values](wiki/spec/language/dependent_values.md)).
- **Identity and lifetime:** the fact names an exact value or storage version;
  snapshots retain their meaning, and overlapping writes invalidate facts about
  changed storage without discarding facts about provably unrelated storage
  ([fact validity](wiki/spec/terminal-psi/fact_validity.md),
  [lifetimes](wiki/spec/language/lifetimes.md),
  [ownership](wiki/spec/language/ownership.md),
  [content custody](wiki/spec/resources/content_custody.md)).
- **Composition:** calls substitute exact actuals, joins retain only justified
  facts on every incoming path, and state transitions prove their arrival
  obligations; the consuming operation (such as `total - 100u64`) is proved
  directly from the fact carried through the place, and `<place>/main.omg` vs.
  `<place>/invalid/main.omg` differ only in the construct's own input, domain,
  or contract—never in a caller guard
  ([machines](wiki/spec/language/machines.md),
  [evaluation](wiki/spec/language/evaluation.md),
  [patterns](wiki/spec/language/patterns.md),
  [structural predicates](wiki/spec/terminal-psi/structural_predicates.md)).
- **Soundness boundaries:** proof preserves ownership, access authority and
  termination obligations, tracks transitive assumptions, and respects the
  boundary between erased mathematics and executable values
  ([mathematical bindings](wiki/spec/proofs/mathematical_bindings.md),
  [mathematical values](wiki/spec/terminal-psi/mathematical_values.md),
  [quotients](wiki/spec/proofs/quotients.md),
  [inductive profile](wiki/spec/proofs/inductive_profile.md),
  [termination](wiki/spec/language/termination.md),
  [authority](wiki/spec/resources/authority.md)).
- **Checked-to-Terminal Psi lowering (`Psi 06–08`):** source constructs lower
  without loss into canonical target-neutral Terminal Psi (`PSITERM\0`) and
  execute in the Terminal Psi interpreter (`omega run --both`), erasing
  mathematical/proof-only bindings while preserving erased proof-formal tag lanes
  (`proof-term-tags`), materializing explicit affine cleanup actions
  (`DiscardRoot`, `DiscardResidual`, `InvokeNominal`, `RetireHome`) and
  borrowed-storage restoration windows (`reborrow restored call uses`), lowering
  state transitions, crash routes, and strongly connected ranking/scalar-block-invariant
  edges, separating explicit `dyn` selector vs. data lanes, and retaining
  boundary-application demands, access-policy/atomic observations, integer arithmetic
  modes/certificates, and checked named implementation candidates
  ([product](wiki/spec/terminal-psi/product.md),
  [encoding](wiki/spec/terminal-psi/encoding.md),
  [calls and outcomes](wiki/spec/terminal-psi/calls_and_outcomes.md),
  [control flow](wiki/spec/terminal-psi/control_flow.md),
  [dynamic dispatch](wiki/spec/terminal-psi/dynamic_dispatch.md),
  [ownership](wiki/spec/terminal-psi/ownership.md),
  [loans](wiki/spec/terminal-psi/loans.md),
  [boundary calls](wiki/spec/terminal-psi/boundary_calls.md),
  [observations](wiki/spec/terminal-psi/observations.md),
  [integer certificates](wiki/spec/terminal-psi/integer_certificates.md)).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):**
  target realization joins Build companion rows (`ProgramEntry`, `ProviderPlan`,
  `CompilerBuiltinExecution`, `BoundaryExecutionBinding`, assumed binding
  contracts, and target-keyed provider selections) to the Terminal catalog without
  reopening source, evaluates and validates target layout plans (`size_of`/`align_of`,
  field/bit offsets, variant tags/payloads, `IntegerAt` stored widths,
  element/byte view descriptors, explicit recasts/codecs, and erased zero-byte
  carriers), and lowers calls, returns, foreign bindings, private callbacks, and
  boundary entries into validated `CallPlan`/`StatePlan`/`BoundaryEntryPlan`
  placements across register vs. stack lanes, indirect `sret` aggregate return
  homes, and `&write` write-only destination pointers
  ([provider selection](wiki/spec/build/provider_selection.md),
  [assumed binding contracts](wiki/spec/build/assumed_binding_contracts.md),
  [foreign bindings](wiki/spec/build/foreign_bindings.md),
  [private callbacks](wiki/spec/build/private_callbacks.md),
  [layout plans](wiki/spec/layouts/plans.md),
  [recasts](wiki/spec/layouts/recasts.md),
  [codecs](wiki/spec/layouts/codecs.md),
  [calling plans](wiki/spec/build/calling_plans.md),
  [byte views](wiki/spec/terminal-psi/byte_views.md)).
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):**
  backend lowering selects target ISA instructions (`x86_64` / `arm64`,
  exact/wrapping/saturating integer ops, narrow-to-wide sign/zero extension,
  shift masks, signed/unsigned division and remainder, IEEE float comparisons,
  and checked `asm`), assigns register homes and disjoint stack-frame activation
  storage with live-across-call spill slots, composes worst-case stack usage
  (WCSU) and bounded activation claims (`committed <= bound`), and inserts entry
  prologues and multi-return callee-saved restoration epilogues honoring the
  `StatePlan` ceiling ([storage](wiki/spec/resources/storage.md),
  [activation storage](wiki/spec/resources/activation_storage.md),
  [machine-state evidence](wiki/spec/build/machine_state_evidence.md),
  [checked assembly](wiki/spec/language/assembly.md)).
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):**
  final encoding resolves symbolic stack-frame displacements, lays out
  function-relative basic blocks with short/near branch relaxation and offset
  reflow after prologue/epilogue insertion, binds read-only byte-pool, `dyn`
  dispatch-table, import, and private-callback relocations (`physical children`),
  validates final instruction bytes and transitive machine-state footprint
  evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`), and emits
  direct PE/ELF/Mach-O images or admitted executable placements that execute and
  agree with Terminal Psi (`omega run --both`) across the four hosted targets
  ([machine-state evidence](wiki/spec/build/machine_state_evidence.md),
  [executable installation](wiki/spec/build/executable_installation.md),
  [calling plans](wiki/spec/build/calling_plans.md)).
- **Runtime falsifiability (`Psi 06–08` and `Omega 00–09`):** just as
  `<place>/invalid/main.omg` is the static falsifier that proves `Psi 01–05`
  rejects when the static obligation is removed, `main()` in `<place>/main.omg`
  must be a runtime falsifier that traps (`crash Trap`) if `Psi 06–08` or
  `Omega 00–09` mis-lowers the construct at runtime:
  - **Branch and pattern discrimination:** every arm of a `match`, `transition`,
    or conditional guard under test must yield a distinct runtime outcome, and
    `main()` must invoke the construct with inputs exercising both the matching
    path and the fallback path. Never test only a boundary input where the
    matched branch and the `_` fallback evaluate to the same number (for example,
    testing only `value == 73` when `_ -> 73` so `selected - 73 == 0` passes on
    either branch).
  - **Observable cleanup and restoration:** when the fact's sentence names affine
    cleanup (`DiscardRoot`, `DiscardResidual`, `InvokeNominal`, `RetireHome`) or
    borrowed-window restoration, the discarded or retired carrier must produce an
    observable runtime side effect (such as a `drop` / `dropped by` hook
    updating caller-visible state or a restored borrow window read back after the
    call) that `main()` verifies ran for the discarded value and did not run for
    the surviving projection. Mutating only the dying temporary or dropping a
    plain scalar record is a runtime no-op and does not prove cleanup lowering.
  - **Storage and ABI neighbor canaries:** when the fact's sentence names an
    indexed store, field projection, recast, or multi-slot ABI placement,
    surround the target slot with non-zero canary fields and assert both the
    updated target value and the untouched neighbors in `main()`.

These are requirement categories, not a new test matrix or a claim that they
exhaust the accepted language. Keep the concrete remaining requirements and
acceptance on the existing task board. In the fact folder's README, name the
owning task and exact requirement, link the positive witnesses and fact-sensitive
reject twins, and list how each applicable requirement above is established and
what remains open.
Keep run records in the existing qualification/claim record or commit, with
producer revision and target. Raw run data (CSV, JSON, logs, per-fixture
output, archives) stays out of git: commit a summary, and link raw data by a
pinned commit or keep it in the claim note. `tools/check_rules.py` rejects any
added file under `wiki/drafts/measurements/` that is not a `.md` file of about
300 lines or fewer. Distinguish fresh results, applicable retained
evidence and unverified cases. A missing or unrelated reject diagnostic is not
evidence that the fact is enforced. Subtract only the acceptance demonstrated.
A filled row does not close its parent requirement or the check/proof layer
unless all of that acceptance is accounted for.

A compiler-repair assignment needs a demonstrated current-baseline defect:
a valid positive wrongly refuses or executes with the wrong outcome, or a
well-formed invalid control wrongly accepts. Freeze the source/oracle and record
that result before implementation.
An already passing witness may establish coverage; do not manufacture a failure
or count it as a necessary repair. Preserve formation obligations and the whole
published contract: a condition cannot justify forming its own partial operand,
and a necessary consequence is not an equivalent replacement without its converse.

Implementers and reviewers follow [execution evidence and replay inputs](tools/testing.md#execution-evidence-and-replay-inputs)
and [main-change validation](tools/testing.md#main-change-validation). Runtime
claims require actual engine observations; evidence reuse needs a consumer-based
applicability argument. Required independent review follows the
[exact-candidate review boundary](tools/testing.md#independent-semantic-review).
The implementation owner resolves findings and finishes authorized landing.

## Commands

The Rust toolchain is pinned in `rust-toolchain.toml`; `rustup` selects it.

Compiler execution requires optimization. Ordinary dev/test builds default to
`opt-level = 1`; the compiler build rejects effective `opt-level = 0`, including
Rust flag overrides. Sample/corpus gates verify the supplied binary's build
profile before execution, including `--cli`/`--runner`. Pinned compiler-backed
qualification declares `producer.executables` and captures their verified
profiles bound to the actual binary digests. See
[optimized validation compilers](tools/testing.md#optimized-validation-compilers).

- Use `mbx` in place of Cargo for compile and test commands: `cargo check`
  becomes `mbx check`, `cargo test` becomes `mbx nextest run` (needs
  cargo-nextest 0.9.140 or newer). If `mbx` is unavailable, use Cargo without
  asking. Nextest does not run doctests; use `mbx test --doc`. `mbx` manages a
  shared cross-workspace compilation cache (under `AppData/Local/mbx` on Windows or
  `~/.cache/mbx` on Unix), hardlinking pre-compiled objects across worktrees.
  Update it via `cargo install mbx`. Running `mbx setup` configures plain Cargo
  commands to leverage mbx caching automatically.
- Keep each worktree's Cargo target directory independent. Share mbx's cache,
  not `CARGO_TARGET_DIR` between checkouts: Cargo can treat another checkout's
  artifacts as fresh. In particular, baseline comparisons must not write into
  the candidate's target directory.
- Wrap a lane's local build/test chain in one `tools/build_slot.py`
  hold; release it before waiting on GitHub. Nested holds short-circuit
  ([build slots](tools/build_slot.md#one-hold-for-a-whole-chain)).
- Keep using `cargo fmt` and `cargo clean` directly — `mbx clean` differs.
  Whole-workspace formatting is `python tools/fmt.py`: `cargo fmt --all`
  exceeds the Windows command-line limit at this workspace's size.
  `cargo fmt -p <package>` formats the whole package, so drift in files you did
  not edit lands in your diff: check `git status` afterwards and restore the
  extras, or check only your files with `rustfmt --edition 2024 --check <file>...`.
- Compile a program: `mbx run -p omega -- --check <root.omg>`; run it:
  `mbx run -p omega -- run <root.omg>`. The CLI package is `omega` (binary
  `omega`) — there is no `omega-cli` package. The full flag surface:
  [omega-rust/omega/application/cli/README.md](omega-rust/omega/application/cli/README.md#command-surface).
- The compiler's end-to-end test is the corpus gate:
  `python tools/corpus_gate.py --filter <group/name fragment>`; a crate's own
  Rust tests run by package and name filter
  (`mbx nextest run -p <crate> <filter>`).
- **Validate with witnesses, never with a sweep.** A check answers one
  question: did this change move the behavior it touches? The answer comes
  from the few programs that can move: the change's own fact places and their
  reject twins, the fixtures named for the touched behavior, and the touched
  crates' tests. That is tens of fixtures and minutes of execution, not
  thousands of fixtures and hours. More fixtures do not add confidence about
  this change; they add drift, timeouts and false alarms to chase.
  - Count before running. Write the fixture count, the expected build time and
    the expected execution time in the claim note. Past about 200 fixtures or
    15 minutes, the selection is a sweep: narrow it before starting.
  - A domain is not a focus. `--filter facts/` selects nearly 14,000 fixtures
    and `--filter fail/` about 11,000. Filter by fact or fixture name:
    `--filter facts/saved_observation_snapshot` selects 105.
  - Unfiltered and whole-domain runs, the [full baseline](tools/testing.md#full-baseline),
    and any other whole-corpus or whole-workspace sweep are the user's to start.
  - A base compare over 40 fixtures runs on hosted CI:
    `corpus_gate.py --compare <base> --remote --filter ...`. Hosted runs are
    free; this machine's build slots are not.
  - A comparison must reach the code it compares. Before trusting base and
    candidate agreeing, confirm a fixture got past parsing, admission and
    package review into the stage the change touched. Two builds failing the
    same way earlier prove nothing.
  - Stored outcomes are history. A row that disagrees with today's run is
    attributed with `--compare <base>` on that fixture, not chased.
  - A fixture the change edits, adds or deletes is the change's own witness,
    however many there are: re-record its row in
    `tests/omega/corpus_outcomes.txt`, or delete the row when the change does
    not run it. `tools/check_rules.py` fails admission on a row left
    describing source the change replaced. #958 rewrote 738 variant-payload
    fixtures without re-recording them, and their rows went on saying
    `checked` for programs that no longer checked.
  - If a single fixture takes minutes to check or a build takes most of an
    hour, that is a workflow defect to measure and report, not a reason to
    wait longer.
- Validation policy — what a change must run, baseline gates, corpus and
  bootstrap gates, test selection, slow builds — lives in
  [tools/testing.md](tools/testing.md). Read it before running more than the
  affected crate's check.
- Before each build or test, name the decision, smallest meaningful check,
  applicable reusable evidence, and expected time and resources. If it
  overruns, inspect progress and record a reasoned decision rather than blindly
  waiting or restarting. Reviewers need not repeat expensive checks without a
  material evidence gap; required CI and release gates still apply.
- Reuse valid completed evidence when its inputs and affected consumers still
  apply; retain its revision, command and outcome. A broader run needs a concrete
  change-specific obligation that focused affected-path tests and relevant
  positive/rejection controls cannot establish. This does not authorize the
  user-triggered sweeps above or replace required GitHub `compiler-check`.
- **Keep progress PRs bounded.** Name the changed behavior and its necessary
  checks before building. Reuse unchanged results; finish and enqueue once
  those checks and required review pass. Full Product builds, self-hosting,
  release matrices and pinned qualification are separate assignments unless
  explicitly required for the current acceptance. Do not accumulate every
  earlier repair's tests or turn a discovered performance problem into another
  prerequisite for an independently complete PR. Follow the
  [merge validation budget](tools/testing.md#merge-validation-budget).

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
flow: Psi checks every supplied ordinary implementation in every compilation,
and Build records exact provider selections per target. Target-prefixed machine
declarations and implicit same-path target-body families are retired; remove
their remaining machinery under `BUILD-OWNED-TARGET-SELECTION` rather than
adding compatibility paths. `--target` narrows what Omega realizes, never what
Psi checks. The full
architecture contract — deferred Psi audit policy, crate placement,
discoverability, compositional lowering — lives in
[omega-rust/pipeline.md](omega-rust/pipeline.md).

Psi groups compilation machinery under `compilation/`, execution engines under
`interpreter/`, and shared primitives under `foundation/`. Compile-time admission
and result elaboration belong to `compilation/build-time-evaluation/`, not an interpreter.
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

- Avoid unnecessary secondary labels and descriptions. Let the layout do the explaining.
- Data and code are independent: named implementations satisfy traits; data
  does not. Trait data types are ordinary explicit parameters. There is no
  conformance subject, `Self` type binding, or machine `self` receiver.
  Predicate-only `self` names the subject in data default-domain `requires` and
  domain `requires`; it is not a receiver or runtime parameter. `dyn`
  selects an implementation at runtime; data arguments stay separate. Authored
  implementations declare member machines inline; reuse is an ordinary call,
  not a `Trait::member = Existing::machine;` reference row. Follow
  [named implementations](wiki/spec/language/conformances.md#data-and-code-are-independent)
  and remove legacy support rather than preserving it for old tests.
- Inline cases follow all common fields as a consecutive tail. Destructure data
  by named fields plus `case as binding`, or positionally by authored common-field
  order then one case-only view. Case patterns bind payload fields only; common
  fields in case patterns are retired. Preserve normal nested-match, subject,
  borrow, ownership and cleanup rules; source order is not physical layout.
  Irrefutable `let` bindings use the same named/positional patterns and evaluate
  their initializer once; they add no hidden branch, trap, copying permission or
  special match-as header. Keep the ordinary match forms valid.
  Follow [data decomposition](wiki/spec/language/patterns.md#data-and-case-decomposition);
  `DATA-DECOMPOSITION` owns compiler/source/tooling migration.
- Source `where` is retired. Use `requires` for data/default-domain and generic
  predicates and static callable requirements; assembly carries `requires`,
  `ensures` and `clobbers` directly after `asm`. Keep nested callable contracts
  and their terminators scoped, and preserve each declaration's checking boundary.
  Follow [requirement clauses](wiki/spec/language/state_contracts.md#requirement-clauses);
  `REQUIRES-UNIFICATION` owns compiler/source/tooling migration.
- Source `via` binding clauses and one-to-one bodyless native forwarding
  declarations are retired. The owning package records
  `Build::bind<Requirement>(builder, target, locator)` intent; literal locator
  byte lengths are inferred, not manually counted in helpers or Build generics.
  The compiler derives plans and preserves exact contracts, ABI, provenance,
  admission, and authority. Product rows never install Build-host backing;
  application roots do not repeat dependency-private imports. Follow
  [native bindings](wiki/spec/build/foreign_bindings.md#build-owned-native-binding);
  `BUILD-OWNED-NATIVE-BINDINGS` owns compiler/source/review migration.
- Core numeric carrier/property APIs live in ordinary `integer` and `float`
  modules, not primitive-member `MAX`/`Maximum` aliases or a catch-all limits
  module. Implement them through checked library bodies, exact type refinement
  and authoritative semantic metadata, never spelling-based proof rules or
  per-API intrinsics. Float extrema are finite; exact bit/classification APIs
  preserve their promised sign/payload/kind without strengthening arithmetic
  NaN guarantees. Payload constructors are conveniences, not permanent compiler
  mechanisms. Follow [numeric APIs](wiki/spec/language/numeric_values.md#core-integer-and-float-apis),
  without admitting arbitrary-width integer families or general owned aggregate
  casts.
- Source `state` declarations are retired in favor of arbitrarily nested
  machines. Nesting scopes names, never runtime data or implicit authority.
  All runtime arguments are explicit; ordinary calls resume, terminal calls
  forward completion. Resolve lexical scope before flat downstream identities.
  Do not preserve state-only contract/lowering paths or turn constant-stack
  cycles into growing recursion. Follow
  [nested machines](wiki/spec/language/machines.md#nested-machines-and-identity);
  `NESTED-MACHINE-UNIFICATION` tracks implementation and source migration.
- Real words in code (`character`, `expression`, `arguments` — not `ch`,
  `expr`, `args`). Names must read outside compiler insiders: `pipeline`
  over `driver`.
- Arena-backed data with generational handles over scattered heap objects;
  ZII — the zero handle is the absence state, so do not wrap handles in
  `Optional`; no `RefCell` as an ownership escape. Durable lowered children
  use `Handle<T>`/`HandleSpan<T>`; `Vec` is construction scratch, not the
  durable shape. Arena/vector-backed symbol tables over local hash maps; a
  map keyed by compiler-owned objects is a review trigger for missing
  representation structure. Dense IDs (`semantic_id!` types, arena handles)
  index flat tables (`IdTable`/`IdSet` once they land; until then a `Vec`
  indexed by the ID), and derived data is computed once by its owner, not
  cached; `tools/check_rules.py` fails a file whose count of ID-keyed maps or
  cache-shaped bindings rises. Symbols are handle-first — string names are
  debug/export payload, not identity. Source text stays in loading,
  diagnostic, and debug payload roles.
- Entrypoints stay prominent: `main.rs` owns startup and visible dispatch
  to real subsystems. Split at responsibility boundaries.
- Comments explain non-obvious intent. A `lib.rs` header first says what the
  module does for its caller, then explains the reasoning signatures cannot
  carry. Follow [Explain the change](#explain-the-change).
- Commit subjects are `lane: statement` — pick the lane by changed
  responsibility per [commit naming](#commit-naming) below.
- `TASKS.md` supplies capability priorities and remaining acceptance; the
  sample gate supplies executable witnesses, not the priority order. Keep
  capability outcomes, external prerequisites, release acceptance and unresolved
  regressions on the board, not one item per sample or diagnostic.
  `TASKS_BOOTSTRAP.md` and `TASKS_OPTIMIZER.md` keep the bootstrap chain and
  optimizer work, which have their own acceptance. An item exists only while
  it names unfinished work and is deleted when its acceptance passes; state
  its current frontier once, replacing the paragraph rather than appending
  history. Owner decisions go to `OWNER_QUESTIONS.md`. Nobody owns the boards;
  reconcile concurrent edits at rebase. Implementation status belongs on the
  board, not in `omega-rust/pipeline.md` or `wiki/spec`: those are contracts,
  and `tools/check_rules.py` fails admission when a contract file's status
  phrasing grows. It also fails when this file, a skill, `pipeline.md`, the
  specification or a fact README cites a task ID no board holds, so the change
  that deletes or merges an item repoints or deletes its citations.
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
- Tests check behavior or structure, never source text. Do not read source
  files with `include_str!` or similar and assert that names or fragments
  are present or absent: such a test breaks on a rename, passes when the
  forbidden call goes through another spelling, and says nothing about what
  the program does. An ownership or dependency rule belongs in
  `tools/check_native_boundary.py` (cargo metadata) or in visibility and
  types, so a violation stops compiling. An order or count rule belongs in a
  test that runs the code and observes it. In the week to 2026-11-01, 261
  such reads were added; PR #1139 replaced the ones it could and deleted
  165 spelling-only asserts, and three PRs added 14 more within an hour of
  this rule. `tools/check_rules.py` now fails admission when a file's reads
  exceed its count in `tools/source_text_reads.txt`.
- Read the matching skill before starting: `.agents/skills/` — `advance` for
  the sample loop, `rust-systems-programmer` for Rust implementation and review,
  `architecture-cleanup`, `task-board-cleanup`, `local-swarm`, `devin-swarm`,
  `codex-swarm`, `cursor-swarm`, `claude-swarm`.

Deliberate configuration — do not "fix" it: `clippy.toml` thresholds are
raised and dev/test profiles use `debug = 0` and `opt-level = 1`;
`proof-admission` and `sha2` use `opt-level = 3`. Check `Cargo.toml` and session
overrides before interpreting compiler timings. Debug information can be
restored with `CARGO_PROFILE_DEV_DEBUG=2` or `CARGO_PROFILE_TEST_DEBUG=2`
while retaining optimization. An unoptimized compiler is a build error, not a
supported validation producer. Runtime environment settings do not change the
profile embedded in an existing binary.
`.gitattributes` forces LF because canonical source and evidence identities
are byte-sensitive.

## Scope and escalation

- The session that starts work finishes it. A claim records what a session
  is doing so other sessions can see it; it grants nothing and blocks nothing.
  Parked work is available to any session once its prerequisite is met.
- You still own your open PRs after a session ends or changes. At startup
  and resumption, check the PRs you opened, including earlier sessions, against
  retained worktree and claim records. Keep each PR's URL, submitted head,
  remaining acceptance or failure, and next action in its existing claim note.
  Resume the unfinished work and carry authorized publication through verified
  merge. Close a PR only when its work is deliberately withdrawn or superseded,
  with the disposition recorded; age alone does not end ownership.
- A sample's work includes the complete application and the compiler repairs
  it needs to run; carry it through source, native/Terminal execution, invalid
  controls and publication. Several failing samples with one demonstrated root
  cause are one piece of work; shared files or pipeline stages are not a
  reason to group. Never divide a sample into stages for different agents. Only an actual external prerequisite (an owner decision, an
  unavailable environment) gates work.
- Keep a short record in the claim note: parent task, remaining acceptance,
  current witness and the requirement it discharges, current failure, next
  necessary action and prerequisites. Write it in words a person can read
  without the diff: what of the customer now runs, and what of it still fails.
  Explicitly named standalone work need not
  invent a parent task. Reread the note after context compaction
  and before changing direction. A checkpoint is not completion. A user stop
  or exhausted budget preserves the frontier without claiming success.
- A newly discovered obstacle belongs to the work that found it: fix it
  rather than filing a task for someone else. Do not split work or mine
  tasks to keep agents busy; narrow parallel slices build duplicate routes for
  the same operation. When a slice lands, immediately take the next open
  requirement of the parent `TASKS.md` capability or the next release-gate
  regression family in `tests/omega/corpus_outcomes.txt` and `samples/`.
- Overlap is expected and needs no permission. A Git conflict is a design
  signal: two sessions are changing one decision point, so reconcile them into
  one implementation at rebase. Conflicts only show textual overlap; semantic
  breakage can still land without one; post-merge checks detect those regressions.
- Keep regressions visible without dogpiling or thrashing `main`. Required merge
  admission is the small GitHub `compiler-check`, not the entire corpus or
  customer matrix. The hosted `Post-merge smoke` workflow
  (`.github/workflows/compiler-regression.yml`) runs Linux native/Terminal smoke
  after merge and nightly. `pending`, `in_progress`, or `cancelled` runs are
  never regressions, and an older failed run is already resolved once a newer
  `main` commit succeeds. Only when the **newest completed** run on `main`
  failed is `main` red. Sessions working on an assigned capability or sample
  ignore unrelated `main` regressions and keep their warm base. The regressing
  PR's author owns repair/revert; otherwise at most **one** unassigned session
  claims the regression (`python tools/claims.py claim --item REGRESSION-<sha>`)
  and drives a focused repair or revert through the queue. A missing claim is
  not enough: before claiming, read open pull requests whose title, body or
  diff names the failing fixtures or touches the file the repair would edit,
  the commits on `main` since the newest red run, and any smoke run still in
  progress. A repair may already be queued, or landed and not yet verified.
  Repeat that check before pushing. When a candidate repair exists, verify it
  against the failure instead of writing a second one. Once a live claim
  exists for a regression, every other session skips it and takes normal
  unassigned work. Never `git bisect` or check out historical `main` commits in
  a warm worktree to attribute a regression (inspect the diff or CI log instead
  so the compiler build cache stays warm). No machine runs all three OSes, so
  each host with a native golden runs `python tools/native_watch.py` on a
  schedule ([testing](tools/testing.md#corpus-outcome-gate)), and the Windows
  x86-64 development host runs the full
  `python tools/sample_native_gate.py --programs-only` once a day from Task
  Scheduler ([testing](tools/testing.md#corpus-outcome-gate)). Agent sessions,
  local or cloud, never start a full programs-only run for this check. Record a
  regression in `TASKS.md` only when it cannot be repaired in that session, and
  say exactly what prevents the repair.
- After landing a sample, an agent may take a bounded
  iteration-performance or test-environment outcome based on an experienced
  obstruction. Record the workflow and adequate target before repair; return
  to the parent capability when it meets that target. No incidental profiling detours.
- Report which samples now run, which capability requirements their evidence
  removes, and what remains; do not count activity or commits as closure. Missing implementation
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

**The primary repository checkout is strictly an anchor pinned to `main`.**
Never edit files, switch branches, commit, build, or run gates in the primary checkout.
Other agents and background processes rely on the primary checkout remaining cleanly
on `main` to pull from or use as a shared reference. All active development,
repairs, experiments, and validations must take place in an isolated worktree under
`<repository>/.agents/worktree/<short-name>`.

**Local branches are banned.** Do not create local branches (`git branch <name>`,
`git checkout -b <name>`, or `git worktree add -b <name>`). Local branches drift,
accumulate clutter, and collide across sessions. Always create worktrees in
detached HEAD mode directly from the fetched `origin/main` commit:
`git worktree add --detach .agents/worktree/<short-name> origin/main`.
That directory is gitignored local state. An existing checkout stays where it is.
To publish a PR, push HEAD directly to origin:
`git push origin HEAD:refs/heads/work/<short-name>`.
On that first push, open the pull request as a draft and return its URL in the
same turn. Later pushes update that same draft. A request to "push" includes
this step; no separate confirmation is needed. When controlled by a coordinator
session, stop at the draft pull request (`candidate_ready`) so the coordinator
can run [coordinator-review](.agents/skills/coordinator-review/SKILL.md) before
landing; standalone sessions with no coordinator run
`python tools/landing.py land <number> --expected-head <sha>` after
self-reviewing against the same checklist. A draft cannot enter the queue, and it is not a place to leave the work.

**Retain and reuse warm worktrees as workbenches.** Teardown is never mandatory.
Reusing an existing worktree preserves its pre-compiled `target/` directory and
incremental build state, saving significant build time on subsequent tasks.
To reuse a worktree for a new task, simply reset its detached HEAD to the latest
`origin/main` commit:
`git checkout --detach origin/main` (and `git clean -fd` to remove untracked scratch).
Only remove worktrees (`git worktree remove .agents/worktree/<short-name>`) when
explicitly requested or when reclaiming disk space.

In a Claude Code cloud session, `.agents/worktree/warm` arrives checked out at
a recent main commit with that commit's compiler build already restored
([tools/warm_build.py](tools/warm_build.py)). Run
`python3 tools/warm_build.py wait`, then use it as the workbench and record its
commit as the fixed base. Do not move it to a newer main first: main's frequent
changes to the lowest crates make that rebuild as slow as a cold build
(12 minutes).

Before launching a new swarm or creating a new worker, successfully fetch remote
`main` (`git fetch origin main`) and record its exact commit. Do not modify, pull into,
or fast-forward the primary checkout; preserve it intact as a stable reference.
Create new worktrees directly from the fetched `origin/main` commit.
Read `AGENTS.md` and the matching skills from that updated source before selecting or
dispatching work. A failed fetch stops the new launch; a cached `origin/main` is never
a fallback. Each independent cloud executor repeats this preflight before implementation.
Existing outcomes retain their worktrees, unpublished work, and recorded fixed base,
including after resumption or compaction.

**Never chase main with gates or rebuilds.** Main or queue SHA movement alone never
triggers pull, rebase, build, requalification, gate re-runs, or review. A draft
pull request is already submitted for `compiler-check`. Standalone sessions land
it with `tools/landing.py land` in the same session after self-review; workers
under a coordinator report `candidate_ready` so the coordinator reviews and
lands it with `tools/landing.py land`; do not leave a draft across unrelated landings, and do
not rebase it merely because `main` moved. Once it is ready, GitHub's native
merge queue and Actions CI perform the integration build and admission check
against current `main`. Authors only perform focused validation on their own
change on their recorded worktree base. Only a demonstrated actual merge conflict,
a concrete prerequisite or changed affected source justifies targeted local integration
and affected checks. Postlanding verification reads the merged PR, published commit and
CI results; it does not rebuild or rerun acceptance. Offline plans and dry runs are previews,
not a completed launch preflight.

- Give a session or subagent a complete piece of work, never a stage: the
  parent capability and remaining acceptance, its next sample and oracle,
  and that it lands its own result and continues the capability within the
  authorized scope and stopping condition.
  State directions directly; never imply or insinuate them. The session
  carries authorized work through implementation, testing, review, normal
  main landing and remote verification, and keeps working until it is
  complete.
- **Never schedule timers or polling check-ins during coordination.** After
  launching background tasks or subagents, stop calling tools and let
  reactive wakeups (subagent messages and background task completion events)
  resume execution. Periodic timers (`schedule`, cron, `send_later`, or sleep
  loops) burn coordinator context and tokens without advancing work.
- **Mandatory coordinator diff review before landing ([coordinator-review](.agents/skills/coordinator-review/SKILL.md)).**
  When a coordinator session controls threads, subagents, sub-sessions, or
  cloud sessions, workers push their validated branch to a draft pull request
  (`gh pr create --draft`) and report `candidate_ready` to the coordinator;
  workers under a coordinator never mark a PR ready (`gh pr ready`) or run
  `tools/landing.py land` / `gh pr merge` themselves. Only the coordinator marks
  a PR ready and lands it after auditing the full `git diff` against the
  5-point checklist in [coordinator-review](.agents/skills/coordinator-review/SKILL.md)
  (fail-closed soundness and sibling-field isolation, general semantic mechanism
  across all stages, `corpus_outcomes.txt` diff with zero `pass/` rows recorded
  as `rejected` and zero `fail/` rows recorded as `unexpected`, unweakened
  9-place witnesses and reject twins, and algorithmic/ratchet discipline).
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
- A requested Claude Code cloud wave follows
  [claude-swarm](.agents/skills/claude-swarm/SKILL.md). Each worker is a
  Claude Code cloud session with its own container and branch, started at a
  pinned base. Select its model through `create_session`'s `model` field.
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
- When selection policy changes, coordinators explicitly send active workers
  the updated instructions and parent-outcome context at their next safe
  checkpoint. Do not assume a file edit refreshed a running session's context;
  preserve its current witness, unpublished work and original user limits.
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
| `compiler` | Product compilation coordination and reports in `omega-rust/omega/compilation/compiler/`, the source-free native driver in `omega-rust/omega/compilation/native-production/`, or an inseparable compiler contract change spanning Psi and Omega. A coordinator call-site adjustment accompanying a stage fix keeps the stage's lane. |
| `build` | The build system in `omega-rust/omega/build-system/`: project and package flows in `orchestration/` and Build activation in `build-settings/`, plus product build declarations such as `source/omega/build.omg`. Provider planning and trust admission under `realization/` use `omega`; CLI policy-file handling uses `cli`. Repository build commands and CI use `repo`. |
| `packages` | Package acquisition, graphs, review, admission, installation, and update workflows in `omega-rust/omega/packages/`. Package command wiring accompanying those changes keeps `packages`. |
| `cli` | Command parsing, flags, help, and command dispatch in `omega-rust/omega/application/cli/src/`, plus the corresponding product entrypoint. The command's underlying compiler or package behavior uses its owning lane. |
| `tooling` | Shipped developer tools and shared host support in `omega-rust/omega/foundation/`, including artifact file writing and measurements. Semantic trust records and admission under `realization/trust-model/` use `omega`. |
| `library` | Bundled packages in `source/library/`, including `core`, `alloc`, `std`, and ordinary domain packages. A compiler semantic fix exercised by library code keeps its compiler lane. |
| `samples` | Standalone sample/example content and presentation in `samples/` or example directories. A regression fixture for an implementation fix keeps that implementation's lane. |
| `tests` | Shared test infrastructure, corpus registration, fixtures, or architecture gates spanning multiple owners. Tests for one responsibility use that responsibility's lane, even in a test-only commit. |
| `tools` | Repository maintenance/development utilities in `tools/`, including merge-queue clients and sample-refresh scripts. Bootstrap tools follow the rung/chain rules above; shipped tooling uses `tooling`. |
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
change-type prefix such as `feat:` or `fix:`. Fact-row and fixture commits
follow the same rule: they take the lane of the behavior they pin, or
`samples` when they add only sample content. `facts(...)`, `tests(facts):`,
`docs(workflow):` and a bare `Pin ...` or `Complete ...` subject are not lanes;
496 of 1,736 commits from Oct 15 to Oct 21 had no lane, and 178 of 1,017 from
Oct 25 to Nov 1. A squash merge takes the pull request title as its subject,
so `compiler-check` checks the title against this table.

The body follows [Explain the change](#explain-the-change), after a blank line.
Include design constraints or rejected alternatives only when they help explain
the final change. Describe validation by what it establishes, with measured
results where relevant. Omit the body only when the subject carries the full
reasoning (typo, rename, mechanical revert).

Commit messages carry no tool attribution: no "Generated with ..." line, no
`Co-Authored-By` trailer naming an agent or vendor.

## Hard rules

- **No work in primary checkout, no local branches, worktrees only.**
  Never edit, switch branches, commit, build, or test in the primary repository checkout.
  The primary checkout stays pinned to `main` as a pristine reference anchor. Other agents
  and background processes rely on it remaining on `main` to pull into their worktrees.
  All development, testing, and tool runs must take place in an isolated worktree under
  `<repository>/.agents/worktree/<short-name>`.
  Local branches are banned: do not create local branches (`git branch <name>`,
  `git checkout -b <name>`, `git worktree add -b <name>`). Always create worktrees in
  detached HEAD mode directly from the fetched `origin/main` commit:
  `git worktree add --detach .agents/worktree/<short-name> origin/main`.
  Submit PRs by pushing HEAD directly to origin:
  `git push origin HEAD:refs/heads/work/<short-name>`.
  Worktrees may be retained and reused as warm build workbenches (`git checkout --detach origin/main`)
  to accelerate future builds, or removed (`git worktree remove <path>`) when explicitly requested
  or reclaiming disk.
- **Do not add required feature-combination special cases or new `samples/cli/facts` folders. Clean as you go.**
  Generalize the touched operation or representation across all value placements
  (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`,
  nested `machine`, other `machine`, and `Build`-selected target implementation)
  and remove its superseded path in the same change; do not land another
  source-shape recognizer, fallback, or mode with cleanup deferred to a task,
  and do not create new `samples/cli/facts/<folder>/` directories (which are
  frozen by `tools/check_rules.py` to prevent corpus bloat). Pin repairs by
  flipping existing pinned regressions in `tests/omega/corpus_outcomes.txt` and
  `samples/`, or with a focused positive/negative fixture pair under
  `tests/omega/{pass,fail,run}/<behavior>/`. See
  [compositional lowering](omega-rust/pipeline.md#compositional-lowering).
- **Verify at trust boundaries, not after every stage.** Foreign package review
  and untrusted build-script execution verify; a stage never re-validates,
  replays or rebuilds output the same compile just produced. Verifiers stay
  exercised by the corpus runner and tests, off the default compile path. See
  [verification placement](omega-rust/pipeline.md#verification-placement).
- **Every delivery push must have a draft until reviewed and landed.** A user request
  to "push" means push the committed work and open or update its pull request
  in the same turn. The first push opens a draft (`gh pr create --draft`).
  Later pushes update that same draft. Under a coordinator session, the worker
  stops at the draft PR and reports `candidate_ready` so the coordinator can run
  [coordinator-review](.agents/skills/coordinator-review/SKILL.md) before running
  `python tools/landing.py land <number> --expected-head <sha>`
  ([tools/landing.md](tools/landing.md)); standalone sessions run
  `tools/landing.py land` after self-reviewing against the same checklist. A draft cannot enter the queue. Reuse
  the existing pull request, verify its head matches the pushed commit, and
  return the PR URL. A draft is not a place to leave the work: the session
  still lands it and does not leave the draft across unrelated landings. Do
  not open a second pull request for the same branch. A pushed branch or
  retained worktree alone is not delivery. If PR creation fails, report the
  concrete blocker and leave publication unfinished; never report a branch-only
  push as completion.
- **Never push to `main` directly and never chase `main` with gates.** Push candidate commits
  directly from HEAD to a remote work branch ref (`git push origin HEAD:refs/heads/work/<name>`)
  and open that pull request as a draft until reviewed and landed via
  `tools/landing.py land` ([tools/landing.md](tools/landing.md)). The queue
  integrates the candidate with latest `main`, and GitHub Actions CI performs the build and checks.
  Do focused validation only on the author's own changes against the recorded worktree base;
  never chase main by rebasing, rebuilding, or re-running gates merely because main or the integration
  SHA advanced. `tools/landing.py` only observes/enqueues the native queue: no local lease,
  exact-SHA build receipt or direct-publication path remains. An open PR is not
  completion. Verify MERGED, its exact submitted head and published main commit,
  then retain the worktree as a warm workbench or remove clean task-owned worktrees
  if teardown is explicitly requested. Preserve run identities; CI admission is not full runtime qualification.
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
