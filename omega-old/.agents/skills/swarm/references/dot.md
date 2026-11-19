# Native in-session collaboration

These tools start agents on the current computer, including a local Windows
workstation. Their presence does not identify the product as Dot, provision a
cloud machine, or require a remote-artifact recovery workflow.

Execution and publication follow [advance](../../advance/SKILL.md),
[AGENTS.md](../../../../AGENTS.md), and [landing](../../../../tools/docs/landing.md).

## Tools and capacity

| Operation | Native tool |
| --- | --- |
| Start a bounded assignment | `collaboration.spawn_agent` |
| Send information to a running worker | `collaboration.send_message` |
| Continue an assignment after its turn ended | `collaboration.followup_task` |
| Reconcile current handles and statuses | `collaboration.list_agents` |
| Wait when no useful independent step remains | `collaboration.wait_agent` |
| Stop an active turn when necessary | `collaboration.interrupt_agent` |

- `send_message` does not restart an idle worker; use `followup_task` to resume a settled worker.
- Use the capacity actually exposed by the runtime, including the coordinator;
  do not assume a fixed agent count or change user configuration to obtain more.
- Each editing worker gets an isolated `worker-N` pool slot through `/advance`.
  Preserve its fixed path and warm target across successive repairs.
- Workers share this host's CPU, memory and disk. Set the current
  [host build budget](../../../../tools/docs/build_slot.md) before builds and include
  it in every worker prompt; shell settings are not inherited by subagents.
  Keep target directories separate and use the shared mbx scheduler budget.

## Dispatch and continuation

The coordinator performs `/advance` Step 1, then dispatches an outcome with its
worktree, fixed base, claim result, witness, acceptance and build budget. The
worker owns investigation, repair, focused checks, self-review and verified PR
landing. Claims are informational; an overlap is reconciled, not permission to
substitute peripheral work. Share actual producer dependencies, not file locks.

Keep a small ignored wave record under `build/swarm/<wave>/` with assignments,
returned handles, pending command IDs and landing evidence. No Python ledger,
schema migration, cloud identity or external upload is required for this shared
filesystem. Preserve committed source and use normal PR publication.

Observe the exact existing command or worker after a timeout; silence does not
prove it stopped. Do not launch a replacement to discover whether it is alive.
When no independent work remains, end the turn or use `wait_agent`; do not poll
in loops or schedule check-ins. After verified landing, use `followup_task` to
continue on the same worker and warm worktree; `send_message` alone does not
restart an idle worker. Release completed owned claims and slots through the
current repository commands, preserving unfinished work and shared caches.

Only when a task actually selects a separate remote environment should its
provider's cloud setup and recovery guidance apply. Native tools on an existing
computer are not such a selection and confer no extra provisioning authority.
