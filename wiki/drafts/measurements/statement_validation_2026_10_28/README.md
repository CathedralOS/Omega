# Reuse equality closure in subtraction-bound scans

ITERATION-PERFORMANCE, fixed base
`364e3160ff039fb85161ccd46a9700be6321bf86`. Continue the
[previous iteration](../spill_summary_2026_10_28/README.md).
This is a bounded performance experiment on existing programs, not a new
language capability or fact-row completion. Work stayed in
`.agents/worktree/pass-ablation`; no new push is authorized.

## Mechanism

`subtract_floor_limited` compares each retained relation endpoint with the two
subtraction operands. `ValueComparison` reused the exact difference graph but
reconstructed finite constructor-equality closure after each failed direct
comparison. Use the existing `EqualityQuery` separately for each operand during
that scan. Every candidate is already a relation endpoint in its closure.
Separate queries avoid adding the other subtraction operand to the finite term
roster. The mandatory direct comparison remains first, and unavailable closure
is retained only within its own immutable query.

No relation is added or retained across a write, join or rebinding. The existing
storage-preservation admission, match-arm environments, recursion limit,
relation order, finite closure bounds and maximum-floor selection are unchanged.
The governing contract is [fact validity](../../../spec/terminal-psi/fact_validity.md).
The local implementation diff is [compiler.patch](compiler.patch); diagnostic scope
patches were removed before building the candidate. Diagnostic patches are
zero-context diffs against the fixed base; replay them with `git apply
--unidiff-zero`. Their reconstruction and reverse application were checked. No cache framework, new
configuration, source rewrite or special-case arithmetic rule was added.

## Decision and evidence boundary

**Inconclusive end-to-end; preserve the candidate locally for further measurement.**
The evidence
supports reduced statement-checking cost on the stack-call witness. It does
**not** establish a reliable whole-compile speedup or regression-free performance
across workloads. No PR, landing or release qualification is claimed.

The initial [diagnostic](statement-profile.json) put 1806.059 ms in local and
terminal-expression arithmetic-range scopes versus 1906.922 ms in statement
validation. The [arithmetic diagnostic](arithmetic-profile.json) then identified
1002.844 ms in mathematical-interval refinement. See [research](research.md)
for the source trace, deferred alternative and primary-source research.
Inclusive scopes overlap and may span workers; they are not additive wall time.

Four unprofiled measured observations per producer, after warmups; milliseconds,
median [minimum, maximum]. All valid observations remain included:

| Workload | Baseline whole compile | Candidate whole compile | Baseline checking | Candidate checking |
| --- | ---: | ---: | ---: | ---: |
| Stack-call other_machine | 9192 [5146, 10946] | 8909 [7023, 9417] | 3509 [2213, 4078] | 3160 [2391, 3377] |
| Length local | 3591 [2920, 3945] | 4552 [3599, 4711] | 1710 [1348, 1891] | 2151 [1842, 2374] |
| Dispatch match_join | 3621 [3171, 4033] | 4081 [3796, 4282] | 1641 [1526, 2004] | 1880 [1671, 2115] |

The smaller workloads have higher candidate medians and overlapping ranges.
Those observations are not discarded or called improvements. Other Rust builds
were active during the comparison ([snapshot](competing-builds.json)); this
checkout isolates source, not host resources. Whole-compile performance on the
smaller cases remains unresolved.

A bounded six-run ABBAAB diagnostic follow-up tests attribution, not significance
by repetition count. Baseline statement-validation scopes were 1520–1722 ms;
candidate scopes were 670–974 ms, with medians **1623.797 → 690.501 ms (57.5%)**.
The adjacent value-position-call scopes were 516–601 versus 534–769 ms: they did
not share that improvement. Checking medians were 2554 → 2116 ms, but ranges
overlap. Whole-compile medians were **5682 → 5630 ms**, with overlapping ranges;
no dependable user-visible elapsed-time gain is established.
The original profile pair likewise reduced statement validation 2100 → 955 ms
while adjacent call checking remained 825 → 802 ms. This is the basis for the
decision to preserve the local candidate; it is not promotion or a generalized
compiler-speed claim.

[Runs](runs.json), [summary](summary.json), [follow-up](phase-runs.json),
[phase summary](phase-summary.json), and [producer/source metadata](metadata.json)
retain the exact measurements. All 42 comparison/follow-up compiles succeeded,
all native executions exited 0 with empty output, and the images match the
baseline byte for byte within each subject. Both diagnostic builds also
preserved the stack-call image. Stack32 median peak job commitment in the
unprofiled comparison was 1,326,673,920 versus 1,363,232,768 bytes; no memory
saving is claimed.

## Validation

- Optimized candidate build succeeded; both producers report opt-level 1.
- Five focused equality-graph tests passed, covering bounded symbolic walks,
  cycles, overflow, constructor identities, fresh environments and finite limits
  ([output](tests.txt)). Existing tests exercise the reused helper directly;
  no implementation-mirroring test or new language fixture was needed.
- All 68 A/B behavioral observations passed: the unchanged stack32 nine-place
  row and its nine reject twins, twelve arithmetic controls and four additional
  subtraction controls. The eighteen positive row executions agree between
  Terminal Psi and native execution. Rejection exits and diagnostics agree
  between producers; additional rejection fragments match their authored oracle.
  See [results](validation.json), [output](validation.txt) and
  [exact producers](validation-producers.json). Source/build hashes and producer
  digests were checked again after execution.
- The shared [validator](../checking_simplification_2026_10_28/validate.py) now
  isolates A/B Windows source caches. The small [wrapper](validate.py) adds
  captured-result, match-subtraction and invalidated-bound controls, verifying
  their authored reject fragments as well as identical A/B diagnostics.
- Formatting and scoped whitespace checks passed. Both validation scripts parse.
  No full corpus/workspace sweep, Clippy, promotion review, pinned release
  qualification or other-host native execution was run. The candidate remains
  local and unpromoted because end-to-end performance is unresolved.

## Replay

Use an isolated checkout at the fixed base plus the candidate patch, optimized
producers bound to their digests, and new output directories. Serialize the
build/test/measurement chain with `tools/build_slot.py`:

```text
mbx build -p omega --locked --target-dir build/pass-producer
mbx nextest run -p validation --lib difference_graph::tests --target-dir build/p
python wiki/drafts/measurements/checking_simplification_2026_10_28/compare.py <baseline> <candidate> <new-comparison-dir>
python wiki/drafts/measurements/spill_loop_2026_10_28/ablate.py <baseline> <candidate> <new-phase-dir>
python wiki/drafts/measurements/statement_validation_2026_10_28/validate.py <baseline> <candidate> <new-validation-dir>
```

`build/p` is a checkout-local junction to `build/pass-producer`, retained from
the previous iteration to avoid Windows test object path limits. The historical
`ablate.py` name is reused only as a six-run A/B diagnostic harness; this
candidate disables no checking or spilling pass. Baseline reuse is supported by
an empty diff from published `2d87f18a195ec8523aeb4cdccc4ca0f793ba0e1b` to the
fixed base over compiler/library inputs, manifests, lockfile and toolchain, plus
its verified binary digest in metadata.

## Next question

The next performance question is whether the reduced checking work produces a
stable end-to-end gain when host contention is lower, including the smaller
workloads. Do not repeat unchanged trials indefinitely or claim the local scope
reduction as the elapsed-time saving. The next larger measured checking lead is
value-position-call scanning (roughly 0.57 seconds in the candidate follow-up).

The registered claim `75a2942bdb2f4b89aae220b2dad27bc6` was released after
validation. The push response timed out with an unknown outcome
([receipt](claim-release.json)); subsequent [remote read-back](claim-status.json)
confirmed the ticket absent. No registered claim remains for this iteration.
