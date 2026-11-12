# Devin Swarm (Devin Cloud Session Adapter)

This reference defines the Devin cloud-session transport for coordinating a wave of Devin sessions.

Session ownership, execution and publication follow the [swarm skill](../SKILL.md), [advance](../../advance/SKILL.md), and [landing](../../../../tools/landing.md).

## 1. Devin Session Dispatch & Notifications

1. **Preflight & reservation:** Complete `/advance` preflight and [claim registration](../../../../tools/claims.md), and record assignments and session IDs under `build/swarm/<wave>/`.
2. **Session launch & follow-up:**
   - Honor actual Devin service capacity; never run two sessions in one checkout.
   - Pass `notify_on_response=true` where supported, and re-arm on follow-ups (notifications are one-shot). Retain returned session IDs and message page cursors.
   - Inject `"secret_ids": ["TYPESAFE_API_KEY"]` in the manifest when Jev advisories are used and `api.typesafe.ai` is reachable.
3. **Reactive wakeup only:** Never schedule periodic timers, cron jobs, or polling loops; act on session response notifications.

## 2. Continue on Warm Sessions

1. Dispatch each session with its capability, witness, fixed base, and instruction to follow `/advance` through verified landing. Retain the PR URL, submitted head, published commit and observed limits in its completion report.
2. Reuse the same Devin session (keeping its warm checkout and build cache) for the next requirement of the parent capability; compact only after the parent capability's full acceptance is met and the session switches to a new outcome. Follow [recovery](recovery.md) for interrupted lanes.
