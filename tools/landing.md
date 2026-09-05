# Landing on main

Ready worktrees join one FIFO queue shared across machines. The first ticket
reserves final integration; its owner incorporates current main, checks locally,
and publishes. Publication removes that ticket atomically, allowing the next
worktree to reserve. Development and initial testing happen before joining.

`main` is the only shared code branch. Develop in isolated worktrees, including
detached worktrees. GitHub stores queue metadata at
`refs/coordination/omega-landing/main`; it runs no builds, waiting processes, or
model calls. There are no PRs, remote worker branches, or work-ownership records.

## Join when ready

PowerShell 7.2+ and authenticated Git push access are required. Run from your own
clean, checkpointed worktree. The command uses the remote's actual push URL for
all observations and updates, and refuses multiple push destinations.

```powershell
pwsh -NoProfile -File tools/landing.ps1 status

# Retain this ID in the task's handoff before enqueueing so a connection failure
# cannot lose your identity. Never reuse it after cancellation or publication.
$ticket = [guid]::NewGuid().ToString('N')
$joined = pwsh -NoProfile -File tools/landing.ps1 enqueue `
    -Ticket $ticket -Owner 'Jarod / pipeline'
if ($LASTEXITCODE -ne 0) { $joined; throw 'Inspect status before retrying this ticket.' }
$joined
```

The result includes your ticket and position. Order means the order of successful
shared enqueues, not workstation timestamps. Simultaneous arrivals receive a
single order; retries preserve existing entries. Retrying enqueue with the same
live ticket and owner does not add a duplicate. If no ticket is supplied, the
command generates one and returns it; save the result.

## Wait for your turn, then integrate

```powershell
# Wait up to 30 minutes in a local PowerShell process, checking every 10 seconds.
$reservationText = pwsh -NoProfile -File tools/landing.ps1 claim `
    -Ticket $ticket -WaitSeconds 1800
if ($LASTEXITCODE -eq 2) {
    $reservationText
    throw 'Still queued. Resume the wait later or cancel this ticket.'
}
if ($LASTEXITCODE -ne 0) { throw 'Inspect status; integration is not authorized.' }
$reservation = $reservationText | ConvertFrom-Json

# The command fetched this exact main commit for integration.
git rebase $reservation.base
if ($LASTEXITCODE -ne 0) { throw 'Resolve integration or release the reservation.' }
```

Omit `-WaitSeconds` for one immediate attempt. Exit 2 means an earlier ticket
remains; it does not cancel yours. `-PollSeconds` controls the local wait interval
(default 10). There is no AI polling loop or hosted scheduler. The next waiting
process claims automatically when it reaches the head; only that claim, not
queue position alone, authorizes final integration. A worktree without a running
waiter can resume with the same ticket later. An absent head is never skipped.

You can continue independent work while queued, but cancel if the queued change
needs more implementation. Do not concurrently edit or rebase a worktree whose
waiter may acquire its reservation. Read incoming main separately while working.

## Check locally, then publish

Capture the exact candidate before running the checks required by `AGENTS.md`
and the task. Keep the actual exits; all required checks must succeed and the
worktree must remain clean at that commit. The script verifies Git state, not
test success. Compiler advancement still requires its full baseline.

```powershell
$verified = git rev-parse HEAD
# Run the applicable gates here. Stop on failure, then release below.

pwsh -NoProfile -File tools/landing.ps1 publish `
    -Claim $reservation.claim -Base $reservation.base -Candidate $verified
if ($LASTEXITCODE -ne 0) { throw 'Inspect status and remote main before retrying.' }
```

One atomic push fast-forwards main and removes only the active ticket. Remaining
tickets keep their order. A concurrent enqueue retries just the queue update
using the same candidate; it neither drops the new ticket nor repeats the build.
The command rejects a dirty or different HEAD, the wrong reserved base, divergent
history, and merge commits. A rejected main push leaves the queue unchanged.

The coordination reference persists even when empty. Its empty-tree commits
retain queue history outside main, making removed tickets permanently invalid.
The conditional force option applies only to this reference; main is never
force-pushed. Recovery during publication invalidates the former owner's claim.

Every publisher must use this command. Direct main pushes can still bypass this
cooperative protocol. If main advances outside the reservation, publication
stops; release and reconcile before performing final validation again.

## Leave the queue or recover abandoned work

```powershell
# Waiting, but no longer ready:
pwsh -NoProfile -File tools/landing.ps1 cancel -Ticket $ticket

# Active, but a gate failed or more implementation is needed:
pwsh -NoProfile -File tools/landing.ps1 release -Claim $reservation.claim
```

Both operations remove your ticket. Rejoining later requires a new ticket at the
tail. No ticket or reservation expires automatically. After confirming with the
owner that work was abandoned, inspect status and remove that exact identity:

```powershell
pwsh -NoProfile -File tools/landing.ps1 status
# Abandoned waiting ticket:
pwsh -NoProfile -File tools/landing.ps1 cancel `
    -Ticket '<observed ticket>' -Reason 'Owner confirmed the session stopped'
# Abandoned active reservation:
pwsh -NoProfile -File tools/landing.ps1 recover `
    -Claim '<observed claim>' -Reason 'Owner confirmed the session stopped'
```

Cancel cannot remove an active reservation. Recovery requires a reason and
cannot remove a newer owner. An offline machine or slow build alone does not
establish abandonment. Owner labels identify peers; they are not authentication
boundaries between repository writers.

After a network error, inspect status and fetch remote main. If the verified
candidate is already an ancestor of main, it landed. Do not publish again or
release somebody else's claim based solely on the shell error. Queue mutations
retry bounded metadata contention internally; persistent failures remain visible.

## Upgrade and verification

An active v1 reservation remains valid: finish it using the previous command,
or explicitly release its full observed claim SHA. The new command reports
`legacy_reserved` and refuses FIFO mutations until that owner is done. Once FIFO
initializes, its persistent reference makes v1 acquisition fail closed. Both
machines should fetch the updated main and use this guide.

```powershell
pwsh -NoProfile -File tools/tests/landing.tests.ps1
```

Tests use temporary local repositories and independent processes. They cover
FIFO, concurrent arrivals, local waiting, cancellation, recovery, publication
races, server rejection, linear history, legacy upgrade, and distinct fetch/push
destinations. They do not contact GitHub or compile Omega.
