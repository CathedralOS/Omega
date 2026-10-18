# Agent handoff: spec coverage

Start with the [coverage map](spec_coverage_map.md) and its
[structured inventory](spec_coverage_map.json). This is the saved requirements
and evidence inventory for assigning future work. Repository instructions and
the current accepted spec govern implementation.

## What is saved

The audit is pinned to source revision
`9a0775cf7b893963d4d1441b8135c966315301d6`. It inventories 98 spec documents,
744 headings, 1,024 overlapping requirement families, and the existing fact and
test candidates. Family IDs identify review-sized obligations; they are not
1,024 independent tasks or a count of proved requirements.

The Markdown is the human entry point. The JSON retains source hashes, heading
lines, family IDs, fact places, control paths, candidate evidence and remaining
uncertainty. All families still need qualification at that audit revision.
The separately recorded historical execution trial has its own source and
producer identities and is excluded from current coverage counts.

The saved repository branch is `docs/spec-coverage-walk`. Preserve the exact
commit supplied with an assignment. An agent can fetch that branch and read
these files without relying on this chat or its temporary `build/` files.

Validate the saved checkout from any working directory:

```sh
python3 /path/to/Omega/wiki/drafts/audits/spec_coverage_map_validate.py
```

The validator checks the inventory against the files in that checkout. A new
documentation commit may sit above the audit source revision. Changed spec or
fixture hashes are reported as drift. To reproduce the saved inventory, use
its saved commit; to implement a requirement, fetch current main and reconcile
the relevant source and existing evidence before selecting work.

## Assign complete, small facts

Choose an actionable parent outcome from the current task board, then identify
the exact family IDs and normative source for the next fact. The map's six
suggested slices are starting points; verify whether later main already
delivered them. Inspect existing sources before adding a witness. A passing
fact can be qualified without a compiler repair.

Give one agent the complete fact, including its nine places and rejecting
twins when transport is applicable. Keep the fact, bound and consuming
operation fixed. Parallel agents should own distinct facts selected by root
cause. Do not divide one row into nine independent assignments or create a
combinatorial matrix. Follow current claim, worktree, testing and publication
instructions to coordinate overlapping compiler edits.

Artifact decoding, proof rules, ABI, Build execution, installation and hardware
requirements also need evidence from their actual consumers. A nine-place
source row alone does not discharge those obligations.

Use this assignment template:

```text
Parent outcome and remaining acceptance: <current task ID and exact scope>
Requirement families: <IDs from spec_coverage_map.json>
Current normative source: <paths, sections and fetched main revision>
Fact: <one sentence naming the subject, bound and consuming operation>
Existing witness/control candidates: <paths, inspected before selection>
Required places or consumer: <nine-place row, or exact appropriate consumer>
Observable oracle: <payload/output/writeback/proof or admission verdict>
Reject twin: <single necessary premise broken; exact intended diagnostic/site>
Targets and engines: <required combinations; unavailable ones stay visible>
Acceptance: <positive, negative, identity/lifetime and composition checks>
Publication authority and stopping condition: <state explicitly>

Read AGENTS.md and applicable skills from current fetched main. Complete this
bounded assignment, including necessary general compiler/library repairs and
authorized integration. Preserve the original fact and oracle. A currently
passing witness requires no invented defect. Report the exact requirement
removed and the parent's remaining acceptance.
```

## Evidence required to claim coverage

- Establishment: the original fact reaches the original consuming operation.
- Identity and lifetime: another subject, stale storage, expired loan or changed
  arrival cannot supply that fact.
- Composition: every required place and incoming path preserves the same fact;
  calls, state arrivals, cycles and cleanup retain their independent duties.
- Soundness: the intended twin rejects for the intended reason, and relevant
  verifier or artifact corruption controls remain rejecting.

Runtime claims require an observable authored oracle on the required engines.
Check-only acceptance, a runtime header, file presence, README claims and a
green unrelated queue do not establish that result. Keep producer identity,
compiler/library inputs, source revision, target, command, result and evidence
paths with each qualification. Preserve the distinction between a mathematical
proof, an admitted premise and an executed test.

Before marking a row complete, inspect whether its oracle exercises the
consumer and whether its twin removes only the necessary fact. Where useful,
deliberately corrupt the expected result to confirm that the oracle fails.
Do not weaken a contract or substitute literal input merely to make a place
pass. A compiler fix must preserve the original operation and remain general.

Record each completed qualification in the existing fact README and claim
note, naming its family IDs. A useful final report includes:

```text
Parent outcome / family IDs:
Exact fact, bound and operation:
Source revision / producer and library identity:
Target / engine / command / observed result:
Reject control / exact diagnostic / failing operation:
Evidence paths:
Compiler or library repair required:
Integrated revision and publication verification, when authorized:
Requirements established / remaining acceptance / unavailable evidence:
```

## Keep the snapshot and new results distinguishable

Individual agents should retain their new evidence with their fact. Refresh
the aggregate map after integrating that evidence against one source revision;
concurrent workers should not independently rewrite this whole snapshot.
Existing task boards remain the work queue.

The map's reconciliation notes include conflicting spec wording, stale status
claims, legacy scalar-range witnesses and unattributed `length` twins. Read
the exact current sources before acting on those findings. Follow the
repository's owner-decision process for a genuine semantic conflict; neither
this audit nor compiler behavior decides it.

Saving this packet authorizes no agent launch or additional compiler work by
itself. Each future assignment supplies its own bounded goal and publication
scope.
