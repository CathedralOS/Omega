---
name: devin-swarm
description: Coordinate a requested Devin cloud swarm with persistent outcome ownership and coordinator-reviewed landing. Every worker follows /advance, and the coordinator reviews every candidate diff with coordinator-review.
---

# Devin Swarm (Platform Adapter)

This skill defines **only** the Devin cloud-session transport for coordinating a wave of Devin sessions.
- **Worker workflow:** Every session follows [advance](../advance/SKILL.md) (`/advance`) and [AGENTS.md](../../../AGENTS.md#law-of-completion) for capability selection, validation, and board updates. Point sessions to `.agents/skills/advance/SKILL.md` rather than duplicating `/advance` instructions.
- **Coordinator review & landing:** Sessions push a **draft** PR (`gh pr create --draft`) and stop (`candidate_ready`). The coordinator audits every diff using [coordinator-review](../coordinator-review/SKILL.md) before running `python3 tools/landing.py land <pr> --expected-head <sha>`.

## 1. Devin Session Dispatch & Notifications

1. **Preflight & reservation:** Fetch `origin/main`, check `python3 tools/claims.py status`, and follow [dispatch.md](../../../tools/swarm/dispatch.md) to reserve outcomes and record the wave ledger under `build/swarm/<wave>/` (using the `board`/`item` manifest shape in `tools/swarm/launch.py`).
2. **Session launch & follow-up:**
   - Honor actual Devin service capacity; never run two sessions in one checkout.
   - Pass `notify_on_response=true` where supported, and re-arm on follow-ups (notifications are one-shot). Retain returned session IDs and message page cursors.
   - Inject `"secret_ids": ["TYPESAFE_API_KEY"]` in the manifest when Jev advisories are used and `api.typesafe.ai` is reachable.
3. **Reactive wakeup only:** Never schedule periodic timers, cron jobs, or polling loops; act on session response notifications.

## 2. Review, Land & Continue on Warm Sessions

1. Each Devin session follows [advance](../advance/SKILL.md) (`/advance`), validates its repair on its changed compiler, pushes `HEAD` to `origin/work/<name>`, opens a draft PR (`gh pr create --draft`), and reports `candidate_ready`.
2. The coordinator runs the 5-point diff audit in [coordinator-review](../coordinator-review/SKILL.md), marks the PR ready, and lands it via `python3 tools/landing.py land <pr> --expected-head <sha>`.
3. Reuse the same Devin session (keeping its warm checkout and build cache) for the next requirement of the parent capability; compact only after the parent capability's full acceptance is met and the session switches to a new outcome. Follow [recovery](../dot-swarm/references/recovery.md) for interrupted lanes.
