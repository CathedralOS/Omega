# Dot Swarm (Dot Native Collaboration Adapter)

This reference defines the `dot` native-collaboration transport for running concurrent agents on a shared cloud computer.

Session ownership, execution and publication follow the [swarm skill](../SKILL.md), [advance](../../advance/SKILL.md), and [landing](../../../../tools/landing.md).

## 1. Dot Native Collaboration Tools & Executor Budget

Apply [cloud launch preflight](cloud-launch.md), [recovery](recovery.md), and [cloud coordination](cloud-coordination.md):

| Operation | Native tool |
| --- | --- |
| Start a bounded assignment | `collaboration.spawn_agent` |
| Send information to a running worker | `collaboration.send_message` |
| Continue an assignment after its turn ended | `collaboration.followup_task` |
| Reconcile current handles and statuses | `collaboration.list_agents` |
| Wait when no useful independent step remains | `collaboration.wait_agent` |
| Stop an active turn when necessary | `collaboration.interrupt_agent` |

- `send_message` does not restart an idle worker; use `followup_task` to resume a settled worker.
- The runtime enforces seven active-agent slots across the entire root tree (including the coordinator). Native agents share one filesystem and CPU/RAM budget: run one host Rust build at a time and give each editing worker its own `worker-N` pool slot:
  ```bash
  cargo xtask worktree acquire <lane>-<topic>
  ```
  Local named branches are banned. Record slot assignments before dispatch in `build/swarm/<wave>/inflight.json`.

## 2. Dispatch & Reactive Drain

1. **Dispatch:** Fetch `origin/main`, check `cargo xtask claims status`, and give each worker its worktree path, base SHA, assigned capability and witness, and instructions to follow `/advance` through verified landing.
2. **Reactive wakeup only:** Never schedule timers, cron jobs, or periodic check-ins. When no coordinator work remains runnable, end the turn (or use `collaboration.wait_agent`) and let worker completion reports wake the coordinator.
3. **Continue:** After the worker verifies its landing and reports the PR URL, submitted head, published commit and observed limits, use `collaboration.followup_task` on the same worker and warm worktree for the next requirement of the parent capability, and append settled outcomes to `build/swarm/<wave>/outcomes.json`.
