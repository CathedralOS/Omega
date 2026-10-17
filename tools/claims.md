# Outcome claims

The registry at `refs/coordination/omega-claims/main` records which sessions
are working on which outcomes, so overlapping work is visible. A claim never
grants or refuses work: two sessions may claim the same item or overlapping
paths, and they reconcile their changes at rebase. Publication uses
[landing](landing.md).

## Inspect and claim

```text
python tools/claims.py status
python tools/claims.py available --board TASKS.md
python tools/claims.py claim --board TASKS.md --item OUTCOME-ID --owner "Name / outcome" --path affected/subsystem --note "Acceptance; failure; next action; dependencies"
```

Use Python 3 (`python3` where appropriate), on Windows, macOS or Linux.
`--repository` and `--remote` select another checkout/remote. `available`
lists items nobody has claimed; that is not proof their prerequisites are met,
so read the task and its parked status.

- Exit 0 registers the claim. `overlaps` names other live claims on the same
  item (`same item`) or on overlapping paths; read their notes and reconcile
  at rebase.
- Exit 3 means coordination is unavailable. Record the scope locally and
  continue isolated work; do not repeatedly wait or claim registration succeeded.

`--path` accepts repeated, comma/whitespace-separated files or directories.
These are estimates, not edit permissions; necessary scope can grow without a
new claim. Boards remain shared: claim their items, not board-file paths. A
sample and the compiler repairs it needs are one piece of work under one claim.

For capability work, claim the board item and keep its current witness,
requirement removed and remaining acceptance in the note. Explicit sample-only
assignments may still claim the sample path. Existing sample tickets stay valid:
record parent context in their notes rather than replacing tickets mid-repair.
Keep the capability claim across its landed witnesses while actively continuing;
release when its acceptance is complete or the session stops, preserving the
remaining frontier. No registry schema change is needed.

`python3 tools/sample_queue.py` reads one live registry snapshot while listing
failing samples. Unclaimed programs come first; occupied samples show their
owners, tickets and notes. A failed registry read labels ownership unknown in
both text and JSON. This is selection guidance, not a fence on shared paths or
an atomic reservation: inspect the claim registration's overlaps before starting
a newly selected sample. Named outcomes, continuations and known main regressions
retain priority.

## Lifetime and evidence

Default leases last eight hours; `--lease-minutes` accepts 15–1440.

```text
python tools/claims.py renew --ticket TICKET
python tools/claims.py note --ticket TICKET --text "Outcome; acceptance; failure; next action; dependencies"
python tools/claims.py release --ticket TICKET
python tools/claims.py recover --ticket TICKET --reason "Session confirmed stopped"
```

Renew while working. Release when the work lands or the session stops, not
every commit. Expiry is not completion. Preserve abandoned work before
recovery; never remove another session's claim merely for overlap.

Notes survive release for a while. Every note on a live claim stays; a
released or expired ticket keeps only its newest note, for three days, after
which a continuation reads the sample README instead. Every write applies this
bound: the whole registry is one record rewritten on each claim, renew and
note, and unswept notes once grew it to 6.4 MB, past the coordination timeout.
`notes` lists pending entries; `--ticket` (unique prefix), `--owner`, and
`--all` filter them. `sweep --owner <wave>` marks notes consumed. Notes are not
another backlog; board conflicts are reconciled, not union-merged.

`audit --worktree PATH --owner LABEL --base origin/main` reports changes,
uncovered estimates and all peer `overlaps`, including shared estimates.
It exits 0 for overlaps; command/network errors still fail.

## Compatible rollout

The wire format remains `omega-claims-v1`. Existing tickets, notes, leases,
renewals and releases remain valid. No mass cancellation or reassignment.
Several live claims on one item are ordinary records.

`--allow-overlap` remains accepted for old launchers and changes nothing.
Old clients may still refuse a second claim on an item or reject path
overlap; refresh them. Existing tickets can be renewed and released without
conversion.

Deprecated JSON fields `off_board_fences` and audit `violations` remain
empty for existing consumers. Directory estimates appear in `off_board_paths`;
audit collisions appear in `overlaps`.

Commands whose observed registry tip is present locally have a 15-second total
coordination budget. A missing tip uses a 120-second total acquisition budget
and reports that acquisition on stderr. This covers both the first registry
fetch and catching up with concurrent writers; a cache alone does not mean the
newest snapshot is available. The complete history can establish or refresh its
cache instead of repeatedly aborting the same download. Network stall bounds
still apply. Registry fetches use two pack workers and a 96 MiB delta-base cache
to bound memory used by large snapshot histories. Locally present snapshots
retain the 15-second limit. `--timeout-seconds` before the subcommand sets an
explicit total budget up to 300 seconds, including acquisition; it is never
extended.
Timed-out claims save
`build/claims-pending/<ticket>.json`. Inspect status before retrying an
uncertain write, retaining its ticket. Removed tickets cannot be resurrected.

Snapshots retain verified tips under local `refs/coordination-cache/` refs,
scoped by remote URL and protocol ref. This keeps Git from repeatedly walking
the independent claims history. Fetch negotiation explicitly offers that
registry's cached tip instead of competing with unrelated project histories.
These refs are advisory fetch hints; a local
cache lock collision does not reject a successful read or change remote claims.
Coordination fetches disable submodule recursion: registry and landing objects
belong to this repository, and reading them must not walk or fetch application
submodules. This is a per-command setting; ordinary project fetches retain their
configured submodule behavior.

Mutations compare-and-swap the Git reference, so concurrent claims all
survive, including two on the same item. Expired claims are reaped by the
next writer. This serializes registration, not development.

## Validation

`python tools/tests/test_claims.py -v` uses temporary local repositories,
not GitHub. It exercises concurrent claims, advisory overlap, legacy records/flags,
ticket lifecycle, evidence notes and coordination failure.
