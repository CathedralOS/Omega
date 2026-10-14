# Shared integration of complete outcomes

Use this route only for a user-authorized wave with one publication integrator.
Each owner still repairs and validates its entire runnable outcome, retains its
context, and fixes integration regressions through verified remote acceptance.
The integrator selects ready complete fixes, prepares their combined dependencies,
captures the combined behavior, obtains independent review, and lands through
[the existing queue](landing.md). An uploaded archive is evidence, not completion.
Independent owners continue to use the ordinary sample and landing loop.

`python tools/integration.py --help` is the portable Python 3 entrypoint on Linux,
macOS and Windows. It uses existing Git, qualification, claims and landing tools;
it creates no remote code branches, scheduler, registry or publication protocol.
Absolute paths below belong to the executor running each command. Local receipts
are immutable historical evidence when transported, never relabeled observations
of the integrator's runtime or candidate.

## Owner submission

At a clean reviewed candidate, finish an ordinary qualification with passing
positive native/Terminal or test observations and rejecting negative controls.
Keep source, independent oracle/validator and all producer dependencies in the
reviewed check input closure. The separate review JSON must contain
`approved: true`, the exact `candidate`, `config_digest` from the qualification,
and the independent `reviewer`. Preserve actual source reuse review receipts.

```text
python tools/integration.py submit --repository <worktree> --state <qualification.json> --base <source-base> --review <review.json> --id <outcome-id> --outcome <complete-result> --output <ready.tar.gz>
python tools/integration.py inspect --archive <ready.tar.gz>
```

Repeat `--requires <other-outcome-id>` only for an actual dependency. The archive
contains a Git bundle, original qualification observations, review, launcher and
producer bytes, stream logs and their digests. It is capped at 2 GiB and rejects
links and unexpected archive members. It includes historical producer revisions.

Use the executor's supported Library/file transfer tool to upload the archive;
record the returned file ID, version and archive SHA-256 in the existing claim
note/controller assignment and report them to the parent. The integrator uses its
supported download tool to materialize those exact bytes. A path in a cloud worker
is not a parent-readable path. If either transfer capability is unavailable,
report that concrete blocker; do not ask the user to shuttle files, invent URLs,
or recreate the producer build merely to transfer it. This helper deliberately
does not pretend to provide a cloud runtime or Library API.

## Prepare and freeze a ready selection

Receive into a fresh directory, then cherry-pick the selected complete commits in
an isolated clean worktree. Receipt verification binds the original state,
observations, source objects, artifacts, logs and review; a failed receive leaves
its evidence and cannot be selected. Filesystem permissions are not conveyed.

```text
python tools/integration.py receive --repository <integrator> --archive <ready.tar.gz> --destination <inbox>
```

Selection JSON is a list of objects containing `manifest` (the received manifest's
absolute path) and `integrated_commits` (exact applied commits). If reconciliation
changes a patch, add a concrete `reconciliation` explanation for independent
review. Duplicate outcomes/patches, incomplete dependencies and missing commits
are refused. Select only ready outcomes; neither wait for every worker nor fold
late arrivals into a running capture.

Prepare one actual producer per distinct configuration, with an isolated stable
optimized-dev target. Build-plan JSON contains `configuration`, a `command` argv
starting with `mbx build` (or Cargo fallback), `environment` with
`CARGO_PROFILE_DEV_OPT_LEVEL: "1"` and absolute `CARGO_TARGET_DIR`, complete Git
`inputs`, absolute produced `outputs`, and optional finite `timeout_seconds`.
Use host build admission before invoking it. Include Cargo/toolchain files, build
scripts and the full compiler/library source closure; review completeness rather
than trusting a configuration label. Failed builds retain logs and produce no
successful receipt.

```text
python tools/integration.py build --repository <integrator> --plan <build-plan.json> --receipt <producer.json>
```

Prepare packages through their normal admission route before capture. A project
trust decision does not confer OS privileges or filesystem access. In the union
[qualification definition](qualification.md), `producer_groups` maps each group
name to its receipt path. Every applicable check names `producer_group`, includes
that group's actual artifacts, and has `producer: {revision: <actual-producer-sha>,
inputs: [<complete-source-closure>]}`. Preserve all original positive/negative
checks through `integration_checks: [[<outcome-id>, <original-check-id>]]` on each
corresponding combined check. Kind and oracle/dependency closure must be retained;
review cannot substitute for behavioral controls.

A check may declare `prepared_inputs` as absolute frozen files/directories for
package locks, external dependencies or validator resources. Their bytes and
membership are sealed and checked before/after commands. Keep generated outputs
outside these frozen directories. Linked libraries belong in the explicit
artifact closure. Include a `kind: "review", final: true` check for independent
review of the exact combined candidate. Changed canonical contract blobs require
explicit `refreshed_contracts` paths after rereading the new passages.

```text
python tools/integration.py preview --repository <integrator> --base <base> --config <union.json> --selection <selection.json>
python tools/integration.py freeze --repository <integrator> --base <base> --state <qualification.json> --config <union.json> --selection <selection.json> --review <approved-preview.json>
```

The independent reviewer receives the preview plus exact combined diff and focused
canonical context. Their approved JSON retains every preview binding and adds
`approved: true` and `reviewer`. It binds selection, reconciliation, producer and
prepared bytes, candidate, base and definition. Freeze starts one ordinary
qualification state. Run its pending checks through `tools/qualification.py` and
assess that state. Source/spec, artifact, environment or package changes are
invalidation events. Identical output alone never establishes equivalence.

## Reconcile without discarding evidence

An arrival in another inbox cannot mutate a frozen selection. Main writers keep
working; this route provides no exclusive development lease. Fetch and reconcile
main outside an active capture. Amend the qualification definition if its closure
changes, then obtain a fresh preview/review and explicitly `reseal --revision
<current-revision>` using the same arguments as freeze. Old seals and observations
remain recorded. Ordinary qualification determines which unchanged observations
can be reused; exact-candidate final review remains required.

If a combined check fails, preserve its receipt and logs. Reduce/isolate the
interaction, retain passing outcomes, repair with the responsible owner or remove
the suspect complete outcome in an explicit successor selection. Reconcile its
commit mapping, amend/reseal and rerun affected checks. A union failure does not
by itself establish which owner caused it. Do not restart unrelated owners or
repeat unchanged successful checks by default.

Finish ordinary `landing.py verify`, enqueue/claim, publish and fetch to verify
remote ancestry. Mandatory exact-SHA landing checks remain independent obligations
and may entail a different build configuration; do not count them as avoided.
A main race requires reconciliation, explicit resealing and required revalidation,
never a force push or borrowed review. Owners are complete only once their accepted
outcomes are remotely verified. Clean up the landed integrator worktree/branch.

Record actual ready, freeze and publish times in existing claim/controller notes.
Use producer wall time and qualification run/reuse records to report measured
repeated work. An old/new comparison requires equivalent measured scope; do not
invent a speedup from one build or extrapolate a Linux pilot to other hosts.
