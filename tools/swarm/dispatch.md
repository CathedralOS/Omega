# Reserve before automatic dispatch

Use the existing claims registry for automatic assignment across Codex, Cursor,
Devin and local agents. `claim` remains advisory for explicitly owned work.
`claims.py reserve` atomically selects one outcome/session owner; overlapping
compiler-file estimates do not prevent distinct outcomes. All coordinators must
refresh these tools. Older clients and direct external APIs cannot be fenced by
repository code. This is cooperative coordination, not authentication.

## Parent procedure

1. Fetch main successfully and prepare an isolated selection checkout at that
   exact commit. Preserve the starting checkout, unpublished branches, reusable
   target directories and MBX cache. Read current `AGENTS.md`, the relevant swarm
   skill, task acceptance and governing spec. Never reset another owner's lane.
2. Inspect runtime sessions and existing claims, including `expired`. Prefer an
   available completed Codex session in the same saved Omega environment over
   creating a new one. A completed *turn* is not completed outcome acceptance.
   Check its final result, remote landing, cleanup and remaining parent acceptance;
   preserve ongoing or blocked work. Release its old claim only after closure or
   an explicit stopped-owner recovery. Do not repurpose active Zac/Devin work.
3. Rank genuinely unfinished, dependency-ready outcomes from current main. Prepare
   the reviewed selection packet below. `sample_queue.py --facts` is discovery
   evidence, not an atomic reservation or proof a row remains unfinished.
4. Reserve before sending any expensive implementation assignment:

   ```text
   python3 tools/swarm/dispatch.py --repository CHECKOUT reserve --packet PACKET --attempt build/swarm/WAVE/ATTEMPT.json --owner "coordinator / slot"
   ```

   The tool fetches main, checks its exact SHA and Git object fingerprints, skips
   rows with no remaining measured gap, and tries candidates in supplied priority
   order. CAS collisions recheck outcome and session ownership in the common
   registry. Exit 0 with `phase: reserved` is required; `no_work` is not permission
   to invent work. A registry error/unknown write preserves the ticket and stops.
   No eligible completed session stops selection unless `--allow-create` explicitly
   authorizes requesting a new one. This command never starts a session.
5. Refresh the actual runtime observation before sending. Run `arm --attempt FILE --session-observation FRESH-ROW.json`
   once: it rechecks main, gap, session observation and the exact live reservation.
   The fresh row must name the same reserved session and be observed after reservation;
   running or unavailable sessions refuse admission. Then use the discovered supported runtime or existing durable cloud adapter to
   send exactly once to the selected session ID, or create only when authorized.
   Include the reserved ticket, owner, base, full remaining acceptance, current
   spec packet, model/effort, publication authority and stopping condition.
   The worker adopts/renews that ticket, fetches main again, uses a fresh isolated
   worktree, refreshes spec context, and validates affected inputs. Retained caches
   never authorize stale sources, binaries, acceptance or an old landing receipt.
6. Save the actual runtime envelope. `sent --attempt FILE --evidence RECEIPT`
   records known delivery; `uncertain` preserves an unknown send. An armed attempt
   cannot be armed again or automatically cancelled. Read the existing runtime task
   before any retry; preserve the attempt ID, branch and ticket. A definitely
   unsent `reserved` attempt can use `cancel` to release its own claim. If a send
   was definitively rejected, reconcile the actual provider receipt and explicitly
   release that owned claim; do not treat a timeout as rejection. Session/tool
   send enforcement beyond this boundary belongs to the parent; never claim the
   repository can transact with arbitrary external tools.

The Devin `launch.py launch` path also reserves through claims before each actual
create, persists a per-slot intent before POST, and never retries POST blindly.
`--skip-claims-check` skips advisory probes only. `--relaunch` cannot replace an
existing send intent; reconcile/reuse that actual session. Explicit read-only
semantic reviews retain send durability but do not reserve implementation ownership.
Its fixed manifest path remains for reviewed named assignments; automatic outcome
selection uses the common packet procedure above. The local renderer does not send
anything; Cursor/Codex external send APIs require the parent's reserve/arm procedure.

## Selection packet

A packet contains `base`, ordered `candidates`, and actual observed `sessions`.
Session namespaces are `codex`, `cursor`, `devin` and `local`, normalized to
lowercase; provider handles retain their original case. Unknown namespaces refuse
automatic reservation. The same rule applies to `claims.py --session-id`.

Each candidate has `item`, optional `board`, `remaining`, `reviewed_by`, `specs`
(canonical `wiki/spec/` paths), advisory `paths`, and `source_inputs`. Generate
fingerprints with `qualification.source_inputs(repository, base, paths)`; include
its fact folder or board and governing specs. Review priorities, parked work,
exact fact/bound/operation and dependencies; the reviewer label is not authenticated.

For a canonical fact folder, missing named positive places (`local`, `field`,
`parameter`, `call_result`, `match_join`, `const_name`, `other_state`,
`other_machine`, `target_body`) are a concrete coverage gap. Confirm the folder
uses that canonical layout before listing it. Presence of nine files never proves
semantic acceptance: when they all exist, automatic redispatch requires a measured
failure. Do not use missing arbitrary filenames or a green root probe as a gap.
For a spec/board outcome or a filled fact row, supply `failure: {path, sha256}`
pointing to a failed, noninvalidated qualification observation on exactly `base`,
with unchanged source and log digests. `failure_review` states which acceptance
failed and distinguishes semantics from timing/infrastructure failure. An unchanged
or completed outcome with no genuine gap is omitted, not assigned for investigation.

Each session has `platform`, actual `id`, `status`, `available`, `outcome_complete`,
`approval_required`, `observed_utc`, `evidence: {path, sha256}`, and `runtime_fields`:
JSON key/index paths extracting `id` and `status` from that actual runtime receipt.
For example a receipt `{"session_id":"REAL-ID","status":"completed"}` uses
`{"id":["session_id"],"status":["status"]}`. Discover actual schemas; do not guess
fields or handles. Observations older than five minutes are refused. The extracted
identity/status must agree; availability, closure and actual endpoint authorization
still require the parent's runtime/acceptance review. No normalized label creates
service authority or shared compiler acceptance.

`previous_probe` and `current_probe` come from the same session's saved executor:

```text
python3 tools/swarm/dispatch.py --repository STABLE_CHECKOUT probe --initialize --cache CACHE_DIRECTORY
python3 tools/swarm/dispatch.py --repository STABLE_CHECKOUT probe --cache CACHE_DIRECTORY
```

Initialize once while recording the old executor; never replace the ignored
workspace token. A probe records machine/runtime generation, workspace token,
workspace/cache filesystem identities. On hosts without a readable kernel boot ID,
pass `--runtime-id` only from actual runtime metadata. Unknown generation/token,
missing caches or changed identities produce `cache_continuity: false`; the same
completed session may still be continued, with fresh preflight and cold caches.
This reports directory retention, not cache hits or artifact validity. Platform
reprovisioning may discard all caches. Do not delete or force a common Cargo target
across editing agents; reuse the executor's supported cache and isolated lanes.

## Stopped ownership and recovery

Expired records remain unresolved in claims until explicit release/recovery; age
alone does not prove a worker stopped. Inspect actual handles, processes, saved
session, dirty worktree, branch commits and claim notes. Continue live or blocked
work on its existing owner. For a confirmed stop, preserve its checkpoint and
record the observed stop/branch/session evidence in the recovery reason. A named
user-authorized takeover of active work must name that owner and authority in the
reason; a generic lease age is never that authority. No automatic selector calls
`recover`, resets a branch or discards progress. Claims and provenance are labels,
not authenticated identities. Old clients can still reap expired records; refresh
all coordinating clients and reconcile runtime/in-flight work even when a claim is
absent. No new queue, acceptance system or batch integrator is introduced.

Focused simulations: `python3 tools/tests/test_swarm_dispatch.py -v`,
`python3 tools/tests/test_claims.py -v`, `python3 tools/tests/test_swarm.py -v`.
They use temporary local Git remotes and mocked transports, not worker batches.
