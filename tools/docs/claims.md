# Outcome claims

The claim registry shows which sessions are currently working on an outcome so
new sessions can choose work with useful ownership context. `cargo xtask claims`
records that work in `refs/coordination/omega-claims/main`. A claim lasts two
hours from when it was taken or last renewed; ended claims drop off the board.

Claims are informational. Several sessions may claim the same item or overlapping
paths, and they reconcile changes at rebase. Read the task and current work
before selecting it; registry availability does not establish that its
prerequisites are met. Publication follows [landing](landing.md).

## Inspect and claim

Run the Rust repository tool through `mbx` from the checkout:

```text
mbx xtask claims status
mbx xtask claims claim --board TASKS.md --item OUTCOME-ID --owner "Name / outcome" --path affected/subsystem --note "Acceptance; failure; next action; dependencies"
```

`status` prints JSON with `state`, `claims`, `notes_pending`, and the registry
`version`. It omits a claim once it has ended: two hours after its `claimed_utc`
or latest `renewed_utc`, or earlier if its recorded `expires_utc` says so. A
recorded expiry can shorten a claim but never lengthen it. `state` is
`available` when no claims remain in that view, and `claimed` otherwise. This
is registry-wide status, not an item-by-item task selector. A claim with no
readable time stays visible because nothing establishes that it ended.
`notes_pending` counts the stored notes.

`claim` requires one `--item`. It accepts optional `--board`, `--owner`,
`--path`, and `--note`; the owner defaults to `agent`. Repeat `--path`, or use
comma/whitespace-separated paths, for multiple estimated files or directories.
Paths describe expected edits and do not grant or refuse access. A successful
claim prints its generated `ticket`; retain it for notes and release.

For capability work, claim the board item and record its current witness,
remaining acceptance, observed failure, and next action in the note. A sample
and the compiler repairs it needs stay under that outcome claim. Keep boards
shared by claiming their items rather than board-file paths.

## Lifetime and notes

A claim lasts two hours. Renew it after each push; a lapsed claim cannot be
renewed, because someone else may have taken the work, so claim again.

```text
mbx xtask claims renew
mbx xtask claims note --ticket TICKET --text "Outcome; acceptance; failure; next action; dependencies"
mbx xtask claims release --ticket TICKET
```

`claim` records its ticket in the worktree's git directory
(`omega-claim-tickets`, never committed). `renew` with no `--ticket` renews
every claim that worktree took; repeat `--ticket` to name claims instead.
`note` requires one ticket and nonempty text, appends a UTC-stamped note and
renews that claim. A note lasts only as long as its claim, so it is a status
line, not a record: put findings, custody and continuation state in the pull
request, the commits, `TASKS.md` or `OWNER_CODE_CONCERNS.md`. `release` removes the claim and forgets its ticket. Release
your claim when the work completes or the session stops. Expiry is not evidence
that the work finished or its owner stopped; reconcile actual in-flight work
before taking it over, and do not release another session's claim merely for
overlap.

## Stored history and concurrent writes

The wire format remains `omega-claims-v1`. Every write republishes the whole
record, so a write also drops ended claims and the notes of any ticket without a
visible claim. The stored record therefore holds what status shows. Fields a
write does not understand stay intact. Pruning does not rewrite the
reference's history: each earlier record is the message of an earlier commit,
so a dropped note can still be read with
`git fetch --depth=<n> origin refs/coordination/omega-claims/main` and
`git show -s --format=%B <commit>`. The last record before pruning began is
commit `c5e5deebd226ac4eb3b35065eed81607c06d97f1`.

Help and malformed requests are handled before workspace discovery or Git access.
A help flag after the subcommand is supported too. A consumed value, such as
`claims note --ticket t --text --help`, remains ordinary text.

A formed command fetches only the advertised registry commit (`--depth=1`): each
commit's message holds the whole record, and the reference's history runs to
tens of thousands of them, so a full fetch could exhaust a cloud session's
memory. The history stays on the remote. The fetch requests the exact advertised
commit so a concurrent writer cannot change which record belongs to the lease.
Only a successful remote read with no registry reference initializes an empty
registry. Remote read, fetch, commit-message and record-format failures exit 1;
they never report an empty registry or proceed to a write.

Claims and worktree fetches share an OS-owned lock in the repository's common
Git directory. Linked worktrees therefore serialize updates to shared fetch and
shallow metadata, while independent repositories proceed separately. Ownership
lasts only for the fetch and ends on return or process exit; the persistent lock
file stays in place. The shared fetch operation disables automatic maintenance
and caps packing and indexing at one thread for that process and its Git
children. It never changes repository or global Git settings. Worktree network
access still precedes the pool-state lock.
Direct Git commands and older xtask binaries do not participate in this protocol.

Mutations read the registry snapshot, create a new Git commit containing the
updated record, and push with a compare-and-swap lease on its reference. A
push that loses to a concurrent writer is reapplied to the newer snapshot, up to
five attempts. Exit 0 reports success, exit 2 reports an invalid
subcommand or a missing required claim/note/release argument, and reported clock
or Git failures exit 1. Claims remain cooperative coordination rather
than access control.

## Validation

`mbx test -p xtask claims::` runs frozen-clock unit tests without accessing the
remote registry. They check the two-hour lease and its renewal, the cap on
recorded expiries, calendar and offset boundaries, exact expiry filtering,
unreadable expiry retention, and pruning on write. Local temporary Git remotes
also exercise absent, unreadable and malformed registries and concurrent
reference updates without contacting the shared registry.
