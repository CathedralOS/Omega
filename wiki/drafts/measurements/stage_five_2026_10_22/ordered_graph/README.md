# Stage 05: compact ordering graphs within a comparison batch

2026-10-22. **Retained.** Reuse an exact, compact difference graph across sibling
and recursive symbolic equality queries against one immutable relation set.
The final candidate reduces Stage 05 median by 5.35% on `length/local` and
2.59% on `dependent_subtract/local`. This finishes a bounded cold-check experiment;
it does not close CHECKED-PACKAGE-REUSE or claim a fully optimized Stage 05.

## Premise and design

The [preceding design review](../design_review.md) proposed measuring reuse first.
A temporary check-entry probe found no previous Product judgment on either input:
length had Build 34 machines / previous 0 and Product 123 / previous 0;
vector_distance had Build 34 / previous 0, Build 34 / previous 1, and Product
1,347 / previous 0. The latter then refused its existing package admission:
`macos_arm64::ProgramEntry` needed an accepted program-entry binding. That input
was not modified and supplies no successful performance comparison. The raw
[length](length.stderr) and [vector](vector.stderr) observations correct the reuse
premise; machine-visit counts alone never established redundant judgments.
The owner selected reducing cold work with compact symbolic graphs.

The old ordering walk repeatedly collected structurally equal operands and
searched recursive operand trees during edge relaxation. A temporary probe at
`ordered_gaps_by` entry counted 69,939 walks and 729,621 relation entries summed
on length/local ([output](order-profile.stderr)). Its inclusive 159.587 ms timer
contains nested scopes and instrumentation overhead: it is not removable wall
time. These counts include the outer congruence-aware walk as well as exact
leaf walks. They must not be compared one-for-one with the new exact graph rows.

Hypothesis: build exact endpoint identities once, relax numeric edge indices,
and compute each queried source row once within a comparison batch. Reject if
query outcomes change or ordinary CLI timings fail to show useful improvement.
The implementation is deliberately limited to the measured relation family:

- `ValueComparison` borrows the live immutable relation slice. Constructor
  congruence retains its depth bound, primitive/domain identity, resolved handle
  generations and exact effectful call occurrences.
- Its lazily built `DifferenceGraph` borrows operands and stores contiguous
  scratch edges with integer indices. Recursive structural equality remains at
  graph construction and endpoint lookup, outside numeric relaxation.
- Source rows are computed lazily and shared by child/sibling queries. Callers
  retain one comparison for their relation scan or atom-pair batch. Nothing
  survives a write, join, substitution, or new environment.
- Preserve edge order, in-place updates, the original vertex-count-minus-one
  round limit, and checked i64 addition. Even `Unequal` endpoints count toward
  the round limit, but never supply an ordering edge. This preserves precision
  on contradictory cycles and overflow, rather than substituting an idealized
  shortest-path algorithm.
- Literal-bound queries first reject unusable nonliteral or out-of-i64 operands
  before requesting equality. They cannot contribute a bound in either version.

This is temporary query scratch, not a new durable IR or competing place identity.
Worst-case retained rows use O(V²) memory within a batch; endpoint construction
still performs structural comparisons. Peak memory was not measured. No global
term arena, persistent cache, SIMD, or hardware bandwidth claim is introduced.

## Final ordinary measurements

Apple M4, macOS arm64, pinned nightly 2026-09-04, dev **opt-level 1**. Fresh CLI
processes, warm filesystem, unchanged sources, `omega --check --timings`.
Alternate baseline/candidate order per pair; retain every valid run. At least
12 pairs, then stop after both Stage 05 minima remain unchanged for ten seconds,
with a cap of 50 pairs. Both workloads converged. These are successful real
compiler outputs, not a dead-code-eliminable synthetic arithmetic loop.

| Workload / metric | Baseline min / median / mean / max, ms | Candidate min / median / mean / max, ms |
| --- | --- | --- |
| length/local Stage 05 | 567.313 / 585.776 / 599.618 / 707.775 | 532.451 / 554.424 / 563.035 / 687.154 |
| length/local total | 694.444 / 716.045 / 736.212 / 877.040 | 656.899 / 687.780 / 700.210 / 837.999 |
| dependent_subtract/local Stage 05 | 556.846 / 571.982 / 573.284 / 592.583 | 537.333 / 557.196 / 559.064 / 605.233 |
| dependent_subtract/local total | 692.013 / 705.875 / 706.932 / 725.931 | 674.133 / 686.769 / 691.773 / 747.877 |

Length: 28 pairs, candidate Stage 05 faster in all 28, median paired saving
32.185 ms. Total median reduced 3.95%. Subtract: 16 pairs, candidate Stage 05
faster in 13, total median reduced 2.71%. Raw records include stdout/stderr,
load averages, verified optimization profiles, executable hashes, source hash,
patch hash and toolchain: [length](final-length-timings.json),
[subtract](final-subtract-timings.json).

Own builds and timings were serialized. Other sessions were building and running
corpus checks on this shared host; no applications were closed or desktop state
changed. The recorded spread is part of the result. A stabilized minimum is a
bounded stopping rule, not statistical proof or a hardware ceiling. These
measurements do not establish release-profile, Windows, large-program, memory,
or cross-process reuse gains. The earlier exploratory candidate is not pooled
with these final-source observations.

Three separately [profiled pairs](final-phase-timings.json) each report 4,260
graphs, 26,752 vertices summed, 32,172 edges summed and 4,453 computed source rows
for the candidate. Optional profiling counters take a shared lock; instrumented
latencies are perturbed and are not used for the speedup claim. Existing outer
ordering walks remain. The counted waste supports the mechanism but gives no
numeric ceiling on whole-stage savings.

The [actual arm64 assembly](difference-graph-arm64.asm) retains the numeric
relaxation loop: indexed loads/stores, `adds` with overflow tested before update,
and no recursive Operand equality in that loop. Structural equality remains
in endpoint lookup. This confirms the intended code exists, not a specific CPU
throughput or cache limit.

## Behavior and validation

- [Eight validation crate tests passed](final-tests.log). Three new tests compare
  against the previous symbolic walk on 256 deterministic graphs (cycles,
  Unequal edges, missing endpoints, i64 extremes, repeated/reversed queries),
  retain call occurrence and generation distinctions, and prevent equality
  leaking across changed environments. This is an independent representation
  oracle, not a test that merely restates the compact implementation.
- [Checked corpus comparison](corpus-checked.json): 49 fixtures, 0 lost / 0 gained
  against `4533fd644482`. [Candidate records](corpus-checked-0.txt) contain 21
  checked positives and 28 rejects with expected diagnostics; [baseline records](corpus-checked-1.txt)
  retain the corresponding raw observations. Coverage includes constructor
  captures, effectful epochs, snapshot invalidation, congruence false twins,
  missing argument bounds and the length fact row.
- [Native/Terminal comparison](corpus-comparison.json): 21 fixtures, 0 lost /
  0 gained. The [baseline rows](corpus-both-baseline.txt) show 20 native/interpreter
  exit-0 agreements and one check-only proof fixture. `--both` selects positives;
  the separate checked comparison above supplies reject evidence.
- [Public CLI observations](public-length.json): all nine length transport places
  produced native exit 0 and interpreter exit 0; all nine invalid twins rejected
  with their authored `rejection_contains` diagnostic. Sources/oracles unchanged.
- Optimized CLI build passed ([log](final-build.log)). Scoped Clippy completed
  with warnings ([log](clippy-warnings.log), trailing whitespace trimmed); strict `-D warnings` stopped at an
  unchanged `optimization-selections.rs:152` useless conversion ([log](clippy.log)).
  The warning in touched `delivered.rs:96` is also unchanged (`&arguments`).
  No warning points to introduced code. Affected-file rustfmt check and
  `git diff --check` passed. No full corpus or foreign-host execution is claimed.

The semantic boundary remains the existing
[fact validity contract](../../../../../wiki/spec/terminal-psi/fact_validity.md)
and [mathematical values contract](../../../../../wiki/spec/terminal-psi/mathematical_values.md):
exact observations retain meaning, changed storage invalidates facts, and
constructor equality does not manufacture its own premises. The immutable borrow
bounds cache validity; the old algorithm oracle pins its precision choices.

## Identity and replay

Baseline compiler source `b428c347b51786c5f57d577d59413977d6cf5d9f`; integrated
experiment base `4533fd64448238e0218b10c676648c3e1cd2f56d` adds documentation only.
Baseline executable SHA-256:
`7d324fb6afb876d146fb219975e5590bb0dffb4f8f3b6a2fcb457b3f479a633c`.
Final candidate executable SHA-256:
`7236e1c400a8c2b68eea7a27242f677631c0bdd77ed22c93ebf51839c9291b27`.
The decompressed [implementation patch](implementation.patch.gz) hashes to
`67d0438d819b63bf3582ad759ddc381a6a4e75d3edeee4730633e8b2721b89e3`.
[Manifest](manifest.json) binds source files, runner, workload roots/builds and
diagnostic probe binaries. The temporary probes are removed; their outputs are
diagnostic evidence, not the production candidate or a performance denominator.

From the repository root, build each revision separately with `mbx build -p omega`
and preserve its executable. The timing runner verifies each binary's profile.

```sh
python3 wiki/drafts/measurements/stage_five_2026_10_22/state_lookup/measure.py \
  --baseline /absolute/baseline --candidate /absolute/candidate \
  --source samples/cli/facts/length/local/main.omg --pairs 12 \
  --minimum-stable-seconds 10 --maximum-pairs 50 --output /tmp/length.json
# Repeat with samples/cli/facts/dependent_subtract/local/main.omg.
mbx nextest run -p validation
mbx clippy -p validation --all-targets
```

Corpus command (repeat with `--both` for execution comparison):

```sh
OMEGA_CORPUS_JOBS=2 python3 tools/corpus_gate.py --compare 4533fd644482 \
  --filter 'facts/length_,proofs/proof_congruence_add_constant,proofs/congruence_false_twin,proofs/scalar_field_snapshot_,calls/constructor_child_,calls/runtime_constructor_child_,calls/effectful_result_outside_captured_requirement,calls/completed_effectful_result_requirement_exit,calls/ordered_bound_requires_argument_rejected,collections/runtime_ordered_computed_selector_reads_exit,slices/runtime_ordered_nested_length_observation_exit'
```

For public samples, run `omega run --both` on each of the nine
`samples/cli/facts/length/<place>/main.omg` roots and `omega --check` on its
`invalid/main.omg`, requiring the expected diagnostic in `invalid/sample.json`.

The next useful question is whether observation construction and environment
copying still dominate on larger admitted programs after this exact leaf work
is removed. The current gain justifies retaining this bounded change; it does
not justify a general term-arena rewrite without allocation and workload evidence.
Research provenance is in [research.md](research.md).
