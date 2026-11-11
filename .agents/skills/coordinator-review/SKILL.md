---
name: coordinator-review
description: Audit worker, subagent, and cloud-session diffs before marking any pull request ready or landing it on main. Use whenever a coordinator session controls threads, subagents, sub-sessions, or cloud sessions, or when reviewing a compiler/sample PR for soundness, golden-file integrity, witness fidelity, and algorithmic complexity.
---

# Coordinator Review Gate

When one coordinator session controls threads, subagents, sub-sessions, or cloud
sessions ([`swarm`](../swarm/SKILL.md)), the coordinator is not a passive message router.
**The coordinator is the mandatory code-review and landing gate.**

Workers (especially fast/lightweight models or parallel sessions racing to close
an assignment) will occasionally take shortcuts that pass a narrow local check
or CI rule gate while breaking compiler soundness or golden-file integrity.
Never trust a worker's summary text (`"re-recorded 18 rows"`, `"all tests pass"`)
without reading the actual `git diff`.

## 1. The Draft-Only Worker Boundary

In any coordinated wave where a central session manages subagents, threads, or
cloud sessions:

1. **Workers own end-to-end implementation, 9-place execution validation, golden
   recording, and draft PR publication (`gh pr create --draft`), then stop and
   message the coordinator (`candidate_ready`).**
2. **Workers controlled by a coordinator never mark a PR ready (`gh pr ready`),
   never arm auto-merge (`gh pr merge --auto`), and never run
   `cargo xtask land` themselves.**
3. **Only the coordinator marks a PR ready and lands it** (`cargo xtask land <pr> --expected-head <sha>`) **after** completing the
   5-point diff audit below.
4. **Reactive wakeup only:** Neither the coordinator nor any worker schedules
   timers (`schedule`, cron, `send_later`, or polling loops). The worker's
   completion message wakes the coordinator directly to perform the review.

## 2. The 5-Point Coordinator Review Checklist

Before running `gh pr ready` or `cargo xtask land` on any worker branch or
PR, inspect the actual diff:

```bash
gh pr diff <pr> --exclude 'tests/omega/corpus_*.txt'
gh pr diff <pr> -- 'tests/omega/corpus_*.txt' 'TASKS.md'
```

Reject the candidate and send line-level feedback back to the worker if any of
the five checks below fail:

### Point 1: Soundness & Fail-Closed Audit (`omega-rust/` diff)
- **Fail-closed vs. fail-open fallbacks:** When an analysis cannot resolve a
  place, alias, borrow origin, call frame, or proof obligation, it must fail
  closed (`None` / `Err`). Reject any fallback that fails open at the bottom of
  a helper to unblock a positive test:
  - *Bad example:* Returning `Some(vec![place.clone()])` at the end of
    `storage_places_for_canonical_place` when `rebase_exact_local_place` returns
    `None`, which treats an un-rebased reference local as its own backing
    storage and masks aliasing.
  - *Bad example:* Returning `Ok(Vec::new())` in `stored_calls.rs` when
    `stored_reference_fallthrough_effects` returns `None`, instead of requiring
    `.ok_or("stored call has no complete statement-successor effects")?` and
    proving empty writable-slot relations explicitly.
- **Exact path and sibling-field isolation:** When adding a rejection or taint
  check on a carrier, verify it keys on the exact projected path (`path.is_empty()`
  or prefix/suffix overlap) rather than the whole root local or struct:
  - *Bad example:* Checking `operand.retention` for an erasing cast without
    requiring `path.is_empty()`, which taints `pair.good` whenever a sibling
    field `pair.erased` holds an erasing cast.
  - Always ask: *"If a struct has two fields `good` and `bad`, does this check
    on `bad` accidentally reject or admit `good`?"*

### Point 2: General Mechanism vs. Shape-Recognition Audit (`omega-rust/` diff)
- **No AST special-casing:** The repair must operate on canonical semantic
  representations (`StructuralTerm`, `CanonicalPlace`, `CheckedBorrow`,
  `CallFrameResolver`, `FactPayload`) rather than matching the surface AST shape
  of one fixture (e.g., handling only a bare `ExpressionNode::Name` when an
  operand may be a projection, cast, or call result).
- **Full stage carry-through (`Psi 05 -> 06 -> 08 -> Native`):** Verify that a
  construct, carrier, or cleanup disposition admitted in Stage 05 is carried
  through Stage 06 (`06_checked-trees-to-lowered-psi`), Terminal Psi
  execution, and native codegen (`omega run --both`), rather than stopping at
  `--check`.
- **Bounce a sound patch in the wrong place:** Correct is not sufficient. Return
  a PR that adds a carrier, case or per-place branch where a shared operation
  could serve, even if every check passes. Accept a cluster report in its place.
- **Family sweep shown, not claimed:** A claimed family sweep names the rows it
  covered. The PR's `Gate:` line must describe the change's real purpose; a
  `red rows: 0` PR must unlock something this review can check.

### Point 3: Golden File Diff Audit (`tests/omega/corpus_outcomes.txt` & `corpus_both_*.txt`)
Inspect every added or modified line in `tests/omega/corpus_outcomes.txt`:
- **Red rows belong only to an observation-only PR:** The two rules below hold
  for every PR that changes anything besides corpus ledgers and Markdown. A PR
  that changes only those may record regressions already on `main`
  ([who may record a regression](../../../tools/testing.md#who-may-record-a-regression));
  check that each such row was measured on a clean base checkout and attributed,
  and that a lost `pass/`/`run/` success carries `regressed-from:`.
- **Every `pass/` and `run/` row in the diff must be `checked` or `built`:**
  Never allow a worker to record a `pass/` or `run/` fixture as `rejected` just
  to satisfy `cargo xtask check-rules` (for example, recording `pass/facts/<fact>/target_body`
  as `rejected` because `target_body` still used deprecated `Build::select_provider`
  or missed a `reaches` clause).
- **Every `fail/` row in the diff must be `rejected ... expected`:**
  Never allow a `fail/` row to be recorded as `checked` (lost rejection) or
  `rejected ... unexpected` (rejected for the wrong reason, such as a parse,
  import, or reachability error before reaching the intended check).
- **Clean `@<sha>` commit stamps (no `+wt`):**
  Every recorded row must be stamped against a clean committed tree (`@<sha>`),
  never a dirty worktree (`@<sha>+wt`).
- **Zero `regressed-from:` markers** introduced outside an observation-only PR.

### Point 4: Witness, Reject-Twin & Anti-Bloat Audit (`samples/` & `tests/omega/` diff)
- **No new `samples/cli/facts/<folder>/` or 18-fixture fact-matrix bloat:**
  `samples/cli/facts/` is frozen against new folder creation (`cargo xtask check-rules`
  enforces this). Repairs must flip existing pinned regressions in
  `tests/omega/corpus_outcomes.txt` and `samples/` or add a focused
  positive/negative fixture pair under `tests/omega/{pass,fail,run}/<behavior>/`.
- **No weakened positive witnesses:** Confirm the worker did not make a fixture
  or sample pass by adding a caller post-hoc equality guard (`transition x == 100`),
  changing the bound or operation, or simplifying the construct under test.
  Positive executable witnesses must bind `ProgramEntry` for all four hosted
  targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and
  pass `omega run --both` with their authored exit.
- **Minimal reject-twin diff:** Compare positive vs. negative controls
  (`tests/omega/pass/...` vs. `tests/omega/fail/...`). They must differ **only**
  in the construct's own broken fact, domain, or contract—never in a caller
  guard or unrelated syntax.
- **No deprecated syntax in new or migrated `.omg` files:** Reject any diff
  introducing `state` (instead of nested `machine`), `T [lo..hi]` scalar range
  suffixes, or un-targeted `Build::select_provider`.

### Point 5: Algorithmic Complexity, Ratchet & Board Audit
- **No quadratic scans or branch explosions:** Inspect loops in hot compiler
  paths (`flow/context.rs`, `contract_entailment/`, `write_frames/`) for
  accidental $O(N^2)$ linear scans (`Vec::iter().find(...)` inside a statement
  or state walk instead of dense handle-indexed lookup) or uncollapsed branch
  multiplication.
- **Stage 05/06 ratchets:** Confirm no new shape gates or source-context reads
  were added in violation of stage ratchets, and no production boundary checks
  were bypassed.
- **Capability notices:** `cargo xtask check-rules` prints a notice when compiler Rust
  closes nothing that existed at the base (no row turned green, no control newly
  rejecting for its pin or changing its diagnostic, no `TASKS.md` line deleted).
  - `… pinned only on N row(s) it adds`: the PR body names the existing failing
    row, sample or acceptance it closes, or the behavior no existing fixture
    covered. A repair whose only customer is its own new fixture is per-case
    glue: return it to the worker.
  - `… changes no corpus outcome`: accept it only as performance or
    restructuring work with its measurement in the PR body. Any other repair
    with no observable outcome goes back to the worker.
- **`TASKS.md` closure:** Confirm a `TASKS.md` bullet is deleted only
  when all nine places and nine reject twins pass and are pinned under
  `tests/omega/{pass,fail}/facts/`, and never rewritten into `"preserving /
  keeping / now passes"` history prose.

## 3. Review Verdict Actions

- **If the candidate passes all 5 points:**
  1. The coordinator marks the PR ready and queues it for merge:
     `cargo xtask land <pr> --expected-head <sha>`
  2. The coordinator dispatches the next requirement of the parent capability
     to the worker on its warm worktree.
- **If the candidate fails any point:**
  1. Do **not** mark the PR ready or merge it (if auto-merge was armed, disable
     it immediately or convert back to draft).
  2. Send a concise message to the worker (`send_message` / `followup_task`)
     naming the exact file, line, and failed review point, and instruct the
     worker to fix it on the same branch and report back for re-review.

## 4. Workflow & Framing Evolution (Closing Recurring Failure Loops)

Catching a bad diff at review time protects `main`, but catching the **same**
class of mistake repeatedly across multiple workers means the decision-making
framing upstream is incomplete.

Whenever Coordinator Review (or [retrospect](../retrospect/SKILL.md)) observes a
recurring agent failure pattern along the path to compiler completion
([rust_compiler_completion.md](../../../wiki/drafts/reference/rust_compiler_completion.md)):
1. **Update common agent framing in [AGENTS.md](../../../AGENTS.md#decision-making-framing-for-compiler-completion):**
   Put shared rules and decision-making framing in `AGENTS.md` so every agent in
   the repository inherits them, and keep [advance](../advance/SKILL.md) focused
   strictly on the step-by-step execution workflow.
2. **Keep the swarm skill thin:** Never copy workflow patches into
   [`swarm`](../swarm/SKILL.md) or its platform setup references. Those files
   only describe platform session mechanics and point to `/advance` and
   `coordinator-review`.
3. **Prefer mechanical admission checks where possible:** When a recurring
   mistake can be detected deterministically (such as `+wt` stamps or `unexpected`
   rows in `corpus_outcomes.txt`), enforce it in `tools/xtask/src/rules.rs` as well as
   in `AGENTS.md`.
