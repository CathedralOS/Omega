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

You are on Linux x86-64. `mbx` is absent here; use `cargo` everywhere AGENTS.md
says `mbx`. `cargo nextest` is installed. Windows, macOS, and QEMU acceptance
is unavailable on this host and must be reported as unavailable, never as
passing.

## Coordination

Register your assignment before editing so other machines, waves, and local
sessions see it:

`{claim_command}`

Retain the returned ticket. Exit 2 lists a live conflicting claim — stop and
report `blocked` with the named owner and reason; do not retry with
`--allow-overlap` or work around it. If you approach the lease expiry,
`python3 tools/claims.py renew --ticket <ticket>`. Release with
`release --ticket <ticket>` when you finish for any reason.

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
for an unchanged verdict. A row carries at most one current verification
line; a stamp that does land replaces the dated observation it supersedes
rather than stacking under it. Never touch these excluded items:
{exclusions}.
If your fix needs a path owned by another item in the wave, stop and report
`blocked` with the needed path so the coordinator can arrange a handoff. Apply
the same rule to a confirmed active claim outside this wave
(`tools/claims.py status`). Multiple
unassigned stages may belong to your slice; do not stop merely at a crate boundary.

Refetch `origin/main` before you `enqueue`. If the item's acceptance already
landed, or another author materially changed its text, stop and report
`superseded`.

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
- `blocked`: an external obstacle (host, access, another item's path, a
  prerequisite item) stopped you before the slice could be attempted.
- `budget_exhausted`: only when you were actively implementing a slice you
  still believe is landable and ran out of ACU doing so; record the partial
  state and the next step in `remaining_dependency`. Do not describe a
  verification-only investigation as implementation budget exhaustion.

`first_build_seconds` is the wall time of your
first workspace check in this session. `lease_expiries` counts landing-lease
expiries you hit. `unrelated_failures` lists confirmed-unrelated failures you
observed but did not repair.
