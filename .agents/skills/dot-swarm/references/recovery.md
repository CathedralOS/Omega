# Recover interrupted lanes

Use this procedure before dispatch/resume, after a disconnect or uncertain tool
return, and before completion cleanup. It supplements the existing manifest,
`receipts.json`, `slots.json`, claims and landing contracts; it is not another
scheduler. A disconnect is an observation, not an explanation or a failed job.
No documented keep-alive control is available here. Do not create busy loops.

## Local record and adapter boundary

`tools/swarm/recovery.py` provides a versioned, atomically replaced local ledger,
bounded command wrapper and read-only decision planner. Python 3.9+ and Git are
the existing prerequisites; use `python` instead of `python3` on Windows when
appropriate. OS file locks release on process exit on Linux, macOS and Windows.
Ledger schema 2 binds execution and checkpoint identities. Earlier schema-1
review ledgers are refused, never silently upgraded or reset; preserve them and
reconcile their observations explicitly before registering new work.
An interrupted write leaves the previous or next complete revision, never a
partially written record. Temporary files are not committed state. Filesystem
or machine destruction still requires an independently confirmed backup.

Choose **one shared ledger per actual executor**, covering all its waves and
builds. Obtain executor identity and an incarnation/generation from the runtime
or an explicitly confirmed host assignment; never guess from a path, PID or
hostname alone. Coordinators on the same executor must use the same ledger.
Different computers get distinct ledgers and build slots. Record the mapping in
the coordinator handoff. A copied ledger cannot establish a new host's identity.

Keep it under the coordinator's ignored `build/swarm/recovery/` directory. Retain
lane checkpoints outside that computer through the approved artifact route.
Do not store credentials, environment dumps, signed URLs or full tool responses.
The portable helper has no dot, Devin or Library API. Its adapter is the agent
using the **currently exposed** tools, not a guessed endpoint or tool name.

The adapter supplies fresh read-only observations (maximum age 120 seconds):

- Runtime executor/generation, availability, worker handle and exact assignment.
  An unavailable/disconnected or missing handle is `unknown`, not stopped.
- Worktree existence, exact HEAD, clean/dirty state and primary-worktree status,
  via Git on that executor. A remote path is never presumed locally accessible.
- Current main and ownership from existing Git/claims/landing helpers. A quiet
  tree or expired claim is not process evidence. `ownership: clear` means the
  reviewed paths have no conflict; it does not acquire a claim or authorize edits.
- Every operation's exact recorded ID, process status including descendants,
  terminal exit if known, and evidence locator. For a remote mutation, report
  `applied`, `not_applied` or `unknown` only after reconciling the exact request
  against the authoritative registry/result. Absence from an incomplete list
  proves nothing. Timeout, cancellation and a failed client exit are **unknown**.
- For cleanup, actual owned resources, exact HEAD proved landed in fetched main,
  and fresh claims/ticket lists. Shared caches and provider lifecycle are separate.

## Concrete coordinator loop

Examples use a ledger path chosen for this executor; substitute actual IDs. Init
is idempotent only for the same executor and generation. Every later write needs
the revision just read; a conflict means reread, not forced overwrite.

```sh
python3 tools/swarm/recovery.py --state build/swarm/recovery/host-a.json init --executor host-a --generation instance-7
python3 tools/swarm/recovery.py --state build/swarm/recovery/host-a.json --revision 0 register --input build/swarm/lane.json
```

Minimal `lane.json` (replace symbolic commit IDs with real full IDs):

```json
{"id":"atomic","assignment":"atomic-leg1","worktree":"/executor-local/Omega/.agents/worktree/atomic","branch":"swarm/atomic","base":"BASE_SHA","head":"HEAD_SHA","reviewed_main":"BASE_SHA","worker_handle":"runtime-handle","mode":"implementation","checkpoint":{"status":"incomplete","summary":"source preserved; two checks failed; native unrun"},"resources":{"target":"/executor-local/atomic-target","cache":"shared; preserve"}}
```

Checkpoint before long commands, blocked waits, or any remote mutation. Record
commands/exits, unrun gates, patch hash, residuals and exact base in the checkpoint;
`status` is `incomplete`, `blocked` or `validated`, never inferred from agent exit.
After legitimate source progress include new `head` and `reviewed_main` values
from Git. Each checkpoint call increments its lane's checkpoint revision. Backup
identity always includes the lane's canonical HEAD/base, executor/generation,
lane/assignment and that revision, even when the input omits `head`. Repeating
old summary text cannot revive an old backup. No recovery command rebases or
upgrades validation evidence.

```sh
python3 tools/swarm/recovery.py --state build/swarm/recovery/host-a.json --revision 1 checkpoint --lane atomic --input build/swarm/checkpoint.json
python3 tools/swarm/recovery.py --state build/swarm/recovery/host-a.json --revision 2 reconcile --input build/swarm/observed.json
python3 tools/swarm/recovery.py --state build/swarm/recovery/host-a.json status
```

Example observation for revision 2; timestamp must be freshly observed UTC:

```json
{"schema":1,"id":"observation-1","revision":2,"executor":"host-a","generation":"instance-7","availability":"available","observed_utc":"2026-09-30T22:00:00+00:00","lanes":{"atomic":{"assignment":"atomic-leg1","worker_handle":"runtime-handle","worker_state":"idle","processes_stopped":true,"workspace":{"exists":true,"head":"HEAD_SHA","clean":false,"primary":false},"main":"BASE_SHA","ownership":"clear","claims":[],"landing_tickets":[],"resources_owned":true,"operations":{}}}}
```

Consume each returned `lanes[].action`, instead of treating the ledger as a log:

| Action | Coordinator's next step |
| --- | --- |
| `reconcile_executor/worker/processes` | Read actual runtime/process state; retain work and build slot while unknown. Reuse the existing task if the runtime supports it. |
| `reconcile_operation` | Inspect the exact pending command/result and authoritative remote state; never dispatch another mutation to discover its outcome. |
| `wait_running` | Keep the existing assignment and resources; observe its result without dispatching another worker. |
| `preserve_missing_workspace` | Stop this lane; find its confirmed artifact identity through the approved adapter. Do not create an empty replacement and call it recovered. |
| `materialize_verify_backup` | Materialize onto the destination executor through its supported route, verify archive/member hashes and base, then establish a new explicit local mapping. |
| `review_worktree/upstream`, `reconcile_claims/build_slot` | Inspect current code, claims, versions or process tree. Renew/reacquire only under ordinary ownership rules. Never steal a slot from missing/stale observations. |
| `offer_resume` | Within existing authorization, record the returned `resume_intent` **before** one adapter dispatch to the existing handle. |
| `review_resume_result` | This assignment already has a resume intent. Reconcile acknowledgment/result; do not repeat dispatch. A new leg uses a new explicit assignment record. |
| `review_cleanup` | This exact completed lane has a fresh eligible inventory. Review owned-resource cleanup below; do not dispatch it again. |

```sh
python3 tools/swarm/recovery.py --state build/swarm/recovery/host-a.json --revision 3 intent --lane atomic --operation resume-atomic-leg1 --kind mutation
```

Only after that write succeeds does the adapter invoke the runtime continuation
once. Persist the returned handle/result, then supply another observation.
Duplicate resume fails before dispatch. `tools/tests/test_swarm_recovery.py`
exercises this exact coordinator consumption using explicit fake adapters,
including a disconnect between dispatch and its reply.

For a local command, use the same journal with an explicit wall-clock limit:

```sh
python3 tools/swarm/recovery.py --state build/swarm/recovery/host-a.json --revision 4 run --lane atomic --operation claim-status-1 --kind read --timeout 45 --cwd /executor-local/Omega -- python3 tools/claims.py status
```

`run` executes once without a shell and records intent before starting. Logs and
durable results live under `<state filename>.executions/<identity hash>/`, bound
to that ledger, executor/generation, lane/assignment, worker and unique execution
ID. Duplicate operation names in other lanes or ledgers cannot collide. It never
stores argv or environment in the journal.
The result is atomically saved before merging into the ledger. The merge checks
the exact execution identity under the lock, so an unrelated lane update cannot
discard the exit/PID/result. If the merge is interrupted or busy, replay only the
saved evidence (no `--revision`; this is an identity-guarded merge):

```sh
python3 tools/swarm/recovery.py --state build/swarm/recovery/host-a.json complete --lane atomic --operation claim-status-1
```

`complete` never executes the command. Repeating it is idempotent. Missing,
conflicting or mismatched evidence stops for reconciliation. A process crash
before the result was saved can still lose the exit; never invent it.
Nonzero/timeout results are not automatically retried. Mutation exits still need
remote read-only reconciliation. The wrapper bounds its client, **not** the remote
operation. A zero wrapper exit means the result was recorded: inspect the
operation's `exit`, `local_result` and `outcome` for the command verdict.
On interruption POSIX signals its own process group; Windows stops
its own parent. Detached descendants remain possible: no timeout releases a
build slot. For builds use `--kind build`; the slot stays reserved until a fresh
observation proves the matching operation and children stopped. An agent-turn
interrupt alone does not do that. When fresh exact-operation evidence proves a
local read/build and its children stopped but no exit survived, reconciliation
records `stopped_unknown_result`: terminal, never a pass. A deliberate new intent
may then start another attempt. Remote mutations remain `unknown` until their
effect is reconciled. A log-open/process-creation failure before dispatch is
`not_started`, not an uncertain started process. Do not run `fill.py recover --execute` based
only on its old `dead` ledger flag; perform this reconciliation first.

After a resolved `not_applied` result, a retry still requires the existing
helper's supported workflow and authorization, with a new explicit intent ID.
Never retry create/upload/publish because a response disappeared. Permission
denial stays a blocker; report the exact action for transcript evidence.

## Verified backups

Create a small staged directory with only reviewed recovery files: binary patch,
base revision, checkpoint and essential validation evidence. No recursive cache,
credential, policy-lock or private-session-store collection. The helper's obvious
credential-path exclusions are not a secret scanner; the adapter reviews bytes.

```sh
python3 tools/swarm/recovery.py --state build/swarm/recovery/host-a.json --revision 5 bundle --lane atomic --root build/swarm/staged --output build/swarm/atomic-recovery.tar.gz --file candidate.patch --file base.txt --file checkpoint.json
```

This exclusively creates a bounded archive with a member manifest and verifies
every member checksum, byte count, base and canonical checkpoint identity;
non-padding content after the tar end marker is rejected. It records
`local-only`, not a backup success. A failed/interrupted pack is invalid; inspect
and choose a fresh destination rather than overwriting a potentially saved file.

Record an upload intent, then use the **current supported Library skill** or the
user's explicitly approved artifact adapter. Search/reuse existing file identity;
preserve returned version and xattrs. Reconcile uncertain upload outcomes before
any retry. Do not embed transfer APIs in this script or assume another machine's
path exists. Correlate the successful response with these exact local bytes and
make a minimal receipt (not a raw response):

```json
{"operation":"upload-1","status":"succeeded","route":"Library","file_id":"RETURNED_LIBRARY_FILE_ID","version":0,"sha256":"ARCHIVE_SHA256","bytes":12345}
```

```sh
python3 tools/swarm/recovery.py --state build/swarm/recovery/host-a.json --revision 7 backup --lane atomic --artifact build/swarm/atomic-recovery.tar.gz --sha256 ARCHIVE_SHA256 --receipt build/swarm/receipt.json
```

`confirmed-upload` means a successful adapter receipt for the verified bytes;
it is not a download/restore test. A new checkpoint makes an old backup stale.
Retain file ID, version, hash and base in the coordinator handoff, outside the
executor. A local JSON ledger cannot survive deletion of its own machine.

## Completion inventory and cleanup

`status` includes owned worktree/branch, worker handle, observed claims/tickets,
declared build/cache resources, backup status and the executor build slot.
`cleanup: eligible-owned-landed` requires fresh matching executor/assignment,
terminal worker and all processes, no uncertain operation, clean non-primary
worktree at the recorded HEAD, that exact HEAD proved landed (`landed_head` plus
`landed_evidence` from Git), no live claims/tickets, and verified resource ownership.
Otherwise it says `preserve`. Neither result deletes anything or stops a worker.

The coordinator may then use ordinary helpers to release **its own** claims or
cancel its own ticket after reconciling outstanding mutations, or remove only
its clean verified-landed worktree/branch. Reobserve after each action; a stale
inventory is not cleanup authority. Preserve blocked/unlanded WIP even with a
backup. Remove only explicitly owned inactive targets if authorized; shared
caches, user task history and unrelated files stay outside this cleanup plan.
Provider-managed machines are a separate inventory category: report their last
observed status and whether a documented lifecycle control is exposed. Do not
claim infrastructure deletion, automatic expiry or continued billing without
evidence. Use existing tasks rather than creating a pool of replacements.

Landing still follows `tools/landing.py`: refresh main and claims after waiting,
reconcile wire/semantic version ownership, validate exact candidate inputs, and
regenerate exact-HEAD receipts after rebase. Its 180-second lease is not extended
by this ledger or a heartbeat. An unrun gate never becomes passing evidence.

## Evidence scope

The regression suite injects interrupted writes, stale records, disappearing and
resumed executors, corrupt/incomplete archives and ambiguous mutations through
local/fake adapters. It does not prove cloud transport, keep-alive, Library restore
or publication reliability. Real observations motivating this procedure were a
missing coordinator workspace, a disconnected executor later returning intact,
asymmetric Library transfers, a stalled claims fetch, evolving wire ownership,
and short landing leases. None established an idle-timeout cause. Report new
transport evidence separately from these simulated recovery tests.
