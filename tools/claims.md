# Outcome claims

The claim registry shows which sessions are currently working on an outcome so
new sessions can choose work with useful ownership context. `cargo xtask claims`
records that work in `refs/coordination/omega-claims/main`; status hides expired
claims while retaining the stored history for reconciliation.

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
`version`. It omits a claim only when its readable `expires_utc` is at or before
the current clock. `state` is `available` when no claims remain in that view,
and `claimed` otherwise. This is registry-wide status, not an item-by-item task
selector. Missing or malformed expiry stays visible because it cannot establish
that ownership expired. Notes are counted as stored; status does not consume or
prune them.

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

New claims record the actual UTC creation time and expire eight hours later.
The current Rust command has no lease override or renewal command. If work
continues after expiry, register a new claim and link the previous ticket in its
note so the continuation remains discoverable.

```text
mbx xtask claims note --ticket TICKET --text "Outcome; acceptance; failure; next action; dependencies"
mbx xtask claims release --ticket TICKET
```

`note` requires one ticket and nonempty text. It appends a UTC-stamped note to
the registry; it does not change the claim's expiry. `release` requires one
ticket and removes that ticket's claim from the latest snapshot. Notes remain
stored after release. Release your claim when the work completes or the session
stops, preserving any remaining frontier in a note. Expiry is not evidence that
the work finished or its owner stopped; reconcile actual in-flight work before
taking it over, and do not release another session's claim merely for overlap.

The supported commands are `status`, `claim`, `note`, and `release`. The removed
Python client's `available`, `renew`, `reserve`, `recover`, `notes`, `sweep`,
and `audit` commands and its batch, filtering, and timeout flags are not
implemented by this Rust command. Do not rely on them for dispatch or recovery.

## Stored history and concurrent writes

The wire format remains `omega-claims-v1`. Status filtering changes only its
output: expired records and notes are not rewritten, released, or reassigned.
Existing claim and note fields remain intact. Historical future expiries,
including `2099-01-01`, retain their recorded meaning; the command does not infer
a replacement expiry from `claimed_utc` or `lease_minutes`.

Every command fetches only the newest registry commit (`--depth=1`): each
commit's message holds the whole record, and the reference's history runs to
tens of thousands of them, so a full fetch could exhaust a cloud session's
memory. The history stays on the remote.

Mutations read the registry snapshot, create a new Git commit containing the
updated record, and push with a compare-and-swap lease on its reference. A
concurrent writer or network error can reject the push; inspect status before
retrying an uncertain write. Exit 0 reports success, exit 2 reports an invalid
subcommand or a missing required claim/note/release argument, and reported clock
or Git write failures exit 1. Claims remain cooperative coordination rather
than access control.

## Validation

`mbx test -p xtask claims::` runs frozen-clock unit tests without accessing the
remote registry. They check creation and eight-hour expiry, calendar and offset
boundaries, exact expiry filtering, malformed expiry retention, and preservation
of historical records.
