# Retained syntax ownership — 2026-10-22

Retain the implementation. Authored names carry a source-page handle and their
original full-width span; the syntax forest owns each source page once. Name
copies no longer increment a source Arc. Generated spellings own their distinct
bytes and move on consumption. Decoded literals share the lexer's byte buffer and
range through later representations, eliminating the token-to-syntax payload copy.
No hash-based name interning or global text registry is introduced.

Clone retains each page once; adoption moves pages; cross-forest copying retains
them before copying handles. Namespace collection records paths and source
ownership together behind private buffers. This closes an early constant-selection
omission found by corpus replay. Complete syntax equality resolves names through
both owners and compares all stored rows and arena metadata, preserving Build
continuation-sharing rules. The [review](REVIEW.md) records those boundaries.

## Storage and allocation traffic

All 463 parser inputs have identical complete logical observations, including
public snapshots and parse diagnostics. The Debug adapter resolves each actual
name through its owner and removes only new ownership metadata. Detached nodes
and pre-existing arena fields remain compared. Source text and tokens are built
before the parser allocation interval, so these numbers exclude their storage,
including the decoded buffer now retained by literal views. Complete assembly
measurement below includes that work.

| Parser quantity, summed across inputs | Baseline | Candidate |
| --- | ---: | ---: |
| Bytes per Identifier | 72 | 48 |
| Retained syntax source references | 100,353 | 766 |
| Allocation requests | 272,037 | 230,629 |
| Total requested bytes | 83,895,983 | 66,440,408 |
| Live requested bytes | 38,987,329 | 30,692,680 |
| Sum of per-input peak live requested bytes | 39,106,025 | 30,799,053 |

These are requested heap bytes, not allocator-resident RAM or a simultaneous
whole-workload peak. Floating-point SourceText values still retain source references.
Generated-name clones copy their boxed spelling; their cost is included.
A literal view can retain its entire source-local decoded buffer. Copy-on-write
protects existing views if a token stream is subsequently appended to.

All 289 assembly inputs match in unique, shared-checkpoint and all-worker modes:
complete syntax, source maps, scoped bindings, generated-source custody and errors.
All-worker counts include parallel parsing; calling-thread counts are separate.

| All-worker assembly quantity, summed across inputs | Baseline | Candidate |
| --- | ---: | ---: |
| Allocation requests | 7,456,768 | 6,179,196 |
| Requested bytes | 2,782,627,385 | 2,356,433,922 |

For `length/local`, allocation requests change from
34,540 to 28,619, and requested
bytes from 12,970,623 to 11,006,636.
The final `take_identifier` machine code has no atomic increment; baseline contains
`ldadd`. [Assembly identities](assembly-identities.json) bind the excerpts to the
actual executables. The harness consumes outputs and black-boxes measured results.

## Timing

Final full-pipeline baseline: `f99397375a26b565fb9b493d8cc6f523901e6591`. Optimized ordinary CLI profile 1,
Apple M4/macOS ARM64, fresh processes with warm filesystem state, 19
alternating AB/BA pairs. Both use the same explicit library root. No owned build
or validation workload overlaps the timing batch. All observations, process CPU
time, page faults, peak RSS, load averages and binary profiles are retained in
[cli-observations.json](cli-observations.json). No slow observation is discarded.

Assembly wall-step milliseconds:

| Producer | Minimum | Median | Mean | Maximum |
| --- | ---: | ---: | ---: | ---: |
| baseline | 4.8520 | 5.0250 | 5.1114 | 6.2070 |
| candidate | 4.7210 | 4.9390 | 5.5759 | 14.1160 |

Assembly median changes by 1.71% downward; candidate is faster in
11/19 pairs. Minima and means remain visible: a small median change
does not establish a universal latency gain. Stage 02 aggregate time is retained
separately and is not substituted for the wall step.

Whole-compile milliseconds:

| Producer | Minimum | Median | Mean | Maximum |
| --- | ---: | ---: | ---: | ---: |
| baseline | 649.2390 | 661.4630 | 662.0546 | 678.3020 |
| candidate | 644.4740 | 656.9400 | 661.9738 | 735.3370 |

The storage/work reduction is established independently of whole-compile timing.
Do not extrapolate the assembly ratio to the entire compiler or other machines.
The earlier `8bd3d573211` batch had assembly medians 5.1435 / 4.921 ms and
whole-compile medians 657.9255 / 657.749 ms. Historical observations remain in the
local archive and are not pooled with this batch.

## Validation and integration

- The 570-test affected suite at `972eb3d68bd74a4b5d52c2b2045778f6383d0830` has 563 passes and seven failures
  reproduced on a freshly rebuilt baseline with identical diagnostics. Names and
  attribution are in [baseline-failure-comparison.json](baseline-failure-comparison.json).
  Expectations were not changed. The focused namespace regression passes.
- Final integration changes only Stage 05 match-join descriptor emission. The
  parser dependency closure, executed assembly path, input bytes and source library
  are unchanged. Their observations remain applicable; both
  CLI/corpus producers are rebuilt, and lowering plus affected reverse-dependency
  tests, Clippy, runtime and
  paired timings are rerun at the final base. [Integration assessment](integration-assessment.json)
  and [runtime integration commands](runtime-integration-commands.json) record that scope.
- The additional lowering suite has four passes and two source-architecture
  guard failures, reproduced with identical diagnostics on current main.
  The five affected dependent packages pass their refreshed tests.
  [Lowering attribution](incoming-failure-comparison.json) records the comparison.
- Scoped Clippy and compiler checks pass. The warning comparison finds no new
  warning; existing warnings are attributed rather than called a clean baseline.
- Canonical corpus comparison has no losses or gains for 13 checked fixtures and
  eight both-engine fixtures. Complete recorded diagnostics, statuses and engine
  facts match the freshly rebuilt baseline. Four generated-source positives already
  refuse on baseline; they remain explicit refusals, not passing tests.
- Four corpus positives execute in both engines with matching results: one exits
  7 as authored; three exit 0. `length/local` exits 0 with `run --both`. The CLI
  dependency-declaration Build query passes on both producers; two generated-source
  CLI checks refuse identically. See [runtime validation](runtime-validation.json)
  and the adjacent raw observations.

The integrated base and changed production-file hashes identify the measured
candidate independently of its later commit ID. Parser/assembly producers were
frozen at `972eb3d68bd74a4b5d52c2b2045778f6383d0830`. The CLI/corpus producers use the final integrated base; the retained parser/assembly
producers use the unchanged parser/assembly code paths and input bytes described above. Normal exact-SHA
landing verification and remote source-hash verification bind publication to this
code. This is macOS execution evidence; no Windows/Linux runtime claim is made.

## Replay and retained evidence

The `harness-*/` and `syntax-probe-*/` sources have `.txt` suffixes so snapshots do
not become production workspaces. Restore one side under
`build/retained-syntax-ownership/`, remove those suffixes, and use
`mbx build --manifest-path build/retained-syntax-ownership/<harness>/Cargo.toml`.
Use the baseline revision for baseline sources and the published candidate for
candidate sources. Restore `normalize_debug.rs.txt` as `normalize_debug.rs` for
candidate captures. Freeze binaries before comparing. The scripts, input hashes,
per-input output digests and source hashes preserve the comparison.

The timing capture uses macOS `wait4` resource accounting. The portable phase-timing
entrypoint on Windows and macOS is
`mbx run -p omega -- --check --timings samples/cli/facts/length/local/main.omg`;
omit macOS resource metrics when replaying elsewhere.

Full parser observations, raw build/check logs, historical captures and frozen
executables are archived at
`/Volumes/Crucial/CathedralOS/Omega/build/measurement-archives/retained-syntax-ownership-2026-10-22/`.
[Research](research.md) records the hypothesis and primary-source comparison;
no external benchmark result is used as evidence for this compiler.
