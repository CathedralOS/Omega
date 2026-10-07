# Outcome claims

The registry at `refs/coordination/omega-claims/main` records the DRI (directly
responsible individual) for an outcome, not exclusive files. Path overlap is
integration information and never blocks a different outcome. Publication uses
[landing](landing.md).

## Inspect and claim

```text
python tools/claims.py status
python tools/claims.py available --board TASKS.md
python tools/claims.py claim --board TASKS.md --item OUTCOME-ID --owner "Name / outcome" --path affected/subsystem --note "Acceptance; failure; next action; dependencies"
```

Use Python 3 (`python3` where appropriate), on Windows, macOS or Linux.
`--repository` and `--remote` select another checkout/remote. Availability
means unowned, not prerequisite-free: read the task and its parked status.

- Exit 0 registers the claim. `overlaps` names peers to notify and reconcile.
- Exit 2 means the same outcome has a DRI. Contribute under that owner, arrange
  a handoff, or select a distinct outcome. Do not invent an alias to evade it.
- Exit 3 means coordination is unavailable. Record the scope locally and
  continue isolated work; do not repeatedly wait or claim registration succeeded.

A handoff records the frontier and successor, releases the old ticket, then the
successor claims. A checkpoint is not a handoff. Delegation retains an integration
DRI; contributors use specifically delegated sub-outcomes, not competing parent claims.

`--path` accepts repeated, comma/whitespace-separated files or directories.
These are estimates, not edit permissions. Necessary scope can expand without
releasing ownership. Boards remain shared: claim their items, not board-file paths.

## Lifetime and evidence

Default leases last eight hours; `--lease-minutes` accepts 15–1440.

```text
python tools/claims.py renew --ticket TICKET
python tools/claims.py note --ticket TICKET --text "Outcome; acceptance; failure; next action; dependencies"
python tools/claims.py release --ticket TICKET
python tools/claims.py recover --ticket TICKET --reason "Owner confirmed stopped; handoff recorded"
```

Renew while working. Release after completion or explicit handoff/stop, not
every commit. Expiry is not completion. Preserve abandoned work before recovery;
never remove a peer claim merely for file overlap.

Notes survive release. `notes` lists pending entries; `--ticket` (unique
prefix), `--owner`, and `--all` filter them. Coordinators incorporate only
justified remaining work, then `sweep --owner <wave>`. Notes are not another
backlog; board conflicts are reconciled, not union-merged.

`audit --worktree PATH --owner LABEL --base origin/main` reports changes,
uncovered estimates and all peer `overlaps`, including shared estimates.
It exits 0 for overlaps; command/network errors still fail.

## Compatible rollout

The wire format remains `omega-claims-v1`. Existing tickets, notes, leases,
renewals and releases remain valid. No mass cancellation or reassignment.
Historical duplicate owners remain readable; agree one DRI at handoff.

`--allow-overlap` remains accepted for old launchers. It is unnecessary for
paths and cannot replace an outcome owner. Refresh old clients before selecting
new work; old clients may still reject path overlap, but existing tickets can
be renewed/released without conversion.

Deprecated JSON fields `off_board_fences` and audit `violations` remain
empty for existing consumers. Directory estimates appear in `off_board_paths`;
audit collisions appear in `overlaps`.

Commands have a 15-second total coordination budget; `--timeout-seconds`
before the subcommand permits up to 300 seconds. Timed-out claims save
`build/claims-pending/<ticket>.json`. Inspect status before retrying an
uncertain write, retaining its ticket. Removed tickets cannot be resurrected.

Snapshots retain verified tips under local `refs/coordination-cache/` refs,
scoped by remote URL and protocol ref. This keeps Git from repeatedly walking
the independent claims history. These refs are advisory fetch hints; a local
cache lock collision does not reject a successful read or change remote claims.

Mutations compare-and-swap the Git reference: distinct concurrent outcomes
survive, while a same-outcome race admits one owner. Expired claims are reaped
by the next writer. This serializes registration, not development.

## Validation

`python tools/tests/test_claims.py -v` uses temporary local repositories,
not GitHub. It exercises ownership races, advisory overlap, legacy records/flags,
ticket lifecycle, evidence notes and coordination failure.
