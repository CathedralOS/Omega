# Dot Swarm (Dot Native Collaboration Adapter)

This reference defines the `dot` native-collaboration transport for running concurrent agents on a shared cloud computer.
- **Worker workflow:** Every worker follows [advance](../../advance/SKILL.md) (`/advance`) and [AGENTS.md](../../../../AGENTS.md#law-of-completion) for capability selection, validation, and board updates. Point workers to `.agents/skills/advance/SKILL.md` rather than duplicating `/advance` instructions.
- **Coordinator review & landing:** Workers push a **draft** PR (`gh pr create --draft`) and stop (`candidate_ready`). The coordinator audits every diff using [coordinator-review](../../coordinator-review/SKILL.md) before running `python3 tools/landing.py land <pr> --expected-head <sha>`.

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
- The runtime enforces seven active-agent slots across the entire root tree (including the coordinator). Native agents share one filesystem and CPU/RAM budget: run one host Rust build at a time (`tools/build_slot.py`) and give each editing worker its own detached worktree:
  ```bash
  git worktree add --detach .agents/worktree/<wave>-<name> origin/main
  ```
  Local named branches are banned. Record slot assignments before dispatch in `build/swarm/<wave>/inflight.json`.

## 2. Dispatch, Reactive Drain & Review

1. **Dispatch:** Fetch `origin/main`, check `python3 tools/claims.py status`, and give each worker its worktree path, base SHA, assigned `TASKS.md` capability and witness, and instructions to follow [advance](../../advance/SKILL.md) (`/advance`) and stop at a draft PR (`candidate_ready`).
2. **Reactive wakeup only:** Never schedule timers, cron jobs, or periodic check-ins. When no coordinator work remains runnable, end the turn (or use `collaboration.wait_agent`) and let worker completion reports wake the coordinator.
3. **Review and continue:** On `candidate_ready`, run the 5-point audit in [coordinator-review](../../coordinator-review/SKILL.md). Once landed, use `collaboration.followup_task` on the same worker and warm worktree for the next requirement of the parent capability, and append settled outcomes to `build/swarm/<wave>/outcomes.json`.
