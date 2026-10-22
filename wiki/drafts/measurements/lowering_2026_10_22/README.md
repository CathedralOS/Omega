# Conformance query scope in lowering, 2026-10-22

Retained: lower conformance applications from one owner-filtered census. The
previous entrance discovered every live source occurrence to ask whether any
emitted owner had an application, then repeated the entire census to retain
those applications. The candidate narrows completed arena candidates by exact
owner before paying for liveness, and passes the result into retention once.

This does not change which source is live: discarded arena applications are
still intersected with the existing live-source traversal. Specializations,
occurrences and trait-argument applications retain their original lane and arena
order. If candidates exist, traversal still covers the whole retained source
roster, including uncalled bodies and non-machine declarations. No cache,
checker bypass or source-shape acceptance path was added. The superseded
second census and always-empty exclusion argument were removed.

Baseline: `b428c347b51`, built in this isolated checkout with the pinned nightly,
optimized dev profile 1. Candidate is that base plus the two-file implementation
in this commit. `timings.json` binds the exact binary digests, profile, Rust
version, Apple M4/macOS ARM64 host, source/library hashes and implementation
patch digest. `instrumentation.patch` contains temporary attribution probes;
it is not part of the candidate Rust source. `attribution.txt` shows the full
source census costing 0.602 ms out of 6.265 ms lowering in one exploratory run.
That exploratory observation is not the A/B baseline.

## Measured result

Sixteen alternating pairs per program, fresh ordinary CLI processes with warm
filesystem/build state. All 64 commands succeeded. No observation was excluded.
Own builds were stopped during measurement; other workstation activity continued.
One-minute load average ranged from 11.55 to 17.86. This is a shared-host latency
comparison, not a throughput or cross-platform claim.

| Workload | Baseline lowering min / median / max, ms | Candidate lowering min / median / max, ms | Median reduction |
| --- | --- | --- | --- |
| `length/local` | 1.882 / 2.381 / 6.350 | 1.540 / 1.882 / 5.661 | 20.9% |
| `open_const_implementation/local` | 2.775 / 2.924 / 3.145 | 2.369 / 2.396 / 5.499 | 18.0% |

The candidate is faster in 13/16 length pairs and 15/16 implementation pairs;
median paired savings are 0.475 and 0.526 ms. The candidate's implementation
maximum is worse; that valid slow observation is retained. The result supports
lowering work reduction, not a universal tail-latency improvement.

Whole-compile medians are 1543.850 → 1526.893 ms and 744.186 → 736.299 ms.
The broad ranges and background load do not establish a reliable end-to-end
speedup. Checking dominates these workloads. Every emitted native image has
exactly the same SHA-256 within each subject across both producers and all
rounds. Raw observations and descriptive statistics are in `timings.json` and
`timing-summary.json`.

## Correctness and limitations

`validation.json` contains 126 observations, grouped into 63 baseline/candidate
comparisons: native/Terminal execution of all nine places of both fact rows and
three trait consumers (21 programs), canonical Terminal artifact bytes for those
21, the 18 fact-sensitive reject twins, and three implementation-argument controls. Every execution has actual native
and interpreter exit 0, every Terminal digest matches, and every reject has the
same nonzero exit and exact diagnostic. The only normalized text is the host
scratch directory printed by `--keep`, outside guest streams. Inputs are frozen
and hashed; no witness, oracle or golden was edited.

The captured-region fixture names `Main::main`, unlike the other subjects. An
initial standalone inspection with `main` failed identically on both producers;
the corrected inspections are the accepted artifact comparisons. Initial raw
probes are retained in `inspection-wrong-machine.json`. A comparison script also
initially treated the per-process `--keep` scratch path as a diagnostic change;
normalizing that line reuses the actual observations without relabeling them.

Affected package build and Clippy complete; Clippy reports existing warnings.
Touched-file rustfmt and `git diff --check` pass. Package nextest runs 26 tests:
24 pass, including all four existing conformance occurrence tests; two lowering
source audits fail on 14 baseline violations. `audit-attribution.json` binds
the unchanged audit definition and each unchanged violating file to their base
hashes. The changed retention source introduces none of their forbidden patterns.
This is source-reader attribution, not a claim of a fresh baseline test run.

The scoped corpus runtime gate executes 21 programs, all native/Terminal exit 0.
It reports 19 newly observed golden rows (exit 1, movement), with no lost successes
or oracle failures; those goldens remain unchanged. The check gate covers 42 fixtures: 21 checked and 21 rejected, all expected
rejection fragments present. Its two diagnostic movements are against old golden
rows with no stored diagnostics; frozen baseline and candidate CLI observations
match exactly for both controls. Full raw corpus reports and
check results accompany this record. No execution on other hosts is claimed.

## Replay and next question

1. At the base, `mbx build -p omega`; copy `target/debug/omega` to
   `build/lowering-performance/omega-baseline`.
2. Apply this commit's two Rust changes, build the same package, and preserve
   `build/lowering-performance/omega-candidate`. Save the Rust diff as
   `build/lowering-performance/candidate.patch`.
3. From the checkout root, run `python3
   wiki/drafts/measurements/lowering_2026_10_22/measure.py` and `validate.py`.
   The latter may re-summarize saved observations with `--summarize`.
4. Corpus commands and their exact filter are retained in `checks.json`.

This bounded iteration removes measured repeated discovery while preserving
behavior. It does not establish that lowering is saturated. The next question
is whether closure discovery, shared catalogs and body emission can consume
retained checked relationships directly instead of rewalking and copying them.
The attribution places body emission above the conformance census; larger
representative closures and operation/allocation counts are needed before
choosing that redesign. See `research.md` for the primary-source lead and its
limits.

The subsequent [design review](design.md) uses a larger packet workload and
ARM64 disassembly to prioritize the next experiment. Its attribution places
leaf-copy reconciliation plus membership propagation ahead of body emission;
it does not change the completed wave's implementation or A/B claims.
