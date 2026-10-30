# Checked corpus recording

The historical full run recorded all **21,104 fixtures discovered at that source**, with unique keys and exact
coverage, from compiler source `ccdb99ca2214b93c0e2535a31cb34a3e2167d9dd` over
frozen base `66fc7ac3c97bfb21af2f79f90fa96757495e5bb1`. The result is
**`recorded_regression` (exit 3)**. This records failures as failures; it is not
an all-green compiler qualification.

The optimized Linux x86-64 producer has SHA-256
`a8850a2324327070e746cae0fd5f91ab1527c795f0be3c95f1fe26114cab3463`.
Its verified compiler/runner profile is opt-level 1. The host has a four-CPU
quota and measurements use `OMEGA_CORPUS_JOBS=4`. All 260 disjoint primary
captures and seventeen prescribed isolated timeout rechecks are retained.
Each initial timeout was rechecked once with the original limits; the later
whole observation was selected even when it still failed. No timeout or oracle
was relaxed. Three obsolete keys were already absent from the frozen base;
980 current fixtures were missing from the old golden.

## Repairs

Refreshing the original `4ca6a6a3ddc7` checkout to the frozen base resolved the
unchanged numeric operand-order stack-overflow reproducer. Local repairs then:

- Share completed callee-relative reference-slot summaries within one inference
  request, keyed by state and the exact parameter-value snapshot. Active
  recursion is checked before lookup; incomplete results are never cached;
  caller substitution and output rebasing still happen after lookup. This
  removes repeated expansion of the same finite call graph. The unchanged
  firmware witness timed out at 120,000 ms before the repair and checked in
  18,800 ms in this recording. These are workflow observations, not a controlled
  historical timing benchmark.
- Bind placed-operation seals to the authenticated satisfied callable schema,
  rather than its later projected lookup schema. Operation, access, permission,
  destructive-take and backing-geometry checks remain. The mixed-backing control
  now reaches its unchanged geometry oracle.
- Form two existing unapplied-provider-owner controls with mandatory empty
  `reaches` ceilings and zero-argument hosted entry wrappers. Their invalid
  owner selections and native-rejection assertions remain unchanged. Three
  test-only optional-entry API uses were also updated.

## Verification and limits

| Check | Observed result |
| --- | --- |
| Complete checked corpus | 9,625 checked; 11,418 rejected; 51 incomplete; 10 timed out |
| Selected Linux native/Terminal witnesses | 35 built, both exits 0, interpreter agrees; comparison gate exit 1 from golden movement |
| Selected rejection controls | 39 reject; 38 match authored fragments; gate exit 3 |
| Strict native-rejection oracles | All seven match |
| Affected crate tests | `validation`: 29 passed; `terminal-psi-to-abstract-operations`: 18 passed, two existing qualification-only skips |
| Strict Clippy | Existing failures: 174 in the validation check and eight in the native-admission check, outside changed lines; no suppressions |
| Integrity | Canonical rows equal raw selected observations; no missing/extra keys, reason-free rows, compiler-crash evidence or custody ancestry errors |

The snapshot retains **80 accepted invalid controls, 581 wrong-reason rejections,
1,256 outcomes contradicting their tier, and 1,018 retained lost-success records**.
Six authored Terminal-artifact promises remain unmet. These are visible debt,
not passing results. The full corpus run is the checked leg; actual native and
Terminal execution is scoped to the 35 witnesses above. Other hosts and release
qualification were not run.

The private-unused control's earlier generic-specialization refusal and a
separate placed callback lowering failure reproduce on pre-repair source;
they do not establish the intended private-access/callback acceptance.

A frozen, independently built pre-repair producer measured **4,475 cases**:
every semantic move plus every retained off-target/wrong-reason/artifact-oracle
failure. No successful baseline case became unsuccessful, and no invalid control
became newly accepted relative to that measured baseline. All 80 accepted invalid
controls and all ten persistent timeouts also occur on the baseline.

Raw comparison has 4,298 equal outcomes and 177 differences. Review accounts for:
153 bijective path-derived package/specialization diagnostic identity changes;
19 optimizer fingerprint changes; one module-local structural type coordinate;
the placed-seal repair; two fixture-formation repairs; and one baseline timeout.
No raw diagnostic or golden field was normalized away. Package identities hash
canonical source lineage; structural coordinates are assigned in sorted identity
order. The optimizer probe holds fixture/library context fixed and reproduces
all 19 baseline records exactly, including recorded fingerprint prefixes and
rollback/module-change facts. Its metadata explicitly distinguishes the supplied
`ccdb` compiler from the `66fc` fixture/library checkout; it does not stamp or
relabel a golden row.

**Baseline limit:** `pass/build/uefi_os_handoff_invocation` times out in both its
batch and one isolated follow-up. The candidate reaches the exact rejection in
the prior golden (missing closed native catalog identities). Fixture source and
the refusal branch are unchanged, but a fresh matching baseline rejection was
not observed within the original budget. This limit remains explicit.

## Retained evidence and replay

[The SHA-256 manifest](evidence-manifest.json) inventories every raw verdict,
log, producer/profile record, comparison, review disposition and scoped test
record in [the compressed archive](raw-evidence.tar.gz). Archive round-trip hashes
were verified. Extract it at a repository root to inspect its original `build/`
paths. `record.json`, `diff-review.json`, `attribution.json`,
`attribution-review.json`, `difference-dispositions.json` and
`optimizer-context-review.json` are the principal records. Commands, filters,
working directories and fresh versus retained observations are explicit there.

The source bundles preserve **tree-identical privacy-safe snapshots** of both
measured source checkpoints. GitHub rejected the original publication with
`GH007` because the author metadata used a private email. The original measured
commits and original gate bundles remain local. Public snapshots use the account's
verified public noreply identity; the manifest binds each original observed
commit to its exact source-tree hash and the corresponding public snapshot.
Original row stamps are unchanged. This is an explicit publication limit: the
source code is inspectable and tree-identical, while original author-bearing
commit objects are not published. No snapshot is represented as a fresh run.

Both bundles require the published base above. Restore the compiler source code
from the privacy-safe snapshot with:

```sh
git bundle unbundle wiki/drafts/measurements/golden_record_2026_10_29/source-tree-8071544e46181b137f347efab4435dbb8a93ed7f.bundle
git worktree add --detach .agents/worktree/replay-golden 5ecab7cf5519689a74ef2c5ffb1b50f4c5539fe7
```

Build a fresh optimized producer in that restored worktree and run the archived
`tools/corpus_gate.py` filters with fresh output paths. Runner binaries are not
included. Commit/version provenance strings may differ when rebuilding the
privacy-safe snapshot; do not substitute its commit ID for the actual measured
source stamp or claim historical capture reuse as fresh execution. A focused
runtime replay is:

```sh
python3 tools/build_slot.py -- python3 tools/corpus_gate.py \
  --filter pass/build/runtime_firmware_map_cycle_exit,pass/calls/runtime_numeric_operand_order_exit \
  --both --json build/replay.json
```

## Integration with main

The completed full recording above is historical: its producer is `ccdb99ca2214`,
not the subsequently integrated compiler. PR #889's conflict was resolved against
main `2e9f231f798bffee7a2bd46857d02adb79de70b6`. The only content conflict was
`tests/omega/corpus_outcomes.txt`; compiler code merged without a content conflict.

The live checked table is a three-way integration, **not a new whole-corpus run**.
It preserves 625 incoming-main observations that were new or changed relative to
`66fc7ac3c97b`, plus 20,820 qualified historical observations: 21,445 rows total.
Incoming-main changes win whenever both sides changed a row. All per-row source
stamps, diagnostics, timing and lost-success markers are preserved. Twenty-one
inactive keys were removed; none contained a changed incoming-main observation.
Current discovery has 21,487 fixture roots, leaving 42 new fixtures absent from
both input tables unmeasured. They are listed in the resolution receipt rather
than assigned invented results. The original complete 21,104-row recording remains
in the historical evidence archive.

Fresh focused qualification uses integrated source
`febc01cd55000e3a34d7df732fc16868283b7ee7` and its own optimized producer. Its
source bundle and separate integration manifest/archive retain the exact source,
three golden inputs, conflict decisions, commands, logs, raw observations and
hashes. This source checkpoint precedes the subsequent incoming-main integration described
below. Runtime observations name this measured checkpoint, not the final head. It is distinct
from both the historical recording source and the final publication commit.


Fresh integrated-source results:

- `mbx nextest run -p validation`: 47 passed, none skipped.
- `mbx nextest run -p terminal-psi-to-abstract-operations`: 18 passed, two existing skips.
- `mbx check --locked -p omega`: passed.
- `corpus_gate.py --both` with the recorded positive filter: 35 native/Terminal
  witnesses built, exited 0 in both engines and agreed. The unchanged numeric
  operand-order crash reproducer is among them. Gate exit 1 (`moved`) reflects
  runtime success observations improving checked-only rows, not a runtime failure.
- The recorded controls filter: 39 rejected, 38 matched their expected diagnostic.
  The same private-unused generic-specialization mismatch remains. Gate exit 1
  (`moved`) is retained in the raw evidence rather than called a clean comparison.
- The seven strict native-rejection oracles all matched: gate `clean`, exit 0.

The fresh optimized producer SHA-256 is
`03a0621e5139a7fabbbe09a618dd531f2e3896683a0d45fe503d6629712439b9`.
All fixture execution used `OMEGA_CORPUS_JOBS=4` with the existing time limits.
No full-corpus or Clippy rerun is claimed for this integration. No compiler,
fixture or library edits were needed to resolve the content conflict.
The integration archive's `integration-review.json` gives the complete selected
observations; `golden-resolution.json` lists every retained, incoming, removed
and unmeasured key. Restore the exact source bundle into a clone containing the
integration base, then use `qualify.py` and the captured commands to replay.

### Subsequent incoming integration

While those checks ran, main advanced to
`e8e2fe90b14b2a7bad0e714ba635161d80c9f585` and introduced two additional golden
rows. `incoming-observations.json` retains those exact records and source stamps.
Git still reported a real content conflict, so the final commit was rebased onto
that main revision and the recorded table was restored with both incoming rows.

The final live table has **21,447 rows**: 627 incoming observations and 20,820
historical recorded observations. The main roster has 21,489 roots; the same
42 listed roots remain unmeasured. The original integration archive and its
21,445-row receipt remain immutable records of the first resolution. The
follow-up receipt supplies the final table digest and source-difference list.

Compared with qualified checkpoint `febc01cd5500`, incoming main changes two
borrow-call checking files and introduces one positive/control fixture pair.
It retires completed alias reads before terminal transfer while retaining
conflict checking for live observations. The task's summary cache, schema seal,
fixture-formation repairs and tools are unchanged. Static review found no
conflicting authored hunk: completed summaries still precede caller substitution,
and placed operation admission retains all permission/backing checks. Required
GitHub CI checks the final integrated compiler. The 65 unit-test passes and 35
runtime successes above are observations of the explicitly named pre-follow-up
checkpoint; no execution of the final upstream-integrated compiler is claimed
by those author results. No full-suite rerun was performed for this later
incoming-only integration.
