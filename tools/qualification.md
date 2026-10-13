# Pinned qualification

Qualify one clean checkpoint coherently. Fetch and integrate main near
publication, then assess the actual dependency changes before repeating work.
`qualification.py` runs validators and records their identities; it does not
decide scientific correctness, infer a complete dependency closure, approve
packages, or replace independent review and the normal landing protocol.

Previously cloud completion could compare historical Git inputs only after the
work finished. That gives an owner no executable requalification plan and does
not identify an untracked compiler library replaced during capture. The runner
records source objects, actual executable/dependency digests, explicit producer
revision, environment, command, stdout/stderr, exit, elapsed seconds and repeat
work. Changed inputs during a successful command make its evidence unusable.
Each run also has a separate hashed observation receipt; editing its state exit,
command, invalidations or provenance cannot manufacture reuse. Source and
artifact filesystem identities/change timestamps constrain the capture interval,
including replacement followed by restoration of identical bytes. This assumes
ordinary kernel timestamp/identity integrity and trusted reviewed validators;
it is not protection against a privileged actor forging the host or evidence.

## Define the checks

Keep the reviewed configuration and state under ignored `build/`. Each check
declares the complete producer/consumer/oracle/harness/configuration closure as
Git paths. Directory tree identities include additions and deletions. List the
exact untracked executable, compiled libraries, producer receipt and runtime
input files consumed under `artifacts`, using absolute file paths. A receipt
alone does not hash the linked libraries it describes. Generated native programs
are artifacts too. Use a private build directory and retain immutable producer
outputs; a passing run does not authorize rebuilding into another owner's cache.

For a conservative compiler-backed closure include the exercised compiler
sources, bundled libraries, Cargo manifests/lockfiles, toolchain configuration,
target provisions, sample sources, frozen oracles and validator. Narrow it only
with independently reviewed dependency evidence. Include environment-dependent
inputs as files/artifacts or in the explicit context. The launcher executable
and common Cargo profile/target variables are captured automatically; this is
not a complete environment dump. Do not put credentials in context or logs.

Example for this repository's Python workflow checks:

```json
{
  "context": {"host": "recorded executor", "target": "host", "toolchain": "recorded Python"},
  "checks": [{
    "id": "qualification", "kind": "tests",
    "command": ["python3", "tools/tests/test_qualification.py", "-q"],
    "inputs": ["tools/qualification.py", "tools/landing.py", "tools/swarm/recovery_state.py", "tools/tests"],
    "artifacts": [],
    "closure_reason": "qualification and landing implementation plus imported test helpers",
    "final": false, "expected_exit": 0, "timeout_seconds": 120
  }]
}
```

Kinds are `build`, `tests`, `native`, `terminal`, `negative`, and `review`.
Native/Terminal checks additionally require
`"producer":{"revision":"FULL_SHA","inputs":["COMPLETE_COMPILER_INPUT_PATHS"]}`
and nonempty `artifacts`. The producer's declared source inputs must agree with
the qualification revision before execution. This records an independently
verified producer relationship; an asserted SHA does not establish how a binary
was built. Include its exact build receipt among artifacts and review provenance.

The command is an argv list, never shell text. Use a validator that checks every
numerical/output/diagnostic/count oracle, not a compiler exit alone. Negative
controls must verify the authored reason; numerical native/Terminal validators
must capture actual values. Commands get finite stdin (EOF), captured separate
streams and a bounded timeout. Supply fixture input through the validator.
An unfinished command is not acceptance; reconcile any surviving children using
the existing executor recovery procedure before continuing.

Set `final:true` for checks that must observe the exact integrated candidate,
including user-required final review. Dependency equality cannot carry these
checks across a SHA change. Landing's exact-SHA build check remains mandatory
independently of this list.

Semantic compiler repairs additionally carry a compact `semantic_contracts`
list: `implementation` paths, a canonical `contract` path under `wiki/spec/`,
`classification:"missing_implementation"` and a short `reason` explaining the
required behavior. Include `passages`, a small list of exact relevant canonical
excerpts (at most 2000 characters each, 6000 per mapping). A language change
instead says `classification:"owner_decision"`
and records the explicit owner's `authorization`; an agent's proposed design is
not authority. Mappings require a `review` check with `final:true`. Independent
review must distinguish an implementation gap from an invented feature and
verify preservation of authority, custody, type/loan, arithmetic and provenance.
Contract files enter every check's dependency closure. They must exist unchanged
from the integrated main base: editing a spec alongside its implementation does
not retroactively authorize it. Proposed spec changes need the owner's decision
and separate canonical integration before this implementation qualification.

Before semantic implementation, render the context with
`qualification.py context --base FULL_MAIN_SHA --config CONFIG_JSON`.
Use `context --state STATE_JSON` for the recorded qualification. Include its
actual passages, source revision/blob hashes and behavior-to-contract mapping
in the implementation **and independent-review** handoffs; a separate promise
to retrieve a linked spec is insufficient. The tool rejects missing/stale or
invented excerpts and records the focused context in normal evidence. Review
still judges whether those passages actually authorize the behavior: their
presence proves retrieval and identity, never semantic compliance.
Existing `swarm/launch.py` manifests may supply the same `semantic_contracts`
and exact `source_revision` on a session. Validation reads the canonical blobs,
rejects stale/missing excerpts, and both local/cloud prompt renderers include the
actual focused passages and identities automatically. Do not substitute a
caller-authored `canonical_context` for that verification.
Set session `semantic_handoff_role` to `review` to render the independent-review
packet, or `continuation` with `previous_semantic_handoff` containing the saved
previous packet. Manifest validation rejects a missing prior continuation
packet and verifies current focused fingerprints before the local/cloud prompt
renderers run. Unchanged continuation prompts reference the retained fingerprint;
changed mappings deliver refreshed passages. Save `semantic_handoff` from the
validated manifest as the next previous packet. The generic cloud-controller
follow-up message adapter does not automatically invoke this manifest path:
its dispatcher must supply the validated rendered prompt or CLI handoff packet.

Use `handoff --base FULL_MAIN_SHA --config CONFIG_JSON --role implementation`
before coding, and `handoff --state STATE_JSON --role review` for independent
review. Save the emitted JSON and give the recipient that packet alongside
the candidate diff and captured check logs. It includes the actual passages,
behavior mapping, source identities, exact candidate, changed paths, check
definitions and recorded assessments. For continuation use
`handoff --state STATE_JSON --base INTEGRATED_BASE --role continuation
--previous SAVED_PACKET_JSON`. It checks the selected contract fingerprints:
an unchanged mapping needs no reinjection (`reinject_context` is empty);
changed selected contracts emit refreshed focused context. Stale excerpts fail
and require explicitly updating the mapping after reading the new contract.
An unrelated main advance alone does not trigger reinjection or qualification.
The full canonical context stays in the saved packet for subsequent resumes.
The CLI does not deliver packets to agents or prove that an agent follows them;
the dispatcher must include the packet and the independent reviewer must judge
coverage, semantic authority and the completeness of the test closure.

## Run once, then integrate

All mutations require the latest returned state `revision`; a concurrent writer
or running command causes refusal, never last-writer-wins replacement. Separate
owners retain separate worktrees/states and the existing executor build slot.

```text
python3 tools/qualification.py start --state build/qualification.json --base FULL_BASE_SHA --config build/qualification-config.json
python3 tools/qualification.py run-pending --state build/qualification.json --revision 0 --base FULL_BASE_SHA
```

Retain this revision while qualifying. Another main commit is not a reason to
rebase in the middle of a capture. Once ready, fetch main, inspect incoming
changes, rebase and assess the new candidate **outside the landing queue**:

```text
python3 tools/qualification.py assess --state build/qualification.json --revision LAST_REVISION --base FETCHED_MAIN_SHA
```

The assessment reports exact incoming paths and each check's `reuse`/`rerun`
decision with reasons: changed source tree, compiler/artifact, environment,
definition, damaged observation, failed prior run, or final-candidate requirement.
Reused observations keep their original SHA and logs; they are not relabelled
as fresh candidate executions. `run-pending` assesses, runs only needed checks,
then assesses again. An explicit `run --check ID` always runs and measures
unnecessary repeated work when the previous evidence was still valid.

If exercised compiler inputs changed, rebuild the exact integrated producer in
its isolated directory. Update producer/artifact/check definitions explicitly:

```text
python3 tools/qualification.py amend --state build/qualification.json --revision LAST_REVISION --config build/qualification-config.json --reason "reviewed integrated producer and affected closure"
python3 tools/qualification.py run-pending --state build/qualification.json --revision LAST_REVISION --base FETCHED_MAIN_SHA
```

Amendment retains prior definitions and observations; changed definitions
invalidate their evidence. Review the updated closure and integrated changes.
Do not amend a command/expected result merely to turn a failure into acceptance.

## Land the exact candidate

`start` registers the active state in `build/qualification-active.json` in its
worktree. Ordinary `landing.py verify --base FULL_BASE_SHA` automatically requires
that state's exact candidate/base assessment; an explicit `--qualification`
must name the same state. It checks the assessment before and after its ordinary
mandatory workspace-aware build checks. The receipt binds the state digest;
publish revalidates it and its source/compiler/artifact/observation identities.
Non-Rust qualified candidates retain this check too. Missing, stale or changed
evidence refuses publication. This adds no bypass to existing landing gates.

Finish initial integration/requalification before enqueueing. If the reserved
base subsequently changes, reassess the rebased candidate and rerun affected
checks plus exact candidate checks. Release when they cannot fit the remaining
180-second lease, finish outside it and rejoin. Publication still requires the
fresh reserved base, exact verified HEAD, linear history and remote ancestry.

Every run records `duration_seconds`, `repeated_work` and invalidation reasons;
assessments record comparison time and reuse/rerun counts. Landing receipts
also record build-check and total verification durations. Compare measured
repeated work and qualification cost, not commit count or an assumed compiler
speedup. Keep the active state and logs with the outcome's ordinary evidence.
Local `refs/qualification/` identities retain observed checkpoints through
amend/rebase and normal landed-worktree cleanup. They are evidence retention,
not worker branches or remote publication. Preserve producer artifacts and the
state/log directory through the ordinary confirmed backup route as well.
