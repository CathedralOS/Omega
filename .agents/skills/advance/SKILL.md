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
  executes Steps 2–5 in its own context window and stops at a draft PR for
  `coordinator-review`.

## Step 1: Preflight, Worktree & Claim

1. **Fetch & create an isolated detached worktree:** Never edit, switch
   branches, or build in the primary checkout:
   ```bash
   git fetch origin main
   git worktree add --detach .agents/worktree/<short-name> origin/main
   ```
   Local named branches are banned. Record the fixed base SHA (`git rev-parse HEAD`).
2. **Check live work before selecting or coding:**
   - Run `python3 tools/claims.py status` and `gh pr list --state open`.
   - Select the capability and root-cause family using the priority order in
     [AGENTS.md](../../../AGENTS.md#the-sample-loop) (`TASKS.md` capability and
     migration items, pinned regression clusters in
     `tests/omega/corpus_outcomes.txt` via `--list-pinned-regressions`, and
     failing representative programs in `samples/` via `sample_native_gate.py`).
   - Do **not** create new `samples/cli/facts/<fact>/` folders or 18-fixture
     `tests/omega/{pass,fail}/facts/` matrices (they are frozen by
     `tools/check_rules.py` to prevent corpus bloat).
3. **Register or update the claim:**
   ```bash
   python3 tools/claims.py claim --board TASKS.md --item <ID> --owner "<label>" --path <paths>
   ```
   Keep the claim note concise: parent task, remaining acceptance, current
   witness, observed failure, and next action. At initial dispatch, `--path`
   can name the witness/board paths or subsystem directory from the recorded
   diagnostic; workers refine paths if needed after tracing.

## Step 2: Reproduce & Trace Before Editing (Worker-Owned Under Swarm)

1. **Build or reuse the warm optimized producer** in the worktree's `target/`
   (`CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=<dir> mbx build -p omega`;
   close stdin with `< /dev/null` on manual background runs).
2. **Witness the failure first:** Run the target witness (`omega run --both` or
   `python3 tools/sample_native_gate.py --facts --filter <fact>`). If a proof
   rejection is unclear, use `python3 tools/proof_advisor.py --omega <binary> <root.omg>`
   as an advisory probe.
3. **Cut a refusal down with small probes:** Isolate the failing construct,
   decide from the governing spec in `wiki/spec/` whether the refusal is a
   language rule or a compiler precision/lowering gap, and check
   `git log <base>..origin/main -- <files>` so you do not duplicate an in-flight
   repair.

## Step 3: Implement Across the Full Path & Challenge Assumptions

1. **Generalize the mechanism end to end:** Apply
   [compositional lowering](../../../omega-rust/pipeline.md#compositional-lowering)
   and the [decision-making framing](../../../AGENTS.md#decision-making-framing-for-compiler-completion)
   (fail-closed semantics, exact projected-path scoping, full `Psi 05 -> 06 -> 08 -> Native`
   carry-through).
2. **Write inline design reasoning:** At the module entry or decision site,
   document why the mechanism fits and what invariant holds so the next reader
   does not have to reconstruct it.
3. **Challenge assumptions:** Construct a realistic falsifier (e.g., sibling
   fields where one is valid and one is invalid, or matching vs. fallback
   runtime paths), verify the mechanism works across value placements (`local`,
   `field`, `parameter`, `call_result`, `match_join`, `const_name`, nested
   `machine`, other `machine`, and `Build`-selected target implementation) and
   `omega run --both` (exit 0), and pin any newly uncovered behavior with a
   focused positive/negative fixture pair under
   `tests/omega/{pass,fail,run}/<behavior>/` (never a new 36-file fact folder).

## Step 4: Validate, Sweep the Root-Cause Family & Reconcile `TASKS.md`

1. **Run focused validation & sweep the full root-cause family ([testing.md](../../../tools/testing.md)):**
   - Do not stop at a 1-off fixture win: identify all pinned regressions in
     `tests/omega/corpus_outcomes.txt` (`--list-pinned-regressions`) and failing
     programs in `samples/` (`tools/sample_native_gate.py`) that were blocked by
     the same mechanism or migration gap, and validate the whole family together.
   - Run affected crate unit tests and `python3 tools/corpus_gate.py --filter <fact_and_family_fixtures>`.
   - Attribute any unexpected fixture movement against the fixed base commit with
     `python3 tools/corpus_gate.py --compare <base> --filter <moved_fixtures>`.
2. **Commit code and fixture files BEFORE `--record`:**
   - Ensure no leftover `omega.lock` or `build/` directories sit inside
     `tests/omega/` fixtures (never delete `tests/omega/pass/build/`).
   - Commit all `.rs`, `.omg`, and `expected.txt` changes first so `git status`
     is clean, then run:
     ```bash
     python3 tools/corpus_gate.py --record --filter <affected_fixtures>
     ```
   - Inspect `git diff -- tests/omega/corpus_outcomes.txt` and verify every
     `pass/` or `run/` row is `checked`/`built`, every `fail/` row is
     `rejected ... expected`, and no row carries a `+wt` stamp or
     `regressed-from:` marker.
3. **Update `TASKS.md` monotonically:** Delete the completed bullet or clause
   from `TASKS.md` outright (never rewrite into `"preserving / keeping / now
   passes"` history prose), and run `python3 tools/check_rules.py --base origin/main`.

## Step 5: Publish Draft PR & Continue or Land

1. **Push HEAD to a remote `work/` ref and open a draft PR:**
   ```bash
   git push origin HEAD:refs/heads/work/<name>
   gh pr create --draft --title "<title>" --body "<body following Explain the change>"
   ```
2. **Follow the landing boundary ([AGENTS.md](../../../AGENTS.md#sessions-and-subagents)):**
   - **When controlled by a coordinator session:** Stop at the draft PR and
     message the coordinator (`candidate_ready`). The coordinator runs
     [coordinator-review](../coordinator-review/SKILL.md) before marking ready
     and running `tools/landing.py land`.
   - **When running standalone with no coordinator:** Self-audit the full diff
     against [coordinator-review](../coordinator-review/SKILL.md), then run
     `python3 tools/landing.py land <pr> --expected-head $(git rev-parse HEAD)`.
3. **Chain the next requirement on the warm worktree:** Once landed, continue the
   parent capability's next remaining requirement on the same warm worktree
   (`git fetch origin main && git checkout --detach origin/main && git clean -fd`).
