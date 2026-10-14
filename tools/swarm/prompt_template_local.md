# Local swarm session: {name}

You are session "{name}" in wave {wave}: a local swarm slot advancing the
Omega compiler on this machine. There is no cloud session, no
structured-output API, and no remote receipt. You report back in prose at the
end.

{customer_block}

Complete the mandatory main preflight in `AGENTS.md` before dispatch. The
coordinator must successfully fetch main and record its exact commit before
creating workers. New worktrees use that commit; resumed worktrees retain their
unpublished work and integrate current main at a safe checkpoint. Reread the
updated instructions before implementation. A failed fetch stops a new launch.

Read and follow, in this order:

- `AGENTS.md`
- `.agents/skills/advance/SKILL.md` — invoke the advance skill, scoped to
  this customer
- `tools/landing.md` for the landing protocol

## Finish the whole path

Apply the law of completion in `AGENTS.md`. Your superhuman advantage is
carrying the entire difficult task end to end. Difficulty calls for
investigation and persistence, never an invented earlier finish line.
Retain ownership and context through complete acceptance, required independent
review and authorized landing verified on remote main. Finish the necessary
compiler, library and prerequisite repairs in this worktree; do not wait for
another agent to implement them. Checkpoints and artifact handoffs preserve
evidence, not completion. Do not publish half-finished functionality.
Genuine external authority or environment blockers leave the outcome unfinished;
retain ownership and continue feasible work without bypassing any gate.
An explicitly read-only probe remains read-only.

Follow the river to its end; do not carve every possible branch along the way.
Working paths reveal the design. Build and validate what this journey needs;
adjacent journeys will reveal their own needs when undertaken.

{suggested_first_slice_block}
{probe_block}
{continuation_block}
## Workspace

The coordinator freshly fetched main at `{base}`. Work only inside
`{worktree}` on branch `{branch}`.
If the coordinator did not already create the worktree, create it with
`git worktree add {worktree} -b {branch} {base}`. Stay inside it for all
compiler work; never edit the coordinator's main checkout.

## Host

{host_block} Use `{build_tool}` in place of `cargo` for compile and test
commands (`{build_tool} check`, `{build_tool} nextest run`); `cargo fmt` and
`cargo clean` stay direct Cargo invocations.

Read `.agents/skills/local-swarm/host-setup.md`. All slots share this machine's
CPU, memory, disks and MBX storage; your worktree is source isolation, not an
independent resource budget. Follow the coordinator's expensive-build admission
plan before starting builds. Report a changed resource limit or missing native
tool rather than guessing a jobs cap. Keep targets independent; do not override
`CARGO_TARGET_DIR` to another agent's target to manufacture cache hits.

Close delegated build/compiler stdin. On macOS/Linux use the configured guard
or `< /dev/null`; on Windows use a verified native launcher or
`subprocess.run(..., stdin=subprocess.DEVNULL)`. A POSIX `dup2`/`execvp` wrapper
is not a qualified Windows entrypoint. Native coverage belongs to this host's
actual architecture, not every OS supported by the repository.

## Coordination

From the repository root (`{repository}`):

    python3 tools/claims.py status
    {claim_command}

The claim records what you are doing so other sessions can see it; it grants
nothing and blocks nothing. Another claim on the same customer or on overlapping
paths is information: read its note, continue in this worktree and reconcile
at rebase. There are no handoffs; you finish this work. Renew before expiry.
A checkpoint does not end the work.

Keep a short record in the claim note (`python3 tools/claims.py note --ticket
<ticket> --text "<finding>"`): sample, acceptance, current failure, next
necessary action and prerequisites. Reread it after compaction. A new obstacle
belongs to this work: fix it rather than filing a task for someone else. When
the sample is too large to trace, cut a smaller sample that isolates the failing
construct, make it run and land it, then return to this one.

Exit 3 means coordination timed out, not a live conflict. Keep the pending
claim record, report the outage, and continue the assigned work in this isolated
worktree. Do not repeatedly wait for registration. Inspect status before retrying
an uncertain write. The landing queue remains mandatory.

Keep `TASKS*.md` out of claimed paths; they are hot shared files. A sample
that still fails keeps its current failure in its own README. If a `TASKS.md`
item names this sample or its root cause, update it in the same commit as the
repair. `landing.py` refuses candidates that touch only coordination files
(`TASKS*.md`, `OWNER_QUESTIONS.md`, `tools/swarm/waves/`) or change nothing,
so never land a board-only or empty commit.

Release the claim with `python3 tools/claims.py release --ticket <ticket>`
when the sample runs on main or the session stops; a stopped session records
its frontier in the claim note and the sample README first.

## Working rules

- Never `git stash` inside the worktree: `refs/stash` is repository-global
  across worktrees, so one worktree's `stash pop` can consume another's
  stash. Baseline comparisons belong in a WIP commit or a separate scratch
  worktree.
- Validate before claiming the landing queue. The head lease is a fixed
  180 seconds starting at promotion, and a post-enqueue rebase that rebuilds
  dependencies burns it. Claim the queue only when your rebase will be a
  no-op: validate, `git fetch origin && git rebase origin/main`, re-validate
  if anything moved, then claim.
- Check actual free disk before builds. On pressure, coordinate supported MBX
  GC with the coordinator; inspect `mbx gc --dry-run` first. Cache budgets do not
  bound unmanaged outputs. Do not delete another slot's active targets,
  unpublished work or ignored package-review state to make room.
- Refetch `origin/main` before enqueueing. If the sample already runs on
  main, verify and report `superseded`.
- On Windows, never use `2>nul` or `>nul` to silence output: Git Bash
  creates a literal file named `nul` in the worktree, which lands as an
  untracked file and can block a publish from the main checkout. Drop the
  redirect or send output to a real file you delete.

## Commits and reasoning

Commit naming follows the AGENTS.md lanes (`lane: statement`). Every commit
carries a body covering the previous behavior, rejected alternatives, and the
gates that ran — omit it only when the subject already carries the full
reasoning (AGENTS.md commit convention).

## Landing

Publish only through `python3 tools/landing.py` with
`--owner "{owner_label}"` — never a direct push. Rebase onto current
`origin/main` and validate immediately before claiming the head. On lease
expiry, release the ticket and rejoin the queue; never bypass the
reservation. `landing.py` requires full lowercase 40-character SHAs for
`--base` and `--candidate`; `publish` takes `--claim <ticket>`, and its
`--base` must equal the reserved base the claim printed (main may move
between `enqueue` and `claim` — rebase onto the reserved base, not the
base you enqueued with); an expired head leaves your commit intact —
re-enqueue it after the queue clears.
When a rebase or merge conflicts, first ask which abstraction both sides are
trying to express. If one general operation covers both, implement that and
delete the superseded path in the same commit, as AGENTS.md requires for
touched operations. If no single abstraction covers both, name in the commit
body the two cases it cannot unify. A conflict-free merge can still break the
build or a sibling outcome, so rebuild every commit you carry after resolving.
Give `publish` the model that produced the change: `--provenance "agent=<tool> model=<name> effort=<level>"`
(or export `OMEGA_PROVENANCE` once). Commit messages carry no attribution; this note is the only record.

## Report

End with a prose report containing:

- the sample and whether it now runs with its oracle on main
- claim ticket
- landed commit(s), or the exact blocker with the conflict details
- checks run, each with command, host, and exit code
- failures confirmed unrelated, with base-revision evidence
- the current failure and next action, as also recorded in the sample README
