# Empty-requirement shortcut in value-call checking

ITERATION-PERFORMANCE; fixed base `ba621694ff483f2b66c2e8343aac4d8b24098581`.
Continue [statement validation](../statement_validation_2026_10_28/README.md), now
submitted as [PR 750](https://github.com/CathedralOS/Omega/pull/750). That PR's
compiler check passed on its submitted head. This iteration stays local in the
same isolated worktree and does not update or enqueue that PR.

## Decision

**Inconclusive; remove the candidate from production source.** The six-run
ABBAAB screen does not establish a repeatable reduction in value-call scanning.
Do not retain a new fast path on the strength of overlapping timing ranges.
The candidate remains reproducible in [candidate.patch](candidate.patch).
No compiler speedup, diagnostic parity, general contract acceptance, release
qualification or new language capability is claimed.

## Measured mechanism

The unchanged representative is
`samples/cli/facts/thirtytwo_arg_stack_calling_plan/other_machine/main.omg`.
Its exact source, build, producer and image identities are in the records below.

The [first diagnostic](scanner-profile.json) measured 517.653 ms in call-bound
validation out of 578.157 ms in value-position scanning. The [second
diagnostic](detail-profile.json) measured 484.386 ms in
`reject_refuted_value_call_requires` out of 536.922 ms in call bounds. Argument
checking was 44.674 ms; constant-range call handling was 0.134 ms. These are
inclusive scopes, not independent contributions to process wall time.
Both diagnostic compiles produced the same PE image and native exit 0.

The candidate gathered the same ordered `requires` expression list before
preparing argument terms and a closed-operand structural judge. An empty list
returned immediately; nonempty lists retained all existing judgments and
diagnostics. The surrounding call validation remained unconditional. The
existing caller is `validate_value_call_arguments`; its arity, type, access,
formation and later obligation checks were untouched. This is a bounded
performance experiment, not a repair of a demonstrated language defect.

See [research](research.md) for the source trace, applicable earlier research,
prediction and rejection criteria. Governing semantics remain
[invocation contracts](../../../spec/language/state_contracts.md).

## Comparison

Three observations per producer, interleaved ABBAAB, all with phase profiling;
milliseconds, median [minimum, maximum]. No valid observations excluded.

| Measurement | Baseline A | Candidate B |
| --- | ---: | ---: |
| Value-position scanning | 697.609 [630.754, 754.241] | 665.262 [644.526, 667.592] |
| Checking | 2501.219 [2258.562, 2604.454] | 2317.785 [2198.958, 2480.715] |
| Whole compile | 5580.739 [5416.715, 5622.070] | 5453.772 [4959.677, 5539.288] |

The scanner median difference is 4.6%, and whole-compile medians differ by 2.3%.
Ranges overlap in all three measurements. These observations do not isolate a
reliable benefit; neither a speedup nor a regression is established. Peak job
commitment medians were 1,345,134,592 versus 1,368,850,432 bytes; no memory saving
is claimed. A process snapshot during the build observed our own Rust compiler;
it does not establish a quiet host during measurements. Source isolation is not
resource isolation, and no competing workload is asserted as the cause of noise.

All six compiler exits and native exits were 0. All six PE images were
byte-identical, 114176 bytes, SHA-256
`55adb749405f33a9dcefcadb5a0c3f165452d1a8f05993f11f30ce8114aeb8b8`.
Source/build hashes and both compiler digests were verified again after the run.
This proves the representative's observed result and code identity, not general
semantic equivalence of the shortcut.

## Replay and evidence

- [Metadata](metadata.json): exact producers, optimized profiles, fixed source
  base, toolchain, target, build command and exit status.
- [Raw observations](screen-runs.json), [summary](summary.json),
  [console output](screen.txt). The existing
  [ABBAAB harness](../spill_loop_2026_10_28/ablate.py) and
  [profile runner](../spill_loop_2026_10_28/profile.py) create fresh projects and
  Windows source caches for every observation and perform ordinary package
  review before the measured compile. Historical harness names do not imply
  that this experiment changes spilling.
- [First instrumentation](diagnostic.patch), [detailed instrumentation](detail.patch),
  and [candidate](candidate.patch) are independent zero-context patches against
  the fixed base. Replay with `git apply --unidiff-zero`. Instrumentation was
  removed before building B. Candidate reverse application and application
  checks passed; the working compiler source was restored exactly to the base.
- Optimized build and changed-file `rustfmt --check --edition 2024` passed.
  No new test was added to production: the new branch was removed. The planned
  call-contract controls and three-subject unprofiled comparison were not run
  after this screen failed to justify retaining it. No full corpus, Clippy,
  other-host run, independent review or landing is claimed.

The retained baseline at `build/statement-loop/candidate/omega.exe` remains the
correct producer for the restored source. The warm `build/pass-producer` and
`build/call-scan-loop/candidate/omega.exe` still hold the experimental binary;
do not mistake either for a producer of the restored source.

Next question: split the measured refutation cost between argument-term
construction, structural-judge preparation and actual requirement judgments.
Only then select another change. Do not repeat this empty-list experiment
unchanged or introduce a broad proof cache without exact input/lifetime evidence.

Claim `f0e57b42f1ec4b71a0c4a661f6f6e155` was released after the experiment;
registry read-back confirmed no remaining claim with that ticket.
