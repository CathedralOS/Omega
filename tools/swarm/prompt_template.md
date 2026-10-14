# Swarm wave {wave} session: {name}

You are one session of a coordinated wave. {customer_block}

First complete the mandatory main preflight in `AGENTS.md`: fetch main
successfully, update clean main or create an isolated checkout from its exact
fetched commit, then reread current instructions. A failed fetch stops new work;
cached main is not a fallback. Preserve unpublished work in a resumed checkout.

Read `AGENTS.md` fully, then run the `advance` skill
(`.agents/skills/advance/SKILL.md`) restricted to this customer. Do not select
other work.

## Finish the whole path

Apply the law of completion in `AGENTS.md`. Your superhuman advantage is
carrying the entire difficult task end to end. Difficulty calls for
investigation and persistence, never an invented earlier finish line.
Retain ownership and context through complete acceptance, required independent
review and authorized landing verified on remote main. Finish the necessary
compiler, library and prerequisite repairs in this worktree; do not wait for
another agent to implement them. Checkpoints and artifact handoffs preserve
evidence, not completion. Do not publish half-finished functionality.
Genuine external authority or environment blockers leave the outcome unfinished;
retain ownership and continue feasible work without bypassing any gate.
An explicitly read-only probe remains read-only.

Follow the river to its end; do not carve every possible branch along the way.
Working paths reveal the design. Build and validate what this journey needs;
adjacent journeys will reveal their own needs when undertaken.

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

The claim records what you are doing so other sessions can see it; it grants
nothing and blocks nothing. Another claim on the same customer or on overlapping
paths is information: read its note, continue in this worktree and reconcile
at rebase. There are no handoffs; you finish this work. Renew before expiry.
A checkpoint does not end the work.

Keep a short record in the claim note (`python3 tools/claims.py note --ticket
<ticket> --text "<finding>"`): sample, acceptance, current failure, next
necessary action and prerequisites. Reread it after compaction. A new obstacle
belongs to this work: fix it rather than filing a task for someone else. When
the sample is too large to trace, cut a smaller sample that isolates the failing
construct, make it run and land it, then return to this one.

Exit 3 means coordination timed out, not a live conflict. Keep the pending
claim record, report the outage, and continue the assigned work in isolation.
Do not repeatedly wait for registration. If a write timed out, inspect status
before retrying; its outcome may be uncertain. Landing remains mandatory.

## Shared files

A sample that still fails keeps its current failure in its own README. If a
`TASKS.md` item names this sample or its root cause, update that item in the
same commit as the repair: subtract what now works, delete it when its
acceptance passes. `landing.py` refuses candidates that touch only
`TASKS*.md`, `OWNER_QUESTIONS.md` or `tools/swarm/waves/` or change nothing,
so never land a board-only or empty commit. A missing owner decision goes to
`OWNER_QUESTIONS.md` with the exact question, referenced from the sample README,
in the commit that records the frontier. Never touch these excluded items:
{exclusions}.
Required edits may span other sessions' paths and multiple stages. Reconcile
shared repairs rather than adding a competing mode or dropping incoming work.
Refetch main before landing. If the sample already runs, verify it and report
superseded.

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
When a rebase or merge conflicts, first ask which abstraction both sides are
trying to express. If one general operation covers both, implement that and
delete the superseded path in the same commit, as AGENTS.md requires for
touched operations. If no single abstraction covers both, name in the commit
body the two cases it cannot unify. A conflict-free merge can still break the
build or a sibling outcome, so rebuild every commit you carry after resolving.
Give `publish` the model that produced the change: `--provenance "agent=<tool> model=<name> effort=<level>"`
(or export `OMEGA_PROVENANCE` once). Commit messages carry no attribution; this note is the only record.
Carry the sample through complete producer and consumer repairs. Rerun its
`omega run` after relevant changes; helper tests alone do not complete the
work or justify claiming native execution.

## Budget

`max_acu_limit` is {max_acu_limit}. If it is exhausted, preserve the witnessed
failure, tested revision, command, host and next acceptance in the claim note
and the sample README for whoever continues it. Report `budget_exhausted`, not completed work; do not land a
partial improvement to fit the budget. Board-only churn is not an improvement.

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

- `landed`: the sample runs with its oracle on remote main (or the board item's
  full acceptance is met), required review is satisfied, not merely a landed
  slice.

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
