# Reuse interval equality closure

Bounded continuation of [pass ablation](../pass_ablation_2026_10_28/README.md),
for ITERATION-PERFORMANCE. Fixed base:
`7635e6e61287a9e2c67fb5a85b5debbc064a0b7d`.
The target is the existing stack-call fact's slow source-to-native compile;
this is not a new fact row or closure of a language capability.

## Mechanism and decision

`refine_place_interval` compared a subject against retained relation endpoints
with `ValueComparison`. Failed direct comparisons reconstructed the same finite
constructor-equality closure. The candidate uses the existing `EqualityQuery`
once per immutable relation scan. Closure construction already collects every
relation endpoint and its children, so adding the subject covers this scan.
The query and its unavailable-result cache die at the end of the invocation.

This changes neither premise construction nor write/join invalidation. Exact
operand identities, constructor rules, ordering edges, overflow checks and
finite search limits remain owned by the existing equality implementation.
The relevant contract is [fact validity](../../../spec/terminal-psi/fact_validity.md).
No diagnostic profiling instrumentation or rejected type-query rewrite remains.

**Retain the optimization.** All focused behavioral controls passed.
Stack-call compile wall time fell **40.8%** and
checking time fell **61.8%**. Each candidate wall-time observation was faster
than every baseline observation for that workload. The two smaller cases have
higher candidate medians with overlapping ranges; they establish no benefit.

Four unprofiled measured observations per producer; median [minimum, maximum],
in milliseconds:

| Workload | A wall | B wall | A checking | B checking |
| --- | ---: | ---: | ---: | ---: |
| Stack-call `other_machine` | 8923 [8524, 9918] | 5280 [4772, 5490] | 5963 [5627, 6755] | 2279 [2203, 2537] |
| Length `local` | 2334 [2178, 4034] | 2460 [2232, 3070] | 1094 [1084, 1861] | 1173 [1074, 1308] |
| Dispatch `match_join` | 2154 [2135, 2331] | 2242 [2125, 2433] | 1092 [1064, 1153] | 1100 [1066, 1171] |

Stack-call median peak job commitment is essentially unchanged: 1.259 versus
1.258 GiB. All 36 warmup/measured/profile compiles succeeded and their native
programs exited 0. Within each workload, all twelve executable hashes match.
Raw observations and unrounded summaries are in
[runs.json](evidence/isolated/runs.json) and [summary.json](evidence/summary.json).

Separate profiles check the same 414 machines and 1,154 statement nodes.
Statement-node validation falls from 6334.655 to 1480.698 ms inclusive.
Both load five source candidates, 17,354,488 preparation bytes and 9,638,525
source bytes; checked-source records are 63,678,713 and 63,678,825 bytes.
This fixes the large cache-size mismatch in the earlier experiment. Profile
scopes overlap and may span parallel workers; do not sum them into wall time.

## Comparison protocol

Windows x86-64, optimized dev compiler (`opt-level=1`), pinned Rust
`1.100.0-nightly (a69a63265 2026-09-03)`. Retained producers:

- A, fixed-base compiler: `64983946d8bccabbfa18efcc9a25e8339ff8a5aa6cc297d044b1454bcda4308d`.
- B, candidate runtime source: `767a0684e0f99c63e16c87f086dca27b6f4e3135f535dff56a8fc93df590c667`.

The final [metadata](evidence/isolated/metadata.json) binds source/Build hashes,
producer digests and verified optimization profiles. Its compiler patch includes
the tests; the additional finite-limit test was added after the CLI build and
compiled in the test binary, without changing any CLI runtime source.

[compare.py](compare.py) uses the existing benchmark runner. Each observation
gets identical source in a fresh project and a fresh process-local Windows
source-cache base. Ordinary package review runs before timing. Per workload:
one A/B warmup, eight measured runs in ABBAABBA order, then separate A/B profiles.
The harness requires nonempty successful compiler exit, actual published output,
fresh checking work, native exit 0 and identical native bytes across producers.
It verifies source, Build and producer hashes remain unchanged.

Timing excludes package-review setup and runtime execution. Peak memory is the
Windows job's peak committed bytes, not physical working set. Other applications
remained active; ranges and individual observations are retained. These are
three local examples, not a general compiler speedup or statistical confidence
interval.

## Correctness

The five focused equality tests cover exact graph behavior with cycles and
overflow, authenticated constructor identity, environment replacement, 64
deterministic endpoint-query comparisons, and parity near finite term/comparison
limits in both relation orders. Direct proofs still work after closure
exhaustion. This is bounded differential evidence, not a proof of identical
search precision for every possible finite-budget graph ordering.

[validate.py](validate.py) compares the unchanged nine stack-call positives
through native and Terminal execution, their nine fact-sensitive rejection
twins, and twelve existing arithmetic checking controls on both producers.
The controls exercise wrapping/saturating interpretation, casts, short-circuit
paths, mutation and private calls. All **60 observations passed**: 18 paired
native/Terminal executions, 18 fact-twin refusals, and 24 arithmetic checks.
Baseline and candidate exits and error lines agree for every case. The six
additional negative controls also match their authored `expected.txt` fragments
on both producers. The [oracle audit](evidence/validation/oracle-audit.json)
verifies source, Build and retained producer hashes after execution.
No sample, oracle, or golden is edited.

Crate Clippy completed with 220 warnings and no errors; scoped Rust formatting,
Python syntax checking and source/document `git diff --check` passed. Raw patch
context and diagnostic-log whitespace is preserved in the evidence; an
unfiltered whitespace check flags those captured lines. No whole corpus,
whole workspace, other-host execution or release qualification was run.

## Rejected experiments and measurement repairs

- Replacing arithmetic meaning analysis with the existing type query did not
  establish a useful improvement; reverted. Its partial results were also
  exposed to severe unrelated memory pressure and do not establish a slowdown.
- The first same-project run reused checked results. Its checking timings are
  excluded; output agreement remains observed.
- Fresh projects alone did not isolate accumulated source graphs. The first
  closure profiles loaded 324,261,921 versus 63,583,753 checked-source bytes.
  Both checked 414 machines and 1,154 statements, but elapsed/memory differences
  are confounded. The [shared-cache run](evidence/shared-cache/runs.json) is
  retained as diagnostic/output evidence, not the final speed claim.
- An unrelated test consumed about 199 GB committed memory and exited before
  the authorized stop reached it. Fresh measurements began after memory
  recovered. Later ComfyUI activity and available memory are recorded separately;
  no interactive application was stopped for the benchmark.
- The initial correctness harness tried to approve an intentionally invalid
  package. It stopped at the expected arithmetic refusal before recording that
  control. The repaired harness checks invalid controls directly; the failed
  harness log and its two completed runtime observations are retained.
- After 40 completed observations, an explicit standard-library dependency
  exposed a Windows bug in the existing benchmark helper: `write_text` changed
  generated LF review framing to CRLF, which the compiler correctly refused.
  Discarding and regenerating the proposal reproduced it, ruling out the initial
  stale-state hypothesis. The helper now reads UTF-8 and writes UTF-8 with explicit
  LF, changing only decision tokens. The new byte-level regression fails with
  the original helper and passes with the repair; all six review-settlement
  tests pass. The real previously failing review succeeds as well. This
  independent workflow repair is local commit `128412f345d`. It affects untimed
  review setup, not compiler runtime source or the retained timing observations.
  Validation resumed only unfinished cases after verifying prior source and
  producer hashes. The interrupted aggregate log stays failed; the
  [resume log](evidence/validation-resume.log), complete results and oracle audit
  establish final success without relabeling that failure.

[Research and hypothesis history](research.md) links the primary source and
prior experiment. Raw transcripts, patches, successful and failed gate logs,
and machine-readable observations are under [evidence](evidence/).

## Replay

Use separately retained exact A/B producers. The recorded run wraps the full
local chain in one build-slot hold; individual replay commands are:

```powershell
python tools/build_slot.py -- python wiki/drafts/measurements/checking_simplification_2026_10_28/compare.py <baseline.exe> <candidate.exe> <new-output-directory>
python tools/build_slot.py -- python wiki/drafts/measurements/checking_simplification_2026_10_28/validate.py <baseline.exe> <candidate.exe> <new-validation-directory>
```

The scripts use existing authored source and ordinary package review. The
comparison creates private cache directories beneath its new output directory;
it never deletes the user's cache. Retained local binaries and build artifacts
remain under `build/checking-simplification` and `build/pass-producer`.
An additional digest-verified copy of B is retained at
`build/checking-simplification/candidate/omega.exe`.

## Next question

In the candidate's separate stack-call profile, native register allocation
costs 1504.316 ms, including 627.175 ms in bounded spilling across 3300 calls.
Checking still costs 2326.213 ms, including 966.964 ms summed over flow-state
statement scopes. The next experiment should distinguish repeated allocation
work from necessary spill decisions before changing a heuristic. These are
profile leads, not another measured optimization or an instruction to start an
unbounded campaign. This iteration stops after its focused acceptance and local
result commit; publication is not authorized.

Follow-up: the owner subsequently authorized publication as PR #740, which
merged as `2d87f18a195ec8523aeb4cdccc4ca0f793ba0e1b` after source and queue CI.
The [next bounded iteration](../spill_loop_2026_10_28/README.md) profiled spill
slot assignment and rejected disabling reuse because of its stack cost.
