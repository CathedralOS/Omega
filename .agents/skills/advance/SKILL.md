---
name: advance
description: >-
  Execute the step-by-step workflow to advance an Omega capability outcome
  through executable samples, validation, and landing. Use for advance,
  continue the compiler, or pick the next compiler work. Common rules and
  decision framing live in AGENTS.md.
---

# Advance Workflow

This skill contains **only** the step-by-step execution workflow for advancing a
compiler capability. Common repository knowledge, the [law of completion](../../../AGENTS.md#law-of-completion),
the nine-place sample loop, requirements and evidence, and decision-making
framing for compiler completion live in [AGENTS.md](../../../AGENTS.md)
(`#decision-making-framing-for-compiler-completion`, `#the-sample-loop`,
`#requirements-and-evidence`, `#sessions-and-subagents`).

- **Standalone session:** One agent executes Steps 1–5 end to end.
- **Coordinator + subagents:** The coordinator runs Step 1 from `TASKS.md` and
  `tests/omega/corpus_outcomes.txt` (without reading `omega-rust/` source files
  to pre-diagnose the bug) and dispatches each worker immediately; the worker
  executes Steps 2–5 in its own context window and lands its own PR.

## Step 1: Preflight, Worktree & Claim

1. **Acquire an isolated `worker-N` pool slot:** Never edit, switch branches, or
   build in the primary checkout. Claim an idle `.agents/worktree/worker-N` slot
   with `cargo xtask worktree acquire <lane>-<topic>` so the slot path stays
   fixed and its warm `target/` cache is preserved across tasks:
   ```bash
   cargo xtask worktree acquire <lane>-<topic>
   ```
   Local named branches are banned. Record the fixed base SHA (`git rev-parse HEAD`).
2. **Check live work before selecting or coding:**
   - Run `cargo xtask claims status` and `gh pr list --state open`.
   - Select the capability and root-cause family using the priority order in
     [AGENTS.md](../../../AGENTS.md#the-sample-loop) (`TASKS.md` capability and
     migration items, pinned regression clusters in
     `tests/omega/corpus_outcomes.txt` via `--list-pinned-regressions`, and
     failing representative programs in `samples/` via `cargo xtask sample-gate`).
   - Do **not** create new `samples/cli/facts/<fact>/` folders or 18-fixture
     `tests/omega/{pass,fail}/facts/` matrices (they are frozen by
     `cargo xtask check-rules` to prevent corpus bloat).
3. **Register or update the claim:**
   ```bash
   cargo xtask claims claim --board TASKS.md --item <ID> --owner "<label>" --path <paths>
   ```
   Keep the claim note concise: parent task, remaining acceptance, current
   witness, observed failure, and next action. Start it with the cluster line:
   `Gate: <release-gate> | mechanism: <what fails> | rows sharing it: <N> of <total> | fix location: shared, or one-off because <reason>`.
   Refine the mechanism after tracing; the count keeps the task from shrinking
   to its witness. At initial dispatch, `--path`
   can name the witness/board paths or subsystem directory from the recorded
   diagnostic; workers refine paths if needed after tracing.

## Step 2: Reproduce & Trace Before Editing (Worker-Owned Under Swarm)

1. **Build or reuse the warm optimized producer** in the worktree's `target/`
   (`CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=<dir> mbx build -p omega`;
   close stdin with `< /dev/null` on manual background runs).
2. **Witness the failure first:** Run the target witness (`omega run --both` or
   `cargo xtask sample-gate --facts --filter <fact>`).
3. **Read the spec, then cut the refusal down:** Read the governing page in
   `wiki/spec/` first. Isolate the failing construct, decide from that spec whether the refusal is a
   language rule or a compiler precision/lowering gap, and check
   `git log <base>..origin/main -- <files>` so you do not duplicate an in-flight
   repair.
4. **Budget verification:** Most agent time goes to waiting on builds and gates.
   Finish reading and reasoning first, then build once per hypothesis. Do not
   rebuild to learn what the source already says.

## Step 3: Implement Across the Full Path & Challenge Assumptions

Before editing, apply rule 6 of the [decision-making framing](../../../AGENTS.md#decision-making-framing-for-compiler-completion):
if the fix wants another carrier, case or per-place branch for a mechanism
that already has one, widen that mechanism and make the general fix in this PR.

1. **Generalize the mechanism end to end:** Apply
   [compositional lowering](../../../omega-rust/pipeline.md#compositional-lowering)
   and the [decision-making framing](../../../AGENTS.md#decision-making-framing-for-compiler-completion)
   (fail-closed semantics, exact projected-path scoping, full lowered-form checker
   `-> 08 -> Native` carry-through).
2. **Write inline design reasoning:** At the module entry or decision site,
   document why the mechanism fits and what invariant holds so the next reader
   does not have to reconstruct it.
3. **Challenge assumptions:** Construct a realistic falsifier for the placement
   you changed (e.g., sibling fields where one is valid and one is invalid, or
   matching vs. fallback runtime paths) and confirm the mechanism holds and the
   invalid one is still rejected. Pin any newly uncovered behavior with a
   focused positive/negative fixture pair under
   `tests/omega/{pass,fail,run}/<behavior>/` (never a new 36-file fact folder).
   The other placements and targets are the capability's acceptance on
   `TASKS.md`; reach for one only when a result here points at it.

## Step 4: Validate, Sweep the Root-Cause Family & Reconcile `TASKS.md`

Done means the compiler still holds the same object. The pinned diagnostic, and
the finite `tests/omega` facts fixture, is a photograph of that object.
Changing the fact, the bound, or the operation so a place passes is repainting
that photograph: a different object, and a green check on it is not done.

1. **Prove what you built, then the ring around it ([Law of completion](../../../AGENTS.md#law-of-completion)):**
   - Prove the change itself: the unit tests of the crates you touched.
   - Run `cargo xtask corpus --filter <fixtures>` naming only the fixtures
     this change targets. You already know which they are. The
     gate adds the ring around them (each one's twin in the other tiers and
     its neighbours by name) and prints every record as it finishes.
   - A clean blast ends pre-merge testing. Land.
   - A blast that is not clean is an investigation of those rows: run the
     command the gate prints, the same fixtures at the base commit, to learn
     whether your change moved them. Never widen the run.
   - A full run is a checkpoint (`--sweep`, in the background, on request or
     nightly), not a step toward landing one change.
   - Put the result in the PR body: `Proof: <tests on the change> | blast: <n>
     fixtures | <clean, or what the rows that moved turned out to be>`.
2. **Leave the ledger alone:** Do not run `corpus --record` or edit
   `tests/omega/corpus_outcomes.txt`; the post-merge check writes it. Ensure no
   leftover `omega.lock` or `build/` directories sit inside `tests/omega/`
   fixtures (never delete `tests/omega/pass/build/`).
3. **Update `TASKS.md`:** Delete the completed bullet or clause
   from `TASKS.md` outright (never rewrite into `"preserving / keeping / now
   passes"` history prose), and run `cargo xtask check-rules --base origin/main`.

## Step 5: Publish and Land

1. **Push HEAD to a remote `work/` ref and open the PR ready:**
   ```bash
   # In a compiler lane, PR_BODY also carries the `Gate:` line from AGENTS.md.
   cargo xtask provenance  # append this model-audit line to the commit and PR_BODY
   mbx clippy -p <crate> --all-targets -- -D warnings  # each Rust crate you touched
   git push origin HEAD:refs/heads/work/<lane>-<topic>
   cargo xtask check-rules --base origin/main --title "<title>" --body-file PR_BODY
   gh pr create --base main --head work/<lane>-<topic> --title "<title>" --body-file PR_BODY
   ```
2. **Land it:** Review your own diff the way a careful reviewer would
   ([coordinator-review](../coordinator-review/SKILL.md)), then squash-merge it
   when `admission` passes. You already have permission; waiting for it only
   slows the project down.
3. **Chain the next requirement on the warm worktree:** Once landed, continue the
   parent capability's next remaining requirement on the same warm worktree.
   Inspect `git status --short` and preserve unfinished or untracked work before
   fetching main and switching the detached checkout to the next fixed base.
   Retain the warm build cache; do not clean the worktree blindly.
