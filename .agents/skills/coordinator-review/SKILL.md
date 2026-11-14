---
name: coordinator-review
description: Review an Omega compiler or sample change for soundness, witness fidelity and algorithmic cost before landing it. Use for author self-review and coordinator review; landing ownership follows AGENTS.md.
---

# Review a Change Before Landing

Read the actual diff before landing so a passing check does not hide a broken
language guarantee or a weakened test. Authors use this checklist on their own
changes; coordinators can use it when reviewing a change that needs attention.
Ownership follows [AGENTS.md](../../../AGENTS.md#sessions-and-subagents), and
publication follows [landing](../../../tools/docs/landing.md).

## 1. Who Lands

Workers are trusted with their task end to end. A worker runs this checklist
on its own diff, opens the PR ready, and squash-merges it when `admission`
passes. A coordinator may run the same checklist on any diff it has a concern
about; workers do not wait for it. Neither side polls or schedules timers;
completion messages wake the session.

Before requesting the squash merge, read the exact PR description that will
become its commit body against [Explain the change](../../../AGENTS.md#explain-the-change).
Check its row-count delta and public runtime names as well as its source and
validation claims. Correct the description before merging; updating a PR after
merge does not correct the published squash message. This is part of author
self-review, not an additional approval gate.

## 2. The 5-Point Coordinator Review Checklist

Before merging, inspect the actual diff:

```bash
gh pr diff NUMBER
# For a scoped local view, use the recorded base and submitted head:
git diff BASE HEAD -- omega-rust/ samples/ TASKS.md
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
- **Full stage carry-through (`Psi 05 -> 06 -> publication -> Native`):** Verify
  that a construct, carrier, or cleanup disposition admitted by the lowered-form
  checker is carried through Terminal Psi execution and native codegen
  (`omega run --both`), rather than stopping at `--check`.
- **Bounce a sound patch in the wrong place:** Correct is not sufficient. Return
  a PR that adds a carrier, case or per-place branch where a shared operation
  could serve, even if every check passes; the general fix belongs in the same PR.
- **Known affected programs shown:** Focused results name the tests and
  programs they covered. The PR's `Gate:` line must describe the change's real
  purpose: something this review can check.

### Point 3: Outcomes and Diagnostic Expectations
- **No corpus, no ledger:** `tests/omega/` is deleted and nothing may be
  added under that path. The `Proof:` line names the tests on the change and on the
  small area around it; reject a PR that widened its run instead of
  investigating a failure there.
- **Read actual focused results:** Positive programs must pass their intended
  checks and runtime oracles. Negative controls must reject for the obligation
  they break; a parse, import or reachability error does not establish that.
- **Inspect changed expectations:** A new `expected.txt` must describe the
  intended refusal. Never weaken a program or its diagnostic just to make a
  failed check green. Keep source and host limits beside the reported result.

### Point 4: Witness, Reject-Twin & Anti-Bloat Audit (`samples/` & kept-fixture diff)
- **No expansion of the frozen fact matrix:**
  No new `.omg` file may be added under `samples/cli/facts/`, including inside
  an existing folder (`cargo xtask check-rules` enforces this). Repairs target
  existing failing programs in `samples/` or
  add a test on the changed code with, at most, a focused positive/negative
  pair kept beside it under `tests/fixtures/`.
- **No weakened positive witnesses:** Confirm the worker did not make a fixture
  or sample pass by adding a caller post-hoc equality guard (`transition x == 100`),
  changing the bound or operation, or simplifying the construct under test.
  Positive executable witnesses must bind `ProgramEntry` for all four hosted
  targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and
  pass `omega run --both` with their authored exit.
- **Minimal reject-twin diff:** Compare positive vs. negative controls
  (`pass/...` vs. `fail/...` under the kept fixtures). They must differ **only**
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
- **Frozen-checker deletion boundary:** The retired checked-tree stages
  (`typed-trees-to-checked-trees`, `checked-trees-to-lowered-psi`) and other crates
  named in [AGENTS.md](../../../AGENTS.md#law-of-completion) take only deletions
  or moves of still-needed code into surviving owners. Reject repairs or
  fallback routes through them; verify the lowered-form checker retains the
  required obligations and no production boundary check was bypassed.
- **`TASKS.md` closure:** Delete a bullet only when its full stated acceptance
  is demonstrated. For a fact row that is its existing nine places and reject
  twins; other outcomes follow their own acceptance. This is the test for a
  pull request that deletes the bullet. A progress pull request toward it is
  judged on the repair it makes and the checks that establish that repair, not
  on the whole matrix. Never rewrite completed
  work into `"preserving / keeping / now passes"` history prose.

## 3. Review Verdict Actions

- **If the diff passes all 5 points:** squash-merge it, then take the next
  requirement of the same capability on the warm worktree.
- **If it fails a point:** fix it on the same branch and review again. A
  coordinator who finds the failure tells the worker the exact file, line and
  point.

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
   mistake can be detected deterministically (such as a new file under the
   retired `tests/omega/`), enforce it in `tools/xtask/src/rules.rs` as well as
   in `AGENTS.md`.
