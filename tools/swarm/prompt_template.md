# Swarm wave {wave} session: {name}

You are one session of a coordinated wave. Your customer is board item
**{item}** in `{board}`. Read its current text from fresh `origin/main`, not
from this prompt; the board may have moved since this prompt was written.

Read `AGENTS.md` fully, then run the `advance` skill
(`.agents/skills/advance/SKILL.md`) restricted to this item. Do not select
another item.

{suggested_first_slice_block}

{probe_block}

## Host

You are on Linux x86-64. `mbx` is installed when your snapshot carries the
blueprint step (run `mbx --version` once; if absent use `cargo` everywhere
AGENTS.md says `mbx`, per its documented fallback). `cargo nextest` is
installed. Windows, macOS, and QEMU acceptance is unavailable on this host
and must be reported as unavailable, never as passing.

## Coordination

Register your assignment before editing so other machines, waves, and local
sessions see it:

`{claim_command}`

Keep the ticket and own this outcome through acceptance and integration.
Exit 2 means another DRI owns the same outcome: ask the coordinator for a
handoff or a delegated contribution, not an unrelated task. Paths are advisory:
overlaps do not reject or require permission. Notify peers where practical,
continue necessary work in this worktree and reconcile at landing. Refresh old
claims clients that still reject file overlaps; existing tickets remain valid.
Renew before expiry. A checkpoint does not end ownership.

Keep a short assignment record: outcome, acceptance, current failure, next
necessary action and dependencies. Reread it after compaction. New obstacles
normally belong to this outcome; do not manufacture residual tasks or switch
to profiling/testing work without completing or handing off the assignment.

Exit 3 means coordination timed out, not a live conflict. Keep the pending
claim record, report the outage, and continue the assigned work in isolation.
Do not repeatedly wait for registration. If a write timed out, inspect status
before retrying; its outcome may be uncertain. Landing remains mandatory.

## Collision rules

Never edit board or coordination files (`TASKS*.md`, `OWNER_QUESTIONS.md`,
`tools/swarm/waves/`): board text is the coordinator's, and `landing.py`
refuses candidates that touch only those files or change nothing — do not
land `board:` commits or empty ledger commits. Record durable findings on
your claim ticket instead — `python3 tools/claims.py note --ticket <ticket>
--text "<finding>"` — and repeat them in the structured output; the
coordinator writes board updates at drain. A re-verification that confirms
the board's current text is the common case and rides the claim note alone:
do not take a board-only claim to stamp the row, and expect no board edit
for an unchanged verdict. Re-verification at a newer revision that reproduces
the recorded verdict is noise, not evidence — no stamp lands for it. A row
carries at most one current verification line; a stamp that does land
replaces the dated observation it supersedes rather than stacking under it,
and at most one stamp per assigned row per session is permitted — only when
it changes the row's frontier. Serial witness paragraphs ("re-verified a
second/third time") are never appended. Never touch these excluded items:
{exclusions}.
Required edits may span other workers' paths and multiple stages. Reconcile
shared repairs rather than adding a competing mode or dropping incoming work.
Refetch main before landing. If acceptance already passes, verify it and report
superseded; a changed board paragraph alone does not end the assignment.

## Landing

Publish only through `python3 tools/landing.py` with
`--owner "{owner_label}"`. Rebase onto current `origin/main` and validate
immediately before `enqueue`. After `claim`, rerun the checks whose inputs
changed. On lease expiry, `release` and rejoin; never bypass the reservation.
`landing.py` requires full lowercase 40-character SHAs for `--base` and
`--candidate`; `publish` takes `--claim <ticket>`, and its `--base` must
equal the reserved base the claim printed (main may move between `enqueue`
and `claim` — rebase onto the reserved base); an expired head leaves your
commit intact — re-enqueue it after the queue clears.
Give `publish` the model that produced the change: `--provenance "agent=<tool> model=<name> effort=<level>"`
(or export `OMEGA_PROVENANCE` once). Commit messages carry no attribution; this note is the only record.
Keep checkpoints bounded, but retain the assigned customer through producer and
consumer repairs. Rerun its outer command after relevant changes; helper tests
alone do not complete the assignment or justify claiming native execution.

## Budget

`max_acu_limit` is {max_acu_limit}. If you approach it without a landable
improvement, stop and put the witnessed failure, tested revision, command,
host, and next acceptance in the structured output. Board-only churn is not an
improvement.

## Commits and reasoning

Commit naming follows the AGENTS.md lanes (`lane: statement`). Every commit
carries a body covering the previous behavior, rejected alternatives, and the
gates that ran — omit it only when the subject already carries the full
reasoning (AGENTS.md commit convention). Preserve the reasoning a next reader
needs on the common reading path, per the advance skill.

## Structured output

When you finish, for any result, fill this schema exactly:

```json
{structured_output_schema}
```

`result` is one of `landed`, `verification_only`, `blocked`, `superseded`,
`budget_exhausted`, `aborted`. Choose it by the evidence, not by why you
stopped:

- `verification_only`: investigation produced evidence but no implementation.
  Name the exact missing dependency and next acceptance. This is the planned
  outcome only for a probe-only assignment; in an implementation assignment,
  diagnosis is intermediate while a design-backed repair remains actionable.
  A rejection or a slice spanning stages is not by itself a stop condition.
- `blocked`: an actual missing decision, environment or external prerequisite.
  Shared files and missing implementation are not blockers.
- `budget_exhausted`: only when you were actively implementing a slice you
  still believe is landable and ran out of ACU doing so; record the partial
  state and the next step in `remaining_dependency`. Do not describe a
  verification-only investigation as implementation budget exhaustion.

`first_build_seconds` is the wall time of your
first workspace check in this session. `lease_expiries` counts landing-lease
expiries you hit. `unrelated_failures` lists confirmed-unrelated failures you
observed but did not repair.
