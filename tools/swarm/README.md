# Swarm coordination

Session transport and pool setup are the [swarm skill](../../.agents/skills/swarm/SKILL.md).
Execution follows [advance](../../.agents/skills/advance/SKILL.md), and publication follows
[landing](../landing.md). Pool slots are `.agents/worktree/worker-N`.

`mbx xtask worktree acquire <lane>-<topic>` claims a slot.
`mbx xtask worktree status` lists the checkouts.
`mbx xtask claims status` shows live claims.
Record a finding with `mbx xtask claims note --ticket <ticket> --text "<finding>"`.
Release a finished claim with `mbx xtask claims release --ticket <ticket>`.
Queue a pull request whose checks are already green with
`mbx xtask land enqueue <pr> --expected-head <sha>`.

Apply the [law of completion](../../AGENTS.md#law-of-completion): each
implementation session retains context through its complete assigned acceptance,
necessary cross-layer repairs, required review and authorized landing verified
on remote main. Checkpoints and transferred artifacts preserve evidence; they
are not finish lines.

## Coordinator notes

The [local swarm reference](../../.agents/skills/swarm/references/local.md) carries
partition, launch, monitor, recover, and drain. Cursor cloud subagents follow
[the Cursor reference](../../.agents/skills/swarm/references/cursor.md). Devin
sessions follow [the Devin reference](../../.agents/skills/swarm/references/devin.md).

- One org message budget covers every session. Launch a batch, let claims
  register, then backfill a freed slot.
- A spawned slot is not a running slot. Verify each handle, and correlate a
  quiet worktree (no commits, no dirt, no recent mtimes, no live claim) with
  that handle before assuming progress. Spawn only through the session's own
  subagent mechanism.
- A live claim is not a live worker. Leases outlive dead agents. Verify
  liveness through the agent handle or through worktree activity.
- Preserve dirty work with `git add -A` and a WIP commit on that worktree's
  branch before removing it. Clean worktrees and landed branches can be removed.
  Continue unfinished work on the same branch.
- Declare publication and required review under
  [AGENTS.md](../../AGENTS.md#sessions-and-subagents) before dispatch.

Follow [capability selection](../../AGENTS.md#the-sample-loop): retain regressions
and explicit user scope, then select by outcome priority and release
dependencies. One session owns an outcome, including the pipeline crates that
outcome needs. Shared files are allowed; reconcile overlapping implementations
at landing. Name the item in `TASKS.md`, `TASKS_BOOTSTRAP.md`, or
`TASKS_OPTIMIZER.md` as `**<item>.**`. Check `mbx xtask claims status` before
assigning an outcome that already has an owner.

Claims record outcome DRIs. Path overlaps appear in reports, not as a
rejection. Landing still serializes publication.
