# Shared scalar namespace during lowering, 2026-10-22

Retain one immutable scalar value-type table per descriptor-machine adaptation.
The previous implementation rebuilt that identical table for every payload
expression. The candidate binds the table to its borrowed machine in
`TerminalScalarExpressions`; graph construction, expression expansion and
independent validation still execute normally.

This continues the bounded experiment proposed in
`../lowering_2026_10_22/design.md`. It delivers a lowering improvement, not a
claim that developer-visible whole-command latency has improved. Broader
ITERATION-PERFORMANCE acceptance remains open.

## Why this change

The packet program's approximately 43 ms copy-processing interval was mostly
descriptor-graph setup. Its propagation loop ran once, visited 262 operations,
proposed 17 contracts, and updated none. This rules out a propagation worklist
as the first fix for this subject.

The first implementation deferred graph creation until a real query, but a
shared-reference history query still demanded the graph. Sixteen alternating
pairs showed no packet improvement. It was rejected and removed; its patch,
raw timings and refusal are retained under `lazy_graph/`. The initial inference
that the graph was unused was wrong: the first counters measured time and
updated rows, not query demand.

Further attribution and emitted ARM64 code revealed repeated scalar type-table
construction. Preparing it once changes the lifetime of immutable setup and
preserves the same collection order, last-entry-wins behavior for duplicate
identities, literal recovery, source-definition search, proof checks, failure
behavior, and depth/work budgets. The namespace belongs to one machine and
cannot outlive its immutable borrow. No persistent cache, worklist, additional
hash map, conditional checker bypass or source-shape path was added.

## Measurement

Base: `60cb9c9da7f`, whose Rust source equals the previous retained implementation
at `0047571a3f1`. The frozen baseline executable is the prior wave's candidate;
its source applicability follows from that exact Rust equality, not binary age.
Candidate: that base plus this commit's three Rust files under
`terminal-psi/semantics/src/descriptor_results/`.

Both producers use the pinned optimized dev profile (opt-level 1), on Apple M4,
macOS ARM64. Binary digests, build profiles, toolchain, source/library hashes,
implementation-patch digest, commands and raw outputs are in `timings.json`.
The machine remained in normal interactive use; own builds were stopped during
measurement. Recorded one-minute load averages span 6.20–12.40.

Sixteen alternating pairs per workload, 128 successful compilations, no excluded
observations. Each invocation is a fresh CLI process with warm filesystem/build
state. Milliseconds below refer to `terminal-production/lowering`, not the
historical Stage 05 checking timer.

| Program | Baseline min / median / max / mean | Candidate min / median / max / mean |
| --- | --- | --- |
| segmented packet checksum | 70.135 / 71.660 / 116.186 / 76.784 | 55.416 / 57.669 / 64.800 / 58.518 |
| length/local | 1.415 / 1.503 / 2.852 / 1.578 | 1.389 / 1.450 / 2.873 / 1.646 |
| open_const_implementation/local | 2.343 / 2.473 / 2.578 / 2.463 | 2.337 / 2.491 / 2.876 / 2.525 |
| copied range record domains | 6.990 / 7.193 / 10.026 / 7.489 | 6.715 / 6.920 / 7.312 / 6.943 |

Packet lowering median decreases **19.5%**; every paired observation is faster,
with a median paired saving of 14.5665 ms. The copied-range fixture improves
in 14/16 pairs (median paired saving 0.354 ms). Small-witness changes are minor
and mixed; length's candidate mean and maximum are worse. All observations,
including those slow candidates, remain in the record.

Whole-packet compilation medians are **945.382 → 950.311 ms**, and means are
956.139 → 951.497 ms. This does not establish a whole-command speedup. Lowering
is about 7.6% of baseline whole-compile median time here; the isolated saving is
only about 1.5% of that command. Checking and host variation dominate the outer
measurement. There is no hardware-throughput or cross-host claim.

Every emitted native image is byte-identical within each workload across both
producers and all timing repetitions. `assembly.json` confirms that table
construction moved from the per-query routine to the graph-adaptation entry,
while expression expansion remains emitted. These are real compiler invocations
producing consumed artifacts, not a removable microbenchmark loop.

## Quality and replay

The evidence ledger in `checks.json` and `validation.json` records scoped checks
and baseline/candidate behavior. No source sample, expected result, package
admission or golden is changed by this experiment. The additional design probes
are compile-only; actual engine claims come from the separate runtime checks.

- All 23 `terminal-semantics` tests pass. The package build, Clippy (32 warnings),
  touched-file rustfmt and `git diff --check` pass.
- Nineteen programs execute with native/Terminal exit 0 on both producers and
  have identical canonical Terminal artifacts. These cover all nine
  `as_slice_views` places, the packet program, both small timing witnesses,
  copied record domains, borrowed mutation windows, returned references,
  qualified scalar copies, store joins and empty qualified arrays.
- Nineteen invalid controls reject identically, with every expected diagnostic
  fragment present. The positive execution/artifact comparisons and negative
  controls establish 57 successful comparisons.
- One additional positive fixture, `runtime_nested_array_range_record_domains_exit`,
  fails before lowering on both binaries with the same four default-domain
  requirements for its entry-context fields. Its two comparisons remain in the
  record as baseline refusals, contributing no runtime or artifact evidence.
- The scoped corpus `--both` gate builds and executes six fixtures, all
  native/Terminal exit 0. It exits 1 for two newly observed golden rows, with
  no lost successes, wrong exits or output-oracle failures. Both new rows also
  pass the frozen baseline CLI comparison; no golden is rewritten.

There are 118 raw validation observations: 114 for the 57 acceptance comparisons
and four for the two baseline-refusal comparisons. The first summarizer stopped
on that baseline failure and initially treated a multiline expected.txt as one
fragment. Its corrected summary uses the same observations, labels the refusal
explicitly, and checks each nonempty expected line, matching the corpus rule.
No failure was relabeled as a successful execution. Full-baseline and other-host
validation are outside this bounded change.

For replay, build the base with `mbx build -p omega` and preserve
`build/lowering-propagation/omega-baseline`. Apply this commit's three Rust
changes, rebuild, and preserve `omega-candidate` in the same directory. Save
the implementation diff as `candidate.patch`, then run `measure.py` and
`validate.py` from the repository root. Preserve historical results before a new
capture. The rejected candidate uses its separately preserved patch/executable;
it must not be confused with the retained producer.

See `research.md` for the decision sequence and primary-source applicability.
Temporary probes are preserved as patches with binary-bound raw observations;
none remains in production. This iteration stops at the demonstrated shared
namespace saving. Remaining expression lookup/expansion and body-emission costs
need separate attribution before another implementation is selected.
