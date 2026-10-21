# Skip impossible applied-requirement probes

The name-expression parser constructed a scratch syntax forest and parsed a
type before every ordinary name. A name-led generic type must continue its
first identifier with `<` or `::`; other following tokens cannot produce the
generic receiver required by an applied-requirement call. Checking that necessary
prefix first avoids work while preserving the original type parser and ambiguity
rules for possible calls. It does not recognize a particular library or sample.

The scoped acceptance is to remove these impossible probes, demonstrate less
parser work and assembly time, and preserve syntax, diagnostics and affected
execution. A small whole-program fraction is not a stopping criterion. This
experiment does not establish a floor for the rest of source assembly.

## Mechanism and machine code

The 17 ingress files contain 124,220 bytes and 24,496 tokens. A diagnostic-only
counter found 1,982 probes, of which 1,456 fail the necessary prefix. The retained
change allocates no scratch forest for those cases. Possible generic receivers,
qualified names and comparison ambiguities still take the original parser.

| Work per traversal | Baseline | Candidate |
| --- | ---: | ---: |
| Allocation/reallocation calls | 39,597 | 23,469 |
| Requested bytes, including reallocations | 7,138,110 | 6,252,697 |
| Full syntax digest | `9b8394d928b3ba35` | `9b8394d928b3ba35` |

An empty syntax forest alone requires nine allocations and 152 requested bytes.
The saved ARM64 disassembly shows why the probe is a separate `inline(never)`
function: a guard inside the original function still reserved and touched five
4 KiB stack pages before testing the prefix. The final guard has an 80-byte frame
and reaches that scratch frame only after the prefix succeeds. No separate timing
win is claimed for this frame change; it removes observed stack traffic.

## Repetition results

Apple M4, macOS ARM64, optimized dev profile (opt-level 1). Baseline source is
`00ef417922c04aea560b7faa5ce05999eb1f0512`; the candidate is that source plus the
19-line `primary.rs` change. Source and producer hashes, actual build profiles,
toolchain, inputs, and all CLI observations are in [observations.json](observations.json).

The isolated harness lexes actual source-backed tokens before timing, keeps
parser outputs live with input/output `black_box`, and destroys results after
timing. Full syntax hashing and allocation counting occur separately. The TLS
counter's disabled branch remains in allocation paths during timing, so these
microseconds are instrumented; the ordinary CLI below corroborates the gain
without that wrapper. Disassembly confirmed parser calls remain inside the loop.
Each run continues until three seconds without a lower minimum, with a 45-second
cap; all reported runs reached the quiet interval. Minima expose low-interference
cost, while averages and maxima retain host variability.

| Parser run | Trials | Minimum µs | Average µs | Maximum µs | Minor faults |
| --- | ---: | ---: | ---: | ---: | ---: |
| Baseline, first order | 1,495 | 5,534.875 | 5,703.558 | 6,473.125 | 174 |
| Prefix guard | 2,794 | 2,482.625 | 2,587.630 | 2,995.250 | 157 |
| Prefix guard, reversed order | 1,229 | 2,479.042 | 2,612.009 | 3,809.666 | 156 |
| Baseline, reversed order | 1,198 | 5,505.833 | 5,702.343 | 6,811.750 | 186 |
| Final separated probe | 2,775 | 2,532.875 | 2,720.353 | 4,053.125 | 137 |

These parser inputs exclude loading, lexing and the generated Build prelude.
Eleven alternating pairs of fresh ordinary CLI processes check `length/local`
with warm filesystem state and no concurrent builds. Both assembly minima went
five seconds without improvement. Every command exited 0.

| CLI scope | Baseline min / mean / max ms | Candidate min / mean / max ms | Baseline / candidate median ms |
| --- | --- | --- | --- |
| Source assembly | 6.652 / 6.884 / 7.312 | 4.635 / 4.759 / 4.940 | 6.830 / 4.747 |
| Stage 02 parsing | 4.596 / 4.800 / 5.156 | 2.545 / 2.660 / 2.829 | 4.781 / 2.635 |
| Total elapsed | 713.461 / 722.091 / 734.718 | 712.957 / 718.393 / 740.138 | 719.594 / 716.177 |

Assembly's median falls 30.5%. Inclusive assembly and Stage 02 overlap; do not
add their savings. Total-time spread does not support a precise whole-compiler
speedup promise. These results establish this workload and host, not release
profile performance or other architectures.

## Behavioral validation

- Full `Debug` syntax/error SHA256s match for all 18,951 fixture/library inputs:
  18,883 parsed and 68 rejected. This compares parsing, not compilation. The
  baseline and final TSV hashes are identical in the observation record.
- Scoped corpus comparison checks 68 fixtures: 25 checked, 43 rejected, no
  outcome changes, no missing expected diagnostic fragments, no invalid acceptances.
  Filter: `applied_requirement,contextual_static_machine,static_machine_argument,runtime_nested_generic_instantiations_exit,length_through_local_exit`.
  A temporary golden records the frozen baseline runner; the candidate compares
  to that temporary file. No repository golden is changed.
- Actual candidate `run --both --keep` observes native 0 and Terminal 0 for
  `samples/cli/facts/length/local/main.omg`,
  `tests/omega/pass/facts/applied_requirement_indices_through_local_exit/main.omg`,
  and `tests/omega/pass/providers/applied_requirement_computed_const_exit/main.omg`.
- An extra direct run of `runtime_nested_generic_instantiations_exit` refuses
  package setup for an unaccepted `MacosApplication` entry binding on both frozen
  producers. It is not counted as runtime success or a semantic reject control.
- `mbx nextest run -p tokens-to-syntax-trees`: 263 passed, seven failed.
  Focused unchanged-baseline reruns reproduce all seven: generic binder admission,
  iterative expression prefixes, opposite Boolean arms, public compare-exchange,
  unrelated Boolean guards, fixed-array equation operands, and the sample ignore
  file check. These are retained baseline failures, not a green suite.
- `rustfmt --check --edition 2024` on the edited file and `git diff --check` pass.
  Scoped Clippy with `--all-targets -- -D warnings` exits 101 on both baseline and
  candidate at unchanged dependency `optimization_selections.rs:152`
  (`clippy::useless_conversion`); it cannot establish a clean parser lint pass.

## Replay and retained evidence

The scratch workspace `build/assembly-perf` contains frozen producers, complete
CLI stderr, parser TSVs, build logs and validation logs. The final source differs
only in the guard and extracted probe; diagnostic counters are removed.
The adjacent repetition and parse-record Rust sources are saved as `.rs.txt`
to avoid introducing another production workspace. To replay, create an ignored
standalone Cargo workspace with edition 2024, dev opt-level 1/debug 1, dependencies
`mimalloc` (default features disabled), `libc`, `sha2`, and path dependencies on
`source`, `syntax-trees`, `tokens-to-syntax-trees`, `source-files-to-tokens`.
Copy the saved sources to `src/main.rs` and `src/bin/parse-record.rs`.
Write an absolute path list from `observations.json`'s `inputs`, build through
`mbx build --manifest-path`, and run the repetition binary with `3 <list>`.
Freeze separate baseline/candidate binaries before timing; never mutate source
while a build runs. `parse-record <all-source-list>` emits full syntax/error
hashes for comparison on a 256 MiB stack matching the product.

The adjacent CLI script expects frozen `build/assembly-perf/omega-{baseline,candidate}`
and writes complete observations there. Corpus replay uses frozen runners with
`OMEGA_CORPUS_JOBS=2 python3 tools/corpus_gate.py --runner <runner> --filter <above>
--golden <temporary-reference>`, adding `--record` only for the baseline.
The [research record](research.md) distinguishes the general prefix-dispatch
technique from Omega's own grammar premise.

## Integration onto current main

Fifteen incoming commits moved main to `59dd468cd329d408775cd7f7c8c594d95c5e1894`.
They change component/backend/runtime and corpus plumbing, but no parser, lexer,
syntax representation, foundation, library input or the three execution witnesses.
Cargo.lock changes only backend dependencies. The isolated parser evidence and
baseline failure attribution retain their original producer identities; these
unchanged consumers support reuse. Full CLI/corpus producers were rebuilt on the
new base both without and with the same parser patch. The measured candidate
`d3b690eb5262e2f267ce67e05d88f6065e2a58a0` differs from the final commit only in
measurement documentation. Source and binary hashes, build profiles, commands,
exits and all fourteen pairs are in [integrated-observations.json](integrated-observations.json).

| Integrated CLI scope | Baseline min / mean / max ms | Candidate min / mean / max ms | Baseline / candidate median ms |
| --- | --- | --- | --- |
| Source assembly | 6.732 / 7.151 / 9.988 | 4.542 / 4.692 / 4.886 | 6.956 / 4.703 |
| Stage 02 parsing | 4.644 / 4.910 / 5.957 | 2.478 / 2.608 / 2.713 | 4.814 / 2.617 |
| Total elapsed | 720.763 / 733.831 / 853.477 | 721.398 / 726.431 / 734.088 | 724.605 / 724.604 |

All 28 checks exited 0 and both minima reached the five-second quiet interval.
Assembly median decreases 32.4%; total median is effectively unchanged in this
batch. This is evidence of the per-layer reduction, not a whole-program claim.
Fresh baseline/candidate corpus results preserve all 68 outcomes (25 checked,
43 rejected) and diagnostic expectations. All three integrated native and
Terminal witnesses again exit 0. Rebased parser tests repeat 263 passes and the
identical seven failures; Clippy repeats the same dependency error and formatting
passes. No expectation or validation gate is weakened.

The first integrated corpus recording attempt exited 2 because HEAD was an
unpublished candidate. Its raw results remain in the scratch record. The retry
followed the recorder's prescribed route: keep the candidate staged at the
published base, record the frozen baseline, compare the frozen candidate, then
restore candidate HEAD. All 68 rows therefore have the inspectable published-base
stamp; the separate producer manifest distinguishes their actual binaries.

Before removing the worktree, raw harnesses, frozen producers, input locks,
outputs and landing receipts are retained in the primary checkout's ignored
`build/measurement-archives/source-assembly-2026-10-21-prefix-probe.tar.gz`.
