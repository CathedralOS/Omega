# Outcome claims

The claim registry shows which sessions are working on which task, so new
sessions can choose work knowing who owns what. `cargo xtask claims`
records that work on the branch `work/claims` (`refs/heads/work/claims`). A
claim lasts two hours from when it was taken or last renewed; ended claims drop
off the board.

The registry is a branch because the Git proxy in front of Claude Code cloud
sessions accepts pushes only to `refs/heads/*`; it refused every push to the
former `refs/coordination/omega-claims/main` (measured 2026-11-15), so claims
from those sessions never reached anyone else. The proxy also refuses forced
updates and deletions, so the tool only ever fast-forwards the branch. Never
force-push, delete or recreate `work/claims` by hand, and never open a pull
request from it; it holds registry commits, not source.

Claims are informational. Several sessions may claim the same item or overlapping
paths, and they reconcile changes at rebase. Read the task and current work
before choosing it; an unclaimed task may still lack its prerequisites. Publication follows [landing](landing.md).

## Inspect and claim

Run the Rust repository tool through `mbx` from the checkout:

```text
mbx xtask claims status
mbx xtask claims claim --board TASKS.md --item OUTCOME-ID --owner "Name / outcome" --path affected/subsystem --note "Acceptance; failure; next action; dependencies"
```

`status` prints JSON with `state`, `claims`, `notes_pending`, the branch
`version` and the `legacy_version` (see [migration](#migration-from-the-coordination-ref)). It omits a claim once it has ended: two hours after its `claimed_utc`
or latest `renewed_utc`, or earlier if its recorded `expires_utc` says so. A
recorded expiry can shorten a claim but never lengthen it. `state` is
`available` when no claims remain in that view, and `claimed` otherwise. This
is registry-wide, not per task. A claim with no readable time stays visible
because nothing shows it ended.
`notes_pending` counts the stored notes.

`claim` requires one `--item`. It accepts optional `--board`, `--owner`,
`--path`, and `--note`; the owner defaults to `agent`. Repeat `--path`, or separate
paths with commas or whitespace, to list several files or directories you
expect to edit. Paths do not grant or refuse access. A successful
claim prints its generated `ticket`; keep it for notes and release.

For compiler work, claim the board item and note the program showing its
current state, remaining acceptance, observed failure and next action. A
sample and the compiler repairs it needs share that claim. Claim board items,
not board-file paths, so boards stay shared.

## Lifetime and notes

A claim lasts two hours. Renew it after each push. If it lapsed, `renew` from
the worktree that took it restores it under the same ticket, item, owner and
paths, and lists any live claim whose paths now overlap; check what is in
flight there before editing.

```text
mbx xtask claims renew
mbx xtask claims note --ticket TICKET --text "Outcome; acceptance; failure; next action; dependencies"
mbx xtask claims release --ticket TICKET
```

`claim` records the published claim in the worktree's git directory
(`omega-claim-tickets`, never committed). `renew` with no `--ticket` renews or
restores every claim that worktree took; repeat `--ticket` to name claims
instead. A claim the worktree never recorded, or recorded only by ticket,
cannot be restored.
`note` requires one ticket and nonempty text, appends a UTC-stamped note and
renews that claim. A note lasts only as long as its claim, so it is a status
line, not a record: put findings and what the next session needs in the pull
request, the commits, `TASKS.md` or `OWNER_CODE_CONCERNS.md`. `release` removes the claim and forgets its ticket. Release
your claim when the original outcome completes; a checkpoint merge keeps its
owner and claim. If the session stops, first record the unfinished outcome and
handoff under [the law of completion](../../AGENTS.md#law-of-completion), then
release the lease. Expiry does not mean
the work finished or its owner stopped; check what is actually in flight
before taking it over, and never release another session's claim just for
overlap.

## Migration from the coordination ref

The first write while `work/claims` does not exist seeds it: the new commit's
parent is the current `refs/coordination/omega-claims/main` commit, and its
record starts from that record, so the history and the live claims carry over.
The record also lists the tickets it took over (`seeded_tickets`).

Agents still running an older tool keep writing the old reference. Until the
transition ends, `status` reads both. It shows a claim from the old reference
only when its ticket is neither on the branch nor in `seeded_tickets`, and
marks it with `"registry": "refs/coordination/omega-claims/main"`. A write from
this tool cannot renew, note or release such a claim; it lapses within two
hours, or its owner releases it with the old tool. A claim on both, taken over
at seeding and later released on the branch, stays gone. Writes never touch the
old reference.

The dual read stays until 2026-11-22 (one week after the switch) and until the
old reference has had no new commit for one two-hour lease. TASKS.md
`CLAIMS-LEGACY-READ-REMOVAL` then deletes `LEGACY_CLAIMS_REF`, `LegacyRead`,
`seeded_tickets` and this section, and makes the first write on an absent
branch start an empty registry.

## Stored history and concurrent writes

The wire format is still `omega-claims-v1`. Every write publishes the whole
record again and drops ended claims and the notes of any ticket without a
visible claim, so the stored record holds what status shows. A write keeps
fields it does not understand. Pruning does not rewrite the
reference's history: each earlier record is the message of an earlier commit,
so a dropped note can still be read with
`git fetch --depth=<n> origin work/claims` and
`git show -s --format=%B <commit>`. The branch continues the coordination
ref's history, so older records are its ancestors. The last record before
pruning began is commit `c5e5deebd226ac4eb3b35065eed81607c06d97f1`.

Help and malformed requests are answered before workspace discovery or Git
access. A help flag after the subcommand works too. A consumed value, as in
`claims note --ticket t --text --help`, stays ordinary text.

A valid command fetches only the registry commit the remote advertises
(`--depth=1`): each commit's message holds the whole record, and the history
has tens of thousands of them, so a full fetch could exhaust a cloud session's
memory. The history stays on the remote. Fetching that exact commit stops a
concurrent writer from changing which record the lease is based on. Only a
successful remote read that finds no registry reference starts an empty
registry. Remote read, fetch, commit-message and record-format failures exit 1;
they never report an empty registry or proceed to a write.

Claims and worktree fetches share an OS-owned lock in the repository's common
Git directory. Linked worktrees therefore update shared fetch and shallow metadata one at a
time; separate repositories do not wait. The lock is held
only for the fetch and released on return or process exit; the lock file
itself stays. The shared fetch operation disables automatic maintenance
and caps packing and indexing at one thread for that process and its Git
children. It never changes repository or global Git settings. Worktree network
access still happens before the pool-state lock is taken.
Direct Git commands and older xtask binaries ignore this lock.

A change reads the registry, creates a new Git commit whose parent is the tip
it read, and pushes it without force. The remote accepts the push only while
that tip is still current; a concurrent writer turns it into a
non-fast-forward, which Git refuses. The change is then reapplied to the newer
record on top of the new tip, up to five attempts. Exit 0 reports success, exit 2 reports an invalid
subcommand or a missing required claim/note/release argument, and reported clock
or Git failures exit 1. Claims are cooperative coordination, not
access control.

## Validation

`mbx test -p xtask claims::` runs frozen-clock unit tests without accessing the
remote registry. They check the two-hour lease and its renewal, the cap on
recorded expiries, calendar and offset boundaries, exact expiry filtering,
keeping claims with unreadable expiry, and pruning on write. Local temporary Git remotes
also exercise absent, unreadable and malformed registries, concurrent
reference updates, seeding the branch from the coordination ref, fast-forward
writes, and the merged status read, without contacting the shared registry.
