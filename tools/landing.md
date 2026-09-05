# Landing on main

Several machines can develop concurrently. The shared landing reservation keeps
one final integration from becoming stale while its local checks run. It starts
after implementation and focused testing, and ends when the verified commit is
published or the owner releases it to continue working.

`main` is the only shared code branch. Use isolated worktrees, including detached
worktrees, for development. There are no PRs, worker branches on the remote,
GitHub Actions jobs, model calls, or work-ownership records in this protocol.
GitHub stores one small reference, `refs/coordination/omega-landing/main`, whose
commit message identifies the owner, creation time, and unique claim attempt.
The record has an empty tree and never enters main's code history.

## Develop, then reserve

PowerShell 7.2+ and authenticated Git push access are required. Run these from
your own clean, checkpointed worktree. The script reads and writes the same
configured push URL; remotes with multiple push destinations are refused.

```powershell
pwsh -NoProfile -File tools/landing.ps1 status

$reservationText = pwsh -NoProfile -File tools/landing.ps1 claim -Owner 'Jarod / pipeline'
if ($LASTEXITCODE -ne 0) { $reservationText; throw 'Not reserved; inspect the command result.' }
$reservation = $reservationText | ConvertFrom-Json

# The returned base was observed and fetched after the reservation was acquired.
git rebase $reservation.base
if ($LASTEXITCODE -ne 0) { throw 'Resolve the integration or release the reservation.' }
```

Exit 2 means another owner holds the reservation. The JSON result identifies
that owner; continue useful local work rather than repeating full integration
checks. This is a reservation, not a FIFO scheduler. Do not run an AI polling
loop. Read current main separately while developing; do not rebase underneath
an active build or edit.

## Check locally, then publish

Run the checks required by `AGENTS.md` and the task against this integrated
candidate. Keep their actual exits and the exact verified SHA. The reservation
command verifies Git state, not test success; it grants no testing exemption.
For compiler advancement, the full baseline remains required.

```powershell
# Capture the candidate before checking it. The checks must all succeed and
# the working tree must remain clean at this exact commit.
$verified = git rev-parse HEAD

# Run the applicable gates here; stop on failure. Then:
pwsh -NoProfile -File tools/landing.ps1 publish `
    -Claim $reservation.claim -Base $reservation.base -Candidate $verified
if ($LASTEXITCODE -ne 0) { throw 'Inspect the remote outcome before any retry.' }
```

Publication checks clean HEAD, ancestry from the reserved base, and absence of
merge commits. One atomic push fast-forwards main and deletes the exact owned
coordination reference. The conditional force option applies only to that
reference; main is never force-pushed. Recovery or replacement of the claim
therefore prevents its former owner from publishing, even if the owner resumes
after a long pause. A rejected main update also leaves the claim intact.

Every writer must use this protocol. An ordinary direct push can bypass the
reservation; this is cooperative coordination, not a server access-control
rule. If that happens, the wrapper refuses a changed base. Release, inspect the
new work, and coordinate before another full integration attempt.

## Release and recovery

If a gate fails or more implementation is needed, release without changing main:

```powershell
pwsh -NoProfile -File tools/landing.ps1 release -Claim $reservation.claim
```

There is no automatic expiry. A slow build, offline machine, or missing heartbeat
does not establish that the owner has stopped. After checking with the owner,
inspect the current claim and explicitly recover that exact object ID:

```powershell
pwsh -NoProfile -File tools/landing.ps1 status
pwsh -NoProfile -File tools/landing.ps1 recover `
    -Claim '<full observed claim SHA>' -Reason 'Owner confirmed the session stopped'
```

Release and recovery compare the remote claim at the moment of update. They
cannot delete a newer replacement. Recovery needs no continuously running
coordinator; either machine can perform it with repository write access.

After a connection failure, do not assume a push failed to reach GitHub. Read
status and fetch main before retrying. If the verified candidate is already an
ancestor of remote main, it landed. Never release somebody else's newer claim
or repeat publication based only on the shell's network error.

## Verify the implementation

```powershell
pwsh -NoProfile -File tools/tests/landing.tests.ps1
```

Tests use temporary local repositories and independent writer processes. They
exercise competing claims, recovery, a replacement between preflight and push,
atomic rollback after server rejection, linear history, dirty candidates, and
an uncoordinated writer. They do not contact GitHub or compile Omega.

The mechanism uses Git's explicit expected-reference form of
[`--force-with-lease` and `--atomic`](https://git-scm.com/docs/git-push).
