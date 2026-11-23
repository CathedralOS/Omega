---
name: advance
disable-model-invocation: true
description: Use only when the user explicitly requests advance. Pick the next compiler capability, reproduce its failure, repair it, test it, and land the change.
---

# Advance Workflow

This skill lists **only** the steps. The shared rules, including the
[law of completion](../../../AGENTS.md#law-of-completion), live in
[AGENTS.md](../../../AGENTS.md)
(`#decision-making-framing-for-compiler-completion`, `#the-sample-loop`,
`#requirements-and-evidence`, `#sessions-and-subagents`).

- **Standalone session:** One agent runs Steps 1–5 itself.
- **Coordinator + subagents:** The coordinator runs Step 1 from `TASKS.md`,
  without reading `omega-rust/` source to diagnose the bug, and starts each
  worker immediately. Each worker runs Steps 2–5 in its own context window
  and merges its own PR.

## Step 1: Preflight, Worktree & Claim

1. **Acquire an isolated `worker-N` pool slot:** Never edit, switch branches, or
   build in the primary checkout. Claim an idle `.agents/worktree/worker-N` slot
   so its fixed path keeps the `target/` build cache from earlier tasks:
   ```bash
   cargo xtask worktree acquire <lane>-<topic>
   ```
   Do not create local named branches. Record the base commit (`git rev-parse HEAD`).
   `acquire` also starts this task's model-audit interval (what
   `cargo xtask provenance` counts). Before the next task on the same clean
   slot, run `mbx xtask worktree begin`; not for checkpoints or rebases. It
   changes no source, cache or pool lease and refuses a dirty or unassigned slot.
2. **Check live work before selecting or coding:**
   - Run `cargo xtask claims status` and `gh pr list --state open`.
   - Choose the capability and the shared cause of its failures by the priority order in
     [AGENTS.md](../../../AGENTS.md#the-sample-loop) (`TASKS.md` capability and
     migration items, and failing representative programs in `samples/` via
     `cargo xtask sample-gate`).
   - Do **not** create new `samples/cli/facts/<fact>/` folders or add anything
     under `tests/omega/` (`cargo xtask check-rules` rejects both).
3. **Register or update the claim:**
   ```bash
   cargo xtask claims claim --board TASKS.md --item <ID> --owner "<label>" --path <paths>
   ```
   Keep the claim note short: parent task, what it still needs, the program
   showing the failure (the witness), the failure seen, and the next action.
   Start it with this line:
   `Gate: <release-gate> | mechanism: <what fails> | programs sharing it: <which> | fix location: shared, or one-off because <reason>`.
   Correct the mechanism after tracing; naming the other programs keeps the
   task from shrinking to the witness alone. At first registration, `--path`
   can name the witness, the board, or the subsystem directory the recorded
   error points to. After tracing, name the files and functions you will edit
   in the note. For paths outside the registered set, take a supplemental claim
   and name the original ticket in its note, following
   [the claim scope rule](../../../AGENTS.md#hard-rules).
   A claim lasts two hours from when it was taken or last renewed. Run
   `cargo xtask claims renew` from the worktree after each push (it renews
   every claim that worktree took; `claims note` also renews). `renew` also
   restores a claim that lapsed and reports any live claim now overlapping its
   paths. Keep ownership through checkpoint merges;
   release all related claims when the original acceptance lands, or record an
   unfinished handoff under the law of completion when the session must stop.

## Step 2: Reproduce & Trace Before Editing (the Worker's Job in a Swarm)

1. **Choose the smallest check that exercises the failure.** Start with the
   owning layer's fixture. Include its consumer when the change spans a boundary.
2. **Reproduce the failure.** Run that check. For a program-level requirement,
   use the named program or `mbx xtask sample-gate --filter <sample_path>`.
   Reuse a suitable compiler when the check needs one. Build `omega` if the
   check needs an executable from changed compiler inputs.
3. **Read the spec, then shrink the failing program:** Read the `wiki/spec/`
   page for the feature first. Cut the program down to the refused construct
   and use the spec to decide whether the language forbids it or the compiler
   is not yet precise enough or cannot lower it. Check
   `git log <base>..origin/main -- <files>` so you do not repeat a merged fix.
4. **Build as little as you can:** Most agent time is spent waiting on builds
   and tests. Finish reading and reasoning first, then build once per idea.
   Do not rebuild to learn what the source already says.

## Step 3: Implement Across the Full Path & Challenge Assumptions

Before editing, apply rule 6 of the [decision-making framing](../../../AGENTS.md#decision-making-framing-for-compiler-completion):
widen the existing mechanism instead of adding a special case.

1. **Fix the shared cause.** Follow the affected contract through the stages
   that consume it. Select the fixture or program using the
   [repository testing rules](../../../AGENTS.md#decision-making-framing-for-compiler-completion).
2. **Write the reasoning into the code:** At the top of the module or where
   the decision is made, write why this approach fits and what must stay true.
3. **Try to break your fix:** Write a realistic program that would expose a
   wrong fix (e.g. two fields of one struct, one valid and one not, or the
   matching versus fallback path at run time). Confirm the valid case works
   and the invalid one is still refused. Pin newly found behavior with a test
   on the changed code and, when it needs a program, one positive/negative
   pair beside that test under `tests/fixtures/` (never a new folder under
   `samples/cli/facts/`). Do not test place by place
   ([sample loop](../../../AGENTS.md#the-sample-loop)); check another hosted
   target only when a result here points at it.

## Step 4: Validate & Reconcile `TASKS.md`

Never make a kept test program pass by changing its fact, bound or operation
([sample loop](../../../AGENTS.md#the-sample-loop)).

1. **Prove the thing you built, then a very small area around it ([Law of completion](../../../AGENTS.md#law-of-completion)):**
   - Prove the change itself: tests on the code you changed, in the crates you
     touched.
   - Then a very small area around it: the tests that exercise the same mechanism
     and the witness the `TASKS.md` item names. Check program output on each
     engine the assignment covers.
   - If that is fine, land.
   - If it is not, investigate that failure: run the same test at the base
     commit to learn whether your change caused it. A pre-existing failure in
     the original program still belongs to this assignment. Never widen the run.
   - There is no test corpus or results ledger to run. Do not build one.
   - Put the result in the PR body: `Proof: <tests on the change and the area
     around it> | <clean, or what failed and what it turned out to be>`.
2. **Leave no build output behind:** Ensure no leftover `omega.lock` or
   `build/` directories sit inside the kept fixtures under
   `tests/fixtures/` (never delete its `build/pass/` group).
3. **Update `TASKS.md`:** Delete the completed bullet or clause
   from `TASKS.md` outright (never rewrite into `"preserving / keeping / now
   passes"` history prose), and run `cargo xtask check-rules --base origin/main`.

## Step 5: Publish and Land

CI compiles and tests no Rust code, before or after merge, so run the affected
tests and Clippy yourself; heavy runs go to a LAN VM
([AGENTS.md](../../../AGENTS.md#heavy-checks-run-on-the-lan-vms)). See
[landing](../../../wiki/development/landing.md) for the merge procedure.

1. **Push HEAD to a remote `work/` branch and open the PR as ready for review:**
   ```bash
   # In a compiler lane, PR_BODY also carries the `Gate:` and `Proof:` lines from AGENTS.md.
   cargo xtask provenance  # append this model-audit line to the commit and PR_BODY
   mbx clippy -p <crate> --all-targets -- -D warnings  # each Rust crate you touched
   git push origin HEAD:refs/heads/work/<lane>-<topic>
   cargo xtask check-rules --base origin/main --title "<title>" --body-file PR_BODY
   gh pr create --base main --head work/<lane>-<topic> --title "<title>" --body-file PR_BODY
   ```
2. **Land it:** Review your own diff for soundness, regression coverage and cost, then squash-merge it
   when `admission` passes. You already have permission; waiting for it only
   slows the project down.
3. **Confirm publication and continue unfinished work.** Check the submitted
   head and published commit using the landing guide. Carry the pre-merge test
   result forward. If the original program still has a known failure, follow
   it through the next repair in the same worktree. Choose another requirement
   only after that acceptance is complete or an unfinished handoff is recorded
   under the law of completion. Keep the build cache. Check
   `git status --short` and keep unfinished or untracked work before you fetch
   `main` and check out the new base. Do not clean the worktree blindly.
