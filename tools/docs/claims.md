# Outcome claims

The claim registry shows which sessions are working on which task, so new
sessions can choose work knowing who owns what. `cargo xtask claims`
records that work in `refs/coordination/omega-claims/main`. A claim lasts two
hours from when it was taken or last renewed; ended claims drop off the board.

Claims are informational. Several sessions may claim the same item or overlapping
paths, and they reconcile changes at rebase. Read the task and current work
before choosing it; an unclaimed task may still lack its prerequisites. Publication follows [landing](landing.md).

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
line, not a record: put findings and what the next session needs in the pull
request, the commits, `TASKS.md` or `OWNER_CODE_CONCERNS.md`. `release` removes the claim and forgets its ticket. Release
your claim when the work completes or the session stops. Expiry does not mean
the work finished or its owner stopped; check what is actually in flight
before taking it over, and never release another session's claim just for
overlap.

## Stored history and concurrent writes

The wire format is still `omega-claims-v1`. Every write publishes the whole
record again and drops ended claims and the notes of any ticket without a
visible claim, so the stored record holds what status shows. A write keeps
fields it does not understand. Pruning does not rewrite the
reference's history: each earlier record is the message of an earlier commit,
so a dropped note can still be read with
`git fetch --depth=<n> origin refs/coordination/omega-claims/main` and
`git show -s --format=%B <commit>`. The last record before pruning began is
commit `c5e5deebd226ac4eb3b35065eed81607c06d97f1`.

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

A change reads the registry, creates a new Git commit holding the updated
record, and pushes it with a compare-and-swap lease on the reference. If a
concurrent writer wins, the change is reapplied to the newer record, up to
five attempts. Exit 0 reports success, exit 2 reports an invalid
subcommand or a missing required claim/note/release argument, and reported clock
or Git failures exit 1. Claims are cooperative coordination, not
access control.

## Validation

`mbx test -p xtask claims::` runs frozen-clock unit tests without accessing the
remote registry. They check the two-hour lease and its renewal, the cap on
recorded expiries, calendar and offset boundaries, exact expiry filtering,
keeping claims with unreadable expiry, and pruning on write. Local temporary Git remotes
also exercise absent, unreadable and malformed registries and concurrent
reference updates without contacting the shared registry.
