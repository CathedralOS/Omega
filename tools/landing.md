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

Successful claim exits 0 and returns `claim` and `base`; retain both. The command
fetched that exact base for integration. Omit `--wait-seconds` for one immediate
attempt. Exit 2 means an earlier ticket remains; it does not cancel yours. Resume
the local wait later or cancel. Other errors exit 1. `--poll-seconds` controls the
wait interval (default 10); waiting runs in the local Python process, not in an
AI polling loop or hosted scheduler.

The next waiter claims when it reaches the head; queue position alone does not
authorize integration. A worktree without a running waiter can resume with the
same ticket later. An absent head is never skipped. Do not concurrently edit or
rebase a worktree whose waiter may acquire its reservation. Read incoming main
separately while working; cancel if the queued change needs more implementation.

## Check locally, then publish

Capture the exact candidate before running the checks required by `AGENTS.md`
and the task. Keep the actual exits and keep the worktree clean at that commit.
The command checks Git state, not test success. Compiler advancement still
requires its full baseline.

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

Both remove your ticket. Rejoining requires a new ticket at the tail. No ticket
or reservation expires automatically. After confirming abandonment with the owner,
inspect status and remove that exact identity:

```text
python tools/landing.py status
python tools/landing.py cancel --ticket <observed-ticket> --reason "Owner confirmed the session stopped"
python tools/landing.py recover --claim <observed-claim> --reason "Owner confirmed the session stopped"
```

Cancel cannot remove an active reservation. Recovery requires a reason and cannot
remove a newer owner. An offline machine or slow build alone does not establish
abandonment. Owner labels identify peers; they are not authentication boundaries
between repository writers.

After a network error, inspect status and fetch remote main. If the verified
candidate is already an ancestor of main, it landed. Do not publish again or
release somebody else's claim based solely on the shell error. Queue mutations
retry bounded metadata contention internally; persistent failures remain visible.

## Upgrade and verification

The `omega-landing-v2` format is unchanged by the Python port. Existing FIFO
tickets remain valid. An active v1 reservation also remains valid: finish it with
its original client or explicitly release its full observed claim SHA. This
command reports `legacy_reserved` and refuses FIFO mutations until that owner is
done. Once FIFO initializes, its persistent reference makes v1 acquisition fail
closed. Fetch current main before using the command.

```text
python tools/tests/test_landing.py -v
```

Tests use temporary local repositories and independent processes. They cover
FIFO, concurrent arrivals, local waiting, cancellation, recovery, publication
races, server rejection, linear history, legacy upgrade, and distinct fetch/push
destinations. They do not contact GitHub or compile Omega. Git invokes its normal
hook shell for the adversarial-hook controls; the helper itself invokes Git
directly, never through a shell.
