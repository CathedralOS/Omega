# Swarm coordination

The [swarm skill](../../.agents/skills/swarm/SKILL.md) covers starting
sessions and the pool. Work follows [advance](../../.agents/skills/advance/SKILL.md);
landing follows [landing](../docs/landing.md). Pool slots are `.agents/worktree/worker-N`.

`mbx xtask worktree acquire <lane>-<topic>` claims a slot.
`mbx xtask worktree status` lists the checkouts.
`mbx xtask claims status` shows live claims.
A claim note disappears with its claim; record findings in the pull request, `TASKS.md` for remaining work, or `OWNER_CODE_CONCERNS.md` for a defect beyond the change.
Release a finished claim with `mbx xtask claims release --ticket <ticket>`.
Land a pull request by squash-merging it once `admission` passes ([landing](../docs/landing.md)).

Apply the [law of completion](../../AGENTS.md#law-of-completion): each
implementation session keeps its context through the whole assignment, the
repairs it needs in every layer, required review, and authorized landing
verified on remote main. Checkpoints and handed-over files keep evidence but
finish nothing.

## Coordinator notes

The [local swarm reference](../../.agents/skills/swarm/references/local.md) covers
splitting work, launch, monitoring, recovery and wind-down. Cursor cloud subagents follow
[the Cursor reference](../../.agents/skills/swarm/references/cursor.md). Devin
sessions follow [the Devin reference](../../.agents/skills/swarm/references/devin.md).

- All sessions share one organization message budget. Launch a batch, let
  claims register, then refill freed slots.
- A started session may not be running. Check each handle; a quiet worktree
  (no commits, uncommitted changes, recent edits or live claim) means check the
  handle before assuming progress. Spawn only through your own subagent
  mechanism.
- A live claim is not a live worker; leases outlive dead agents. Check
  liveness through the handle or worktree activity.
- Preserve dirty work with `git add -A` and a WIP commit on that worktree's
  branch before removing it. Clean worktrees and landed branches can be removed.
  Continue unfinished work on the same branch.
- Before dispatch, state who lands the work and what review it needs
  ([AGENTS.md](../../AGENTS.md#sessions-and-subagents)).

Follow [capability selection](../../AGENTS.md#the-sample-loop): take
regressions and explicit user scope first, then choose by outcome priority
and release dependencies. One session owns an outcome, including the pipeline
crates it needs. Shared files are allowed; reconcile overlaps at landing. Name the item in `TASKS.md`, `TASKS_BOOTSTRAP.md`, or
`TASKS_OPTIMIZER.md` as `**<item>.**`. Check `mbx xtask claims status` before
assigning an outcome that already has an owner.

A claim records who owns an outcome. Path overlaps show up in reports, not as
refusals. Landing still merges one change at a time.
