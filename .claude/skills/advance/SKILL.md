---
name: advance
description: Drive the Omega Rust reference compiler toward completion by working the execution boards (TASKS.md, TASKS_BOOTSTRAP.md, TASKS_OPTIMIZER.md, TASKS_PACKAGE_MANAGER.md) or obvious unspecified work in the same direction. Use when asked to advance, continue, drive, or make progress on the compiler, to work the boards, or to pick up the next task. Not for a specific named bug, a question about existing code, or a review.
---

# Advance the Omega compiler

One iteration of the completion loop: rank the work, isolate it, build it, gate
it, land it. `AGENTS.md` already carries the commands, architecture, crate
placement rule, and conventions; `TASKS.md` already carries board hygiene and the
ownership firewall. Do not restate them — this skill is the loop that runs on top.

Composes with `/loop` for unattended sessions. One invocation is one iteration.

This repository is Rust, Omega, and `sh`; the loop is not a reason to add
JavaScript or Python. The session schedules work with the available harness
tools. The small [admission checker](scripts/verify.sh) only validates lanes,
commit paths, and gate exits; it cannot spawn, switch branches, or publish.
Session manifests, logs, and scratch scripts stay outside the source tree.

Whatever runs the loop must enforce four checks mechanically, not by asking an
agent whether it complied: lanes are pairwise disjoint by path prefix before
anything is spawned; a commit that touched a file outside its lane is refused;
two commits that touched the same file are not both integrated; and a commit
whose gates were not every one literally green is refused. A commit that fails
any of these never reaches `main`.

Use the commands in [the admission procedure](references/admission.md), before
dispatch and before integration. These checks apply to a solo iteration too.

## What done looks like

The bar is not "the board is shorter." It is: **the Rust reference compiler
builds real canaries and serious samples using the full Omega feature set, with
features interleaved.** Interleaving is the actual test — one machine reused
across build time, proofs, runtime, and async in the same program. Features that
each work alone but fail in combination mean the internals have a combinatoric
problem, and that problem is the work.

`canary_suite` is the measurement of that bar, and it is far from green. The
baseline gate list in `AGENTS.md` can be entirely green while the corpus is not,
so the gate list alone never tells you whether the project moved.

## Never claim what you did not run

Every rule here comes from a specific failure that reached a commit message or a
user-facing report. They cost more than the bugs did.

- **A cause you have not measured is a guess — say so.** A worktree build failed
  twice with `LNK1104`; the session reported an mbx defect. The real cause was
  Windows `MAX_PATH`, found by counting the characters in the path. Before you
  name a cause, produce the measurement that distinguishes it from its neighbours.
- **A test you have not watched fail is not coverage.** A canary was committed and
  described as covering two cases. No roster named it, so it never ran. Before
  claiming a test guards a behavior, break the behavior and watch that exact test
  go red.
- **Read the harness before you run it.** Two full ~475-second `canary_suite` runs
  were spent attributing a failure that the filter variables in `AGENTS.md`
  answer in about a second.
- **Never restore a file by copying it back.** A stale copy silently deleted three
  lines that had landed meanwhile. Use `git checkout --`, `git restore`, or
  `git show <sha>:<path>`, and re-read `git diff` before staging.
- **Report what you did not run.** A gate list you imply you ran costs more than
  the failure would have.

## Pick the work

Orient first, always:

1. `git fetch` and read the newest commits in the lane you are about to touch.
   Other sessions land in this same repository, so also read `git status`: files
   dirty in the main checkout are someone's work in progress and their lane is
   taken. If `git pull --rebase` refuses because the tree is dirty, that is not
   your tree to clean — read instead, and leave their files alone.
2. **Rank before you read prose.** Board entries say what is unfinished, not what
   is expensive. Get the actual distribution:

   Use a recent, revision-labelled distribution when one exists. Refresh the
   full measurement only when needed to rank the work; preserve its exit code:

   ```sh
   corpus_status=0
   mbx test -p compiler --test canary_suite > /tmp/canary.txt 2>&1 || corpus_status=$?
   printf 'canary exit=%s\n' "$corpus_status"
   grep -oE 'message: "[^"]{0,90}' /tmp/canary.txt | sed 's/^message: "//' \
     | sed -E 's/`[^`]*`/X/g; s/[0-9]+/N/g; s/: .*$//' | sort | uniq -c | sort -rn | head
   ```

   Prefer the blocker holding down the most canaries. One fence has accounted for
   the clear majority of all failures; a task worth one canary is not the same
   size of work as one worth hundreds, and the board does not say which is which.
3. Read the relevant board. If the invocation names a board or its area, that
   board is the only one in scope — do not widen to the others because it looks
   thin, and do not touch their entries. Unnamed means all four boards are the
   candidate pool. Either way you leave with one task.
4. **Probe the acceptance condition before committing to the task.** Write the
   smallest program or filtered test that exercises what the board claims, and
   run it. It must fail *for the reason the board gives*. If it fails for another
   reason, that reason is the real work. If it passes, the entry is stale — delete
   it and pick again. This is usually one compile, and it is how the highest-value
   work of a recent session was found.
5. Unspecified but obvious work counts — a missing lowering, an unimplemented
   stage, a representation with no producer. It does not need a board entry to be
   the right next task. **A regression counts too**: boards track unfinished work,
   never broken work, so nothing on them will ever point at one.

**Side-quests.** The test is whether the work moves or unblocks a board
acceptance condition. If it does not, it is a side-quest, however tempting:
tidying a file you were passing through, refactoring a crate that is not
blocking, broadening coverage on a feature that already works, renaming for
consistency. Note it and move on. If it names real unfinished work with an
acceptance condition, add a board line; otherwise leave it. Do not open a second
front.

## Isolate it

**Do the work in a worktree, not in the main checkout.** Several sessions share
this repository. In the shared tree, HEAD moves under you, files you have read
change on disk, and gate output mixes your breakage with three other sessions'
half-finished code so that nothing is attributable.

Prefer a short, fixed worktree path per active worker and keep its `target/`
between iterations. Before reusing a slot, inspect `git worktree list`, verify
its repository and branch, and require a clean tracked and untracked state.
Keep the previous result reachable by branch or full SHA before switching to a
new branch at the fetched base. Never force-switch, reset, clean, or unlock a
slot containing another session's work. If ownership is uncertain, use a new
short path. Never share one writable target directory between active slots.

The [measured reuse experiment](evals/fixed-worktree-2026-09-05.md) reduced the
library-test compiler phase from 102 seconds to 10.76 seconds across one source
revision in the same path. This is local artifact reuse, not an mbx action-cache
hit rate or a claim that the full test suite became green.

Delegate only substantial independent tasks. In Claude Code, use an existing
slot through an explicit working directory when supported; otherwise use
`isolation: "worktree"` and verify the returned path. In Codex, create/select
the Git worktree explicitly and pass its absolute path to the worker; context
isolation alone does not isolate files. The assigner checks lanes before
spawning and gives every worker its own scratch path. A solo session uses the
same worktree and admission checks without spawning an agent.

Give each agent: its one task, the acceptance condition to probe, the gates to
run, and the instruction to **commit in its own worktree and neither push nor
rebase**. Assign agents to disjoint crate lanes. One assigner means no claim file
and no race.

Worktrees share the object store, so an agent's commit is reachable from the main
checkout by SHA even after its worktree is removed. Have every agent report the
full SHA; that is the integration handle.

**Path budget.** Generated artifact paths add about 147 characters to the
worktree root, against a 260-character `MAX_PATH` that `link.exe` enforces. The
harness worktree root is about 74 characters and links fine. A worktree created
by hand under the session scratchpad reaches 271 and fails every test-binary link
with `LNK1104: cannot open file` — for a file that exists. Never put a worktree
under the scratchpad. If a link fails, measure the path length before blaming a
tool.

The assigner can inspect main but integrates in its own clean worktree. Shared
main remains untouched until the checked result can be fast-forwarded.

Two worker-prompt rules, each from a measured failure in the first live
iteration:

- **Gate in the foreground and do not return before the commit.** Every worker
  that backgrounded a gate and returned "waiting for a notification" paused
  there; each resume is a fresh turn that re-reads `AGENTS.md` and this skill.
  Four pauses cost more wall time than the gates did.
- **Probes name fixtures, not test functions.** `OMEGA_PASS_CANARY_FILTER`
  matches the `group/name` fixture path (`targets/aarch64_hfa_entry_argument`);
  a Rust test name (`entry_and_abi::…`) goes to the cargo test filter instead.
  Two workers received the wrong shape and had to correct it.

The orchestrator also runs no builds of its own while workers gate.
`bounded-process` has zero workspace dependencies and its CPU-limit test
says in its own comment that a loaded host starves it of user time; it went
red three times under concurrent builds and passed 7/7 with and without the
change the moment the machine was idle. A red gate on a crate the change
cannot reach is attributed with `cargo tree` and an idle re-run, not by
assertion — and not by ignoring it.

## Build it

Priorities specific to this codebase, in the order they bite:

- **Coherent lowering into portable Psi first, then into native.** Terminal Psi is
  the only portable boundary. A native shortcut that skips it is not progress.
- **Arena-backed packed allocation over fragmented `Vec`s.** `Handle<T>` and
  `HandleSpan<T>` for repeated child lists in lowered representations.

Do not add a crate until a module boundary has stopped moving. A crate needs an
independent consumer or a stable dependency; otherwise it is a module inside
its owning transform. A stage doc is a contract, not a crate request. Preserve
the connected X-to-Y, Y-to-Y, Y-to-Z program route when placing a calculation.
The architecture test keeps a closed roster of Omega pipeline directories;
an addition needs its ownership justification and roster update in the same
commit.

A new fixture under `tests/omega/{pass,fail}` is inert until a roster in
`omega-rust/omega/compiler/compiler/tests/canary_suite.rs` names it. Add
the roster entry in the same commit, then prove the case runs and fails without
your change.

## Gate it

Run `mbx --version` first. It must report 1.7.0 or newer. If it does not, stop
and report the missing prerequisite; never substitute direct Cargo. Every
compiling command uses `mbx`, including in a worktree or background job. Only
`cargo fmt` and `cargo clean` remain direct Cargo commands.

Inner loop, while iterating:

```bash
mbx check --workspace --all-targets
```

plus the filtered canary for what you are changing — see the filter variables
under "Running one test" in `AGENTS.md`. Never run the unfiltered suite to
attribute one change.

Then checkpoint the complete change in the worker's branch and run
`scripts/verify.sh gates` as described in the admission procedure. This runs
the full baseline list from `AGENTS.md` on a clean commit and records every
actual exit against its full SHA. A checkpoint is not permission to integrate:
all five results must be zero, and any amendment invalidates those results.
In a worktree that tree is exactly your change, which is the point.
The list is not conditional on which files you touched: fmt and clippy
read every file, the architecture test reads crate layout, and library tests read
checked-in fixtures, so a commit with no `.rs` file in it can still move all
three. The architecture test is the only thing that catches a wrong-direction
dependency.

Report what is red now, what was already red, and what you did not run. Attribute
a red gate by re-running it filtered, or with your change reverted in the
worktree — never by asserting it belongs to someone else.

## Land it

**Land on `main`.** Invoking this skill is the user's standing authorization to
commit and push there. Worktrees are isolation, not a branching strategy: the
work still arrives on `main` in this iteration. A loop that leaves work on side
branches never lands anything.

Validate each worker with `commit` and `green` in the admission procedure,
then cherry-pick accepted commits in sequence into a clean integration worktree
at the fetched main revision. Retain each accepted path list so the next
admission rejects overlap. A red baseline explains a failure; it never counts
as green or authorizes skipping a gate.

Run the checker gates once on the integrated result. Fetch again before
landing; if main advanced, rebase the integration branch and gate the new SHA
again. Verify shared main is clean and at the expected base before
fast-forwarding it and pushing. If another session changed it, keep the checked
branch and report the conflict; do not autostash or overwrite their work. The
user's explicit delivery scope takes precedence over this default landing step.

Commit on coherent milestones — a working improvement, not a finished epic.
Small checkpoint commits are correct; a single giant commit at the end is not.
`AGENTS.md` "Workflow" governs the message.

Update the board in the same change: delete the task when its acceptance
condition passes. Do not append landed substeps, test counts, or release notes —
boards are execution state, not changelogs.

**Debt gets registered on the way in, not on the way out.** If a change leaves
behind state the docs call temporary, transitional, bounded, or compatibility, it
gets a board entry in the same commit, naming the crate and the acceptance
condition that deletes it. Landing the temporary thing without its deletion
condition is the whole failure mode.

## When blocked

**Design blockers go to `OWNER_QUESTIONS.md`. Implementation details do not.**
If the end shape is known and only the work remains, it is not a design question
— build it. Engineering difficulty is never a design blocker.

Before filing, confirm it clears the bar that file sets: an independently
motivated product requirement or credible external use case. A test, benchmark,
or implementation task is never sole motivation. Audit first whether ordinary
Omega already expresses the customer.

If a slice is genuinely blocked and the question is filed, say so plainly and
move to the next task in the same session rather than stalling.
