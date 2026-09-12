# Swarm wave {wave} session: {name}

You are one session of a coordinated wave. Your customer is board item
**{item}** in `{board}`. Read its current text from fresh `origin/main`, not
from this prompt; the board may have moved since this prompt was written.

Read `AGENTS.md` fully, then run the `advance` skill
(`.agents/skills/advance/SKILL.md`) restricted to this item. Do not select
another item.

{suggested_first_slice_block}

## Host

You are on Linux x86-64. `mbx` is absent here; use `cargo` everywhere AGENTS.md
says `mbx`. `cargo nextest` is installed. Windows, macOS, and QEMU acceptance
is unavailable on this host and must be reported as unavailable, never as
passing.

## Collision rules

Edit only your item's board text. Never touch these excluded items: {exclusions}.
If your fix needs a path owned by another item in the wave, stop and report
`blocked` with the needed path.

Refetch `origin/main` before you `enqueue`. If the item's acceptance already
landed, or another author materially changed its text, stop and report
`superseded`.

## Landing

Publish only through `python3 tools/landing.py` with
`--owner "{owner_label}"`. Rebase onto current `origin/main` and validate
immediately before `enqueue`. After `claim`, rerun the checks whose inputs
changed. On lease expiry, `release` and rejoin; never bypass the reservation.
Land the first bounded improvement rather than accumulating a larger one.

## Budget

`max_acu_limit` is {max_acu_limit}. If you approach it without a landable
improvement, stop and put the witnessed failure, tested revision, command,
host, and next acceptance in the structured output. Board-only churn is not an
improvement.

## Commits and reasoning

Commit naming follows the AGENTS.md lanes (`lane: statement`). Preserve the
reasoning a next reader needs on the common reading path, per the advance
skill.

## Structured output

When you finish, for any result, fill this schema exactly:

```json
{structured_output_schema}
```

`result` is one of `landed`, `verification_only`, `blocked`, `superseded`,
`budget_exhausted`, `aborted`. `first_build_seconds` is the wall time of your
first workspace check in this session. `lease_expiries` counts landing-lease
expiries you hit. `unrelated_failures` lists confirmed-unrelated failures you
observed but did not repair.
