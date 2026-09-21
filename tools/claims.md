# Work claims

Machines, waves, and local sessions share one registry of in-progress work at
`refs/coordination/omega-claims/main`. A claim names its owner, a board item or
freeform label, the repository paths it expects to edit, and a renewable lease.
It answers "what is already being worked on" before anyone starts, so a new
agent picks unclaimed work instead of duplicating an active assignment.

The registry is an advisory fence, not a lock. Claiming an item another live
claim holds, or a path overlapping another claim's paths, is rejected so the
coordinator can partition explicitly. `--allow-overlap` records an informed
exception in the result; contested merges still happen and remain the owners'
problem. Publication stays with `tools/landing.py`; claims never touch `main`.

The helper uses **Python 3 and Git** like `landing.py`; `python` below means
your Python 3 executable (`python3` on macOS/Linux). Every command prints one
JSON object. Use `--repository <path>` for another worktree and `--remote
<name>` for a remote other than `origin`.

## See what is live

```text
python tools/claims.py status
python tools/claims.py available --board TASKS.md
```

`status` lists live claims and recently expired ones (kept for visibility until
the next writer reaps them). `available` lists a board's items that no live
claim holds — the supported boards are `TASKS.md`, `TASKS_BOOTSTRAP.md`, and
`TASKS_OPTIMIZER.md`.

## Claim before editing

```text
python tools/claims.py claim --board TASKS.md --item TERMINATION-RANKING-CHECKS \
    --owner "Jarod / windows" --path omega-rust/psi/pipeline/typed-trees-to-checked-trees/src/checks/termination
```

Retain the returned ticket. `--board` validates the item's `**<item>.**`
marker; omit it for work not on a board. `--path` repeats; paths are
normalized to repository-relative form (Windows-style separators accepted).
The default lease is 8 hours; `--lease-minutes` accepts 15..1440.

Exit 2 means a live conflicting claim exists; the JSON lists the owner, item,
and either the shared item or the overlapping paths. Stop and coordinate —
that output is a real assignment, not a transient error. Retrying a live
ticket you already hold with the same owner is idempotent.

## Renew, release, recover

```text
python tools/claims.py renew --ticket <ticket> [--lease-minutes <n>]
python tools/claims.py release --ticket <ticket>
python tools/claims.py recover --ticket <ticket> --reason "Owner confirmed stopped"
```

Renew any time before expiry. Release when finished for any reason — landed,
blocked, or abandoned. An expired claim never blocks reuse: the next writer
reaps it automatically. Recovery removes someone else's claim early and
requires a reason after checking with its owner; owner labels identify peers
but are not authentication.

## Evidence notes

```text
python tools/claims.py note --ticket <ticket> --text "<finding>"
python tools/claims.py notes
python tools/claims.py sweep
```

Workers attach durable evidence to a live claim instead of committing board
files: `note` records `text` (1–2000 characters) under the claim's ticket,
item, and owner, and requires a live claim. Notes survive claim release, so a
finding like "already resolved upstream, verified at `<sha>`" reaches the
coordinator without a board-only commit on `main`. `notes` lists pending
(unswept) entries; `status` reports their count as `notes_pending`. The
coordinator reads them at drain, writes the board updates they justify in its
own sweep commit, then marks them consumed with `sweep`. Swept notes stay in
the record for audit, bounded to the most recent hundred; the ledger is not a
second task board — it carries findings, not assignments.

## Consistency model

Every mutation commits the whole document as an empty-tree commit child and
pushes with an exact expected reference, like the landing queue. Stale writers
are fenced; concurrent claimants retry onto the newest document and keep both
arrivals. Removed tickets stay invalid through the retained history. Clock
skew only shifts apparent lease times; keep host clocks synchronized.

```text
python tools/tests/test_claims.py -v
```

Tests use temporary local repositories and independent processes. They cover
claim/release, item and path conflicts, idempotent retry, renewal, expiry and
reaping, recovery, concurrent arrivals, board item validation, and unknown
formats. They do not contact GitHub.
