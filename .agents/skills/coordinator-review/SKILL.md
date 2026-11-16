---
name: coordinator-review
description: Before merging an Omega compiler or sample change, check that it is sound, that its test programs still test what they claim, and that it adds no needless algorithmic cost. Use for author self-review and coordinator review; who merges follows AGENTS.md.
---

# Review a Change Before Landing

Read the actual diff before merging, so a passing check does not hide a broken
language guarantee or a weakened test. Authors run this checklist on their own
changes; a coordinator can run it on any change it is worried about. Who owns
the work follows [AGENTS.md](../../../AGENTS.md#sessions-and-subagents), and
how to publish follows [landing](../../../tools/docs/landing.md).

## 1. Who Lands

A worker runs this checklist on its own diff, opens the PR ready, and
squash-merges it when `admission` passes. A coordinator may run it on any
diff; workers do not wait for it. Neither side polls or sets timers; the
session wakes when a completion message arrives.

Before you squash-merge, check the exact PR description, which becomes the
commit body, against [Explain the change](../../../AGENTS.md#explain-the-change),
including its public runtime names and its source and validation claims. Fix
it before merging; editing the PR afterwards does not change the commit
message on `main`. This is part of self-review, not an extra approval step.

## 2. The 5-Point Coordinator Review Checklist

Before merging, inspect the actual diff:

```bash
gh pr diff NUMBER
# For a scoped local view, use the recorded base and submitted head:
git diff BASE HEAD -- omega-rust/ samples/ TASKS.md
```

If any of the five checks below fails, reject the change and tell the worker
the file and line:

### Point 1: Soundness & Fail-Closed Audit (`omega-rust/` diff)
- **Refuse when unsure:** When an analysis cannot work out a place, an alias,
  a borrow's origin, a call frame, or what must be proved, it must return
  `None` / `Err` so the program is refused. Reject a fallback at the end of a
  helper that accepts instead so a positive test passes:
  - *Bad example:* When a lookup of what a reference points to returns
    `None`, treating the reference as its own storage. That hides aliasing.
  - *Bad example:* When a call's write effects cannot be found, returning
    `Ok(Vec::new())`, which claims the call writes nothing. Return an error
    naming what is missing, and prove "writes nothing" explicitly when true.
- **Check the exact field, not its siblings:** A new check that refuses or
  marks a value must look at the exact field path (`path.is_empty()` or
  prefix/suffix overlap), not the whole local or struct:
  - *Bad example:* Checking `operand.retention` for an erasing cast without
    requiring `path.is_empty()`, which taints `pair.good` whenever a sibling
    field `pair.erased` holds an erasing cast.
  - Always ask: *"If a struct has two fields `good` and `bad`, does this check
    on `bad` accidentally reject or admit `good`?"*

### Point 2: General Mechanism, Not Recognizing One Program's Shape (`omega-rust/` diff)
- **No special cases on syntax-tree shape:** The fix must work on the
  lowered-form checker's tables in
  `omega-rust/psi/compilation/representations/terminal-psi/check-tables/`,
  which name each place by `ExactPlaceId`, not match one test program's syntax
  (e.g. handling only a bare `ExpressionNode::Name` when the operand could
  also be a field access, cast or call result).
- **Carried to the native program:** Whatever the lowered-form checker now
  accepts must also run in the Psi interpreter and natively
  (`omega run --both`), not just under `--check`.
- **Send back a correct fix in the wrong place:** Correct is not enough.
  Return a PR that adds a second data structure, an extra case or a
  per-location branch where a shared operation could serve, even if every
  check passes; the general fix belongs in the same PR.
- **Affected programs named:** Results name the tests and programs they
  covered. The `Gate:` line must state what the change really makes work, in
  terms this review can check.

### Point 3: Outcomes and Diagnostic Expectations
- **No test corpus, no results ledger:** `tests/omega/` was deleted and
  nothing may be added under it. The `Proof:` line names the tests on the
  change and the small area around it; reject a PR that widened its run
  instead of investigating a failure there.
- **Read the actual results:** Positive programs must pass their intended
  checks and produce their expected run-time result. Negative programs must be
  refused for the requirement they break; a parse, import or unreachable-code
  error does not show that.
- **Inspect changed expectations:** A new `expected.txt` must describe the
  intended refusal. Never weaken a program or its expected error to make a
  failing check pass. Keep any limit on a result (source, host) next to it.

### Point 4: Test Programs, Negative Twins & No New `.omg` Bloat (`samples/` & kept-fixture diff)
- **`samples/cli/facts/` is frozen:** No new `.omg` file may be added under
  it, even inside an existing folder (`cargo xtask check-rules` enforces this).
  Repairs target existing failing programs in `samples/` or add a test on the
  changed code with, at most, one positive/negative pair beside it under
  `tests/fixtures/`.
- **No weakened positive programs:** The worker must not make a fixture or
  sample pass by adding an after-the-fact check in the caller
  (`transition x == 100`), changing the bound or operation, or simplifying the
  construct under test. Positive programs that run must bind `ProgramEntry` for
  all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and
  pass `omega run --both` with their authored exit.
- **Twins differ in one thing:** Each positive program and its negative twin
  (`pass/...` vs. `fail/...` under the kept fixtures) must differ **only** in
  the fact, value range or contract the construct breaks, never in a caller
  check or unrelated syntax.
- **No deprecated syntax in new or migrated `.omg` files:** Reject any diff
  introducing `state` (instead of nested `machine`), `T [lo..hi]` scalar range
  suffixes, or un-targeted `Build::select_provider`.

### Point 5: Algorithmic Complexity, Ratchet & Board Audit
- **No quadratic scans or multiplying branches:** Inspect loops in the
  lowered-form checker and pipeline stages for accidental $O(N^2)$ work
  (`Vec::iter().find(...)` inside a loop over statements or states instead of
  a lookup by dense ID) or branches that multiply with every case.
- **Removed checked-tree stages stay removed:** The Rust compiler's old
  checked-tree stages (`typed-trees-to-checked-trees`,
  `checked-trees-to-lowered-psi`) were removed
  ([AGENTS.md](../../../AGENTS.md#law-of-completion)). Remaining code from them
  may only be deleted or moved to a stage that still needs it. Reject fixes in
  them or fallback paths through them; confirm the lowered-form checker still
  checks everything it must and no production boundary check was bypassed.
- **`TASKS.md` closure:** Delete a bullet only when its full stated acceptance
  is shown: its spec page's behavior, demonstrated by the running program and
  its negative twin ([sample loop](../../../AGENTS.md#the-sample-loop)). That
  is the test for a PR that deletes the bullet. A PR that only makes progress
  is judged on its fix and the tests showing that fix works, not on the whole
  task. Never rewrite completed work into `"preserving / keeping / now passes"`
  history prose.
- **Checkpoint continuity:** For a progress PR, identify the original program
  and expected result, its observed remaining failure and the continuing owner.
  A base failure does not end ownership. After merging, that owner reruns the
  original program on landed source and follows its next failure. If the session
  must stop, use the unfinished handoff in the law of completion.

## 3. Review Verdict Actions

- **If the diff passes all 5 points:** squash-merge it, then return to the
  original program in the same worktree. Follow its next failure until its
  original acceptance is complete before choosing another requirement.
- **If it fails a point:** fix it on the same branch and review again. A
  coordinator tells the worker the exact file, line and point.

## 4. Fixing the Instructions When the Same Mistake Recurs

The **same** mistake from several workers means the shared instructions are
missing a rule. When this review (or [retrospect](../retrospect/SKILL.md))
sees agents repeat a mistake on the way to finishing the compiler
([rust_compiler_completion.md](../../../wiki/drafts/reference/rust_compiler_completion.md)):
1. **Add the rule to [AGENTS.md](../../../AGENTS.md#decision-making-framing-for-compiler-completion):**
   every agent reads it there. Keep [advance](../advance/SKILL.md) to the
   step-by-step workflow only.
2. **Keep the swarm skill thin:** Never copy workflow patches into
   [`swarm`](../swarm/SKILL.md) or its platform setup references. Those files
   only describe how to start sessions on each platform and point to
   `/advance` and `coordinator-review`.
3. **Prefer an automatic admission check:** When a program can detect a
   recurring mistake reliably (such as a new file under the retired
   `tests/omega/`), enforce it in `tools/xtask/src/rules.rs` as well as in
   `AGENTS.md`.
