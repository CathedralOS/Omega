# Landing on main

Ready worktrees join one FIFO queue shared across machines. The first ticket
reserves final integration; its owner incorporates current main, checks locally,
and publishes. Each promoted head has exactly **three minutes (180 seconds)**,
starting when it becomes head, not when its owner claims. Publication or expiry
removes that ticket; the next head receives a fresh three-minute lease.
Development and initial testing happen before joining.

`main` is the only shared code branch. Develop in isolated worktrees, including
detached worktrees. GitHub stores queue metadata at
`refs/coordination/omega-landing/main`; it runs no builds, waiting processes, or
model calls. There are no PRs, remote worker branches, or work-ownership records.

## Join when ready

The helper uses **Python 3 and Git**, without third-party packages or a particular
shell. The same program runs on Windows, macOS, and Linux. Here `python` means
your Python 3 executable; use `python3` if that is its name on your machine.
No PowerShell installation is needed.

Run from your own clean, checkpointed worktree. The command uses the remote's
actual push URL for all observations and updates, refusing multiple destinations.
It prints JSON. Use `--repository <path>` for another worktree or `--remote <name>`
instead of `origin`. Angle-bracketed values below are placeholders for full IDs,
not shell syntax; replace them before running the commands.

```text
python tools/landing.py status
python -c "import uuid; print(uuid.uuid4().hex)"
python tools/landing.py enqueue --ticket <ticket> --owner "Jarod / pipeline"
```

Retain the generated ticket in your handoff before enqueueing, so a connection
failure cannot lose it. Never reuse it after cancellation or publication. If
`--ticket` is omitted, enqueue generates and returns one; save that result.
The result includes your ticket and position. Order is the order of successful
shared enqueues, not workstation timestamps. Simultaneous arrivals receive a
single order; retries preserve existing entries. Retrying a live ticket with the
same owner does not duplicate it.

## Wait for your turn, then integrate

```text
python tools/landing.py claim --ticket <ticket> --wait-seconds 1800
git rebase <returned-base>
```

Successful claim exits 0 and returns `claim`, `base`, and `expires_utc`; retain them. The command
fetched that exact base for integration. Omit `--wait-seconds` for one immediate
attempt. Exit 2 means an earlier ticket remains; it does not cancel yours. Resume
the local wait later or cancel. Other errors exit 1. `--poll-seconds` controls the
wait interval (default 10); waiting runs in the local Python process, not in an
AI polling loop or hosted scheduler.

The next waiter claims when it reaches the head; queue position alone does not
authorize integration. A waiting ticket can resume later, but an absent head
loses its turn at expiry. The next coordinating client atomically removes the
expired head before continuing. Do not concurrently edit or
rebase a worktree whose waiter may acquire its reservation. Read incoming main
separately while working; cancel if the queued change needs more implementation.

## Check locally, then publish

Capture the exact candidate before running the checks required by `AGENTS.md`
and the task. Keep the actual exits and keep the worktree clean at that commit.
The command checks Git state, not test success. Compiler advancement still
requires its full baseline.

Do preparation and initial checks before enqueueing. Finish integration and
publication within the remaining lease, or rejoin with a new ticket. The deadline
does not waive required validation. There is no renewal flag or heartbeat:
claim retries, enqueue retries, and edits behind the head do not reset its clock.

The owner permits documented, verified baseline failures for milestones. Record
the failing commands, unchanged baseline and candidate revisions, and the
comparison showing the failures predate the change. New or unexplained failures
require releasing and investigating. Do not suppress tests, rewrite expectations,
or call a red gate green.

```text
git rev-parse HEAD
python tools/landing.py publish --claim <claim> --base <base> --candidate <verified-sha>
```

One atomic push fast-forwards main and removes only the active ticket. Remaining
tickets keep their order. A concurrent enqueue retries just the queue update
using the same candidate; it neither drops the new ticket nor repeats the build.
The command rejects a dirty or different HEAD, the wrong reserved base, divergent
history, and merge commits. A rejected main push leaves the queue unchanged.

The coordination reference persists even when empty. Its empty-tree commits
retain history outside main, making removed tickets permanently invalid. The
conditional force option applies only to this reference; main is never
force-pushed. Recovery during publication invalidates the former owner's claim.

Every publisher must use this command. Direct main pushes can still bypass this
cooperative protocol. If main advances outside the reservation, publication stops;
release and reconcile before performing final validation again.

## Leave the queue or recover abandoned work

```text
python tools/landing.py cancel --ticket <waiting-ticket>
python tools/landing.py release --claim <active-claim>
```

Both remove your ticket. Rejoining requires a new ticket at the tail. A promoted
head expires automatically, including when it never claims. Waiting tickets have
no clock until promotion. To remove work before its deadline, confirm with its
owner, inspect status, and remove that exact identity:

```text
python tools/landing.py status
python tools/landing.py cancel --ticket <observed-ticket> --reason "Owner confirmed the session stopped"
python tools/landing.py recover --claim <observed-claim> --reason "Owner confirmed the session stopped"
```

Cancel cannot remove an active reservation. Early recovery requires a reason and
cannot remove a newer owner. Expiry needs no abandonment investigation. Owner
labels identify peers; they are not authentication boundaries between writers.

Queue records retain `head_promoted_utc` and `head_expires_utc`, exactly 180 seconds
apart; an empty queue has neither. Only changing the head creates new timestamps.
Keep host clocks synchronized. Clients check expiry before publication and again
immediately before invoking the push. Git's expected-reference check fences an
old publisher once another client promotes the successor. GitHub does not run a
clock-based hook: an already in-flight push has the normal uncertain-network
outcome and must be inspected, not blindly repeated.

After a network error, inspect status and fetch remote main. If the verified
candidate is already an ancestor of main, it landed. Do not publish again or
release somebody else's claim based solely on the shell error. Queue mutations
retry bounded metadata contention internally; persistent failures remain visible.

## Upgrade and verification

The timed queue uses `omega-landing-v3`. On the first mutation, a v2 queue is
upgraded without changing its order or active ticket; its current head receives
one three-minute lease starting at upgrade. A legacy v1 reservation can still be
explicitly released by its full claim SHA, or is converted into a timed head
retaining its owner and old claim as provenance. Old clients reject the new
format; fetch current main and use this command. No existing owner silently
retains an indefinite reservation after upgrade.

```text
python tools/tests/test_landing.py -v
```

Tests use temporary local repositories and independent processes. They cover
FIFO, concurrent arrivals, local waiting, cancellation, recovery, publication
races, expiry, nonrenewal, server rejection, linear history, legacy upgrade, and distinct fetch/push
destinations. They do not contact GitHub or compile Omega. Git invokes its normal
hook shell for the adversarial-hook controls; the helper itself invokes Git
directly, never through a shell.
