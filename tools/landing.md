# Landing on main

The [GitHub merge queue pilot](ci/README.md) is isolated to `integration-pilot`.
Its explicit branch/PR exception does not change this main publication contract.

Ready worktrees join one FIFO queue shared across machines. The first ticket
reserves final integration; its owner incorporates current main, checks locally,
and publishes. Each promoted head has exactly **three minutes (180 seconds)**,
starting when it becomes head, not when its owner claims. Publication or expiry
removes that ticket; the next head receives a fresh three-minute lease.
Development and initial testing happen before joining.

For an active [pinned qualification](qualification.md), finish the coherent
run first, integrate current main near publication outside this queue, and
assess/requalify affected inputs there. Ordinary `verify` automatically checks
the worktree's registered assessment before/after its mandatory exact-SHA build;
publication revalidates the bound assessment and artifacts. Main movement alone
does not invalidate unchanged behavioral observations. Required final-candidate
checks and user-required independent final review still observe the final tree.

`main` is the only shared code branch. Develop in isolated worktrees, including
detached worktrees. GitHub stores queue metadata at
`refs/coordination/omega-landing/main`; it runs no builds, waiting processes, or
model calls. There are no PRs or remote worker branches. In-progress
assignments live in the separate claims registry
(`refs/coordination/omega-claims/main`, [tools/claims](claims.md)); this queue
serializes only publication.

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

Setup and Git operations share a bounded command budget; acquiring the head
further bounds them to its original expiry. Before returning a reservation, the
client reobserves its persisted ticket, exact base and expiry, then checks the
clock again. A slow fetch or push may finish without a usable reservation.
Inspect status after a timeout: a remote mutation may already have landed.

The next waiter claims when it reaches the head; queue position alone does not
authorize integration. A waiting ticket can resume later, but an absent head
loses its turn at expiry. The next coordinating client atomically removes the
expired head before continuing. Do not concurrently edit or
rebase a worktree whose waiter may acquire its reservation. Read incoming main
separately while working; cancel if the queued change needs more implementation.

## Verify the build before publishing

Publish never builds: the head lease is exactly 180 seconds, and a compile
inside it has already cost a session its reservation. A candidate that
touches Rust therefore carries a build receipt instead. At a clean HEAD on
the exact commit you intend to publish:

```text
python tools/landing.py verify --base <base>
```

`verify` computes the affected packages from `git diff --name-only <base>
HEAD`: each changed `.rs`, `Cargo.toml`, or nested `Cargo.lock` walks up to the
first manifest with a `[package]` table; a change to the workspace root
`Cargo.toml`, `Cargo.lock`, or `rust-toolchain.toml` checks the product closure through `omega`. Any
non-empty set also adds `omega`, since checking the CLI package checks the
whole product dependency closure — dependents an API change broke. It then
runs `mbx check -p <pkg> ...` for the root workspace and a separate
`mbx check --manifest-path <workspace-manifest> -p <pkg> ...` for each affected
nested explicit `[workspace]` (plain `cargo` when `mbx` is absent), and
writes `build/landing_receipts/<candidate-sha>.json`. A non-Rust candidate
needs no check and records an empty receipt. `<base>` must be an ancestor of
HEAD, as publish requires; otherwise the diff would include other lanes'
changes, so `verify` refuses and asks for a rebase.

Every workspace check must succeed. The receipt records each workspace's
packages, command and exit; publication requires a passing record for every
affected standalone workspace. Root-only legacy receipts remain readable, but
flat package names alone cannot prove that a standalone workspace was checked.

The flow is: verify → enqueue/claim → if main moved, rebase and verify the
new candidate sha → publish. Publish refuses a Rust-touching candidate
whose receipt is missing, is for a different sha, recorded a failing check,
or covers fewer packages than the diff affects — that is the third publish
guard, after empty and coordination-only changes. There is no bypass flag;
rebase and `verify` again.

**Verify again after every rebase, not once before enqueue.** A rebase inside
the claim iteration produces a NEW candidate sha, and the verify receipt is
bound to the sha it was taken for, so the rebased candidate has none. Publish
never builds inside the lease, so it refuses with `no build receipt at
.../<sha>.json` and a granted claim is lost. Compare the base the claim
returned against your candidate's parent; when they differ, rebase, `verify`
again, then publish. When the incoming main movement between the prior receipt's
base and the new base touched no Rust, Cargo, or toolchain files (and the
candidate's own Rust diff is unchanged), `verify` automatically carries forward
the prior passing Cargo/mbx check evidence into the new candidate's receipt in
under a second (`reused_build_from`) while still re-checking pending markers,
golden corpus rows, and qualification.

**Keep the build warm, or the re-verify cannot fit the lease.** When main moves
under a held claim, the re-verify of the rebased sha has to finish inside the
remaining 180 seconds. A cold `cargo check` of the affected packages takes
about 3m17s and loses every time; the same check against a warm target
directory takes about 12s. So build the affected packages in the candidate
worktree *before* enqueueing, and keep that worktree's target directory for the
whole landing. Four separate sessions lost landings to this on a busy day —
one took four attempts while main moved five times in 25 minutes — and the
ones that pre-warmed landed on the next head.

Under an active swarm main can advance every few minutes, so expect to rebase
inside the lease rather than treating it as the exception.

## Land from one scripted invocation

The reservation is 180 seconds and the sequence verify → enqueue → claim →
rebase → re-verify → publish does not fit with a model turn between the steps;
the lease has been lost to tool-call latency alone. Script the whole sequence,
and carry these guards inside the script rather than around it.

- **The rebase belongs inside the script, after the claim.** A rebase the caller
  ran before starting moves HEAD, so a guard computed against HEAD sees an empty
  delta and cannot fire; the base the reservation is granted against is fetched
  by the claim, which only the script holds. Rebase onto that base, re-verify
  the rebased candidate (a rebase produces a new sha with no receipt), then
  publish.
- **The movement guard reads the measured revision, not HEAD.** Pass the
  revision the evidence was measured on as an argument and compare it with the
  granted base: `git diff --name-only <measured>..<base>` must contain no `.rs`
  files. That is independent of HEAD, so a rebase cannot erase the delta, and
  compiler movement between the measurement and the base releases the
  reservation instead of publishing stale evidence. Re-measure, then rejoin.
- **Write the note before `publish`.** Publish ends the reservation, and
  `claims.py note` attaches only while a claim is live
  ([claims lifetime](claims.md#lifetime-and-evidence)), so a note written after
  the publish is refused and the ticket keeps nothing. Render the note text
  inside the script *after* its own rebase, substituting `git rev-parse HEAD` at
  that point: a note prepared before the run names the commits the rebase
  replaced, and it cannot be corrected afterwards.
- **Run the golden content guard on the rebased tree and release rather than
  publish when it fails**
  ([splicing a golden](testing.md#splicing-a-golden-the-content-guard)); a
  lossy splice published under a granted base is another lane's silent
  regression.
- **When the script builds, let one build-slot hold cover all of it.** A
  re-verify that runs outside the hold queues behind the wave's other builds
  and can lose the 180-second lease while it waits; wrap the whole invocation
  once
  (`python3 tools/build_slot.py --slots 2 --timeout 5400 -- bash land.sh`) —
  the nested `build_slot.py` runs and `verify`'s own hold see the inherited
  `OMEGA_BUILD_SLOT_HELD=1` and short-circuit
  ([build slots](build_slot.md#one-hold-for-a-whole-chain)). A candidate whose
  verify checks no packages needs no hold.

## Check locally, then publish

Capture the exact candidate before running the checks required by [tools/testing.md](testing.md)
and the task. Keep the actual exits and keep the worktree clean at that commit.
The command checks Git state, not test success. Shape guards apply at
publish: a candidate that changes no files is refused outright (an empty commit
carries ledger noise, not a product change — record the finding on the claim
ticket with `claims.py note` and release instead), and a candidate touching
only coordination files (`TASKS*.md`, `OWNER_QUESTIONS.md`,
`tools/swarm/waves/`) is refused unless `publish` receives `--board-update`,
the coordinator's flag for batched board sweeps. A product change that carries
its board update inside the same commit needs no flag. A third guard refuses a
candidate that changes Rust packages without a passing `verify` receipt for
this exact sha (see "Verify the build before publishing"). That receipt also
records a check of every earlier commit in a multi-commit candidate, because
main is bisected one commit at a time; a commit that builds only with its
successor fails `verify` and should be squashed into it. Publish and `verify`
both refuse a commit whose message says its own validation or review is still
pending ("not published", "review pending", "validation … pending", "held for …
review"): finish that work, then reword the commit. Only the candidate's own
changes are checked. Compiler advancement uses the
change-scoped validation policy in [tools/testing.md](testing.md); a fresh full baseline is not a
publication prerequisite. Reuse evidence for unchanged inputs after rebasing,
and rerun checks whose inputs changed.
Apply [main-change validation](testing.md#main-change-validation) to the recorded
qualification revision; ordinary Git conflicts are expected integration work,
not a prohibition on overlapping edits. Preserve incoming behavior and required
independent review of the final tree. If affected integration checks cannot fit
the remaining lease, release, finish them outside the queue and rejoin; do not
waive checks or repeatedly start cold builds inside the reservation.

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
python tools/landing.py publish --claim <ticket> --base <granted-base> --candidate <verified-sha>
```

Both IDs are required and are validated before any queue change: `--claim` is
the 32-character ticket from `enqueue` (the `claim` that `claim` returns), and
`--base` is the **full 40-hex lowercase** `base` the reservation granted — not
your candidate, and not an abbreviated sha. A bare `publish`, or one with a
short base, exits 1 with only `Base requires a full lowercase Git object ID.`
or `Supply the 32-character ticket returned by enqueue`.

That refusal validates before it removes anything, so **the claim survives it**
and a retry inside the same lease is safe: correct the argument and run the
command again. Re-enqueueing is not required and `cancel` would lose the head.

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

### The lane is retained by the publish

A successful `publish` keeps the publishing worktree and its branch. A warm
lane owns a managed target that a cold rebuild pays minutes to recreate, while
reusing the lane costs a rebase; deleting it to save bookkeeping trades
seconds of tidiness for minutes of every later agent's time. Main is confirmed
to hold the candidate and the provenance note is recorded before this decision
is reported, so it can never influence whether a publication is landed. The
outcome appears as `cleanup` in the JSON result.

Provisioning churn is still waste, so retire lanes deliberately with `prune`,
which removes only what cannot pay for its slot:

```sh
python3 tools/landing.py prune --dry-run          # report, change nothing
python3 tools/landing.py prune --idle-hours 24    # also retire stale cold lanes
```

`prune` removes a worktree whose directory is missing (stale registration) or
which has never been built and has been idle past the threshold. It refuses,
naming the condition, whenever anything could still be lost:

- the worktree is the main checkout, or sits on the main line;
- it has uncommitted or untracked changes;
- it holds commits main lacks;
- it has a warm target, which is the reason to keep it.

The same refusals guard `landed_worktree_cleanup`, which `prune` reuses with no
candidate: there, "beyond the candidate" becomes "beyond main". Removal retries
with `--force` once those checks pass, then deletes any directory left behind.
That retry is safe only because every condition above was already rejected, so
`--force` can discard only ignored build output. Without it, removal is useless
on macOS, where a lane that has ever built holds ignored files — a stray
`.DS_Store` suffices — and `git worktree remove` refuses with "Directory not
empty". `git status --porcelain` reports only tracked and untracked files, so
that state passes the safety checks and only the removal notices it.

### Provenance notes

Commit messages carry no tool attribution (AGENTS.md), so after main holds the
candidate, publish records who produced it as a git note on
`refs/notes/provenance`: the queue owner label, the ticket, the publication
time, and — when supplied — free-form provenance such as the agent and model:

```text
python tools/landing.py publish --claim <ticket> --base <granted-base> --candidate <sha> --provenance "agent=devin model=swe-2-medium"
```

`OMEGA_PROVENANCE` supplies the same text when `--provenance` is absent, so a
swarm launcher can set it once per session. Use `key=value` words and always
include `model=<name>` (for example `agent=devin model=swe-2-medium
effort=high`); a publish without one still lands, prints a warning, and
returns `provenance_warning`. Of 201 notes written in the first day, about 15
named a model, so the owner label alone cannot answer "which model". The note is a separate push after
publication: it never blocks or undoes a landing, and the result's
`provenance` field reports `recorded` or why it was not.
Filesystem errors during this advisory write report an unrecorded note, not a
failed publication; failure to clean its unique local scratch ref likewise
cannot overturn a confirmed publication or note push.
Notes commits use a
fixed `omega-landing` committer, since a lander's private address would be
refused by GitHub's email-privacy check. Read them with:

```text
git fetch origin refs/notes/provenance:refs/notes/provenance
git log --notes=provenance
```

## Leave the queue or recover abandoned work

```text
python tools/landing.py cancel --ticket <waiting-ticket>
python tools/landing.py release --claim <active-ticket>
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

An expired head orphans nothing: the verified commit still exists. To finish
its publication, wait for the queue to clear, then re-enqueue the full
candidate SHA, claim the head, check out that exact commit so HEAD matches
`--candidate`, and publish. Any owner may do this for a candidate whose
session died — the claim is on the queue, not on the commit.

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
