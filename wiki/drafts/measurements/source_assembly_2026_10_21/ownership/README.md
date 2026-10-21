# Preserve source buffers across assembly

Source assembly already receives immutable `Arc<str>` text from parsing. Its
source-map registration converted that text to a `String` and then allocated a
new `Arc<str>`. `SourceMap::add_checked_instance` now accepts `Into<Arc<str>>`;
assembly retains the parser's buffer. Generated extension admission passes its
validated UTF-8 slice directly, avoiding an intermediate owned string while still
allocating the required retained text. Source IDs, checked scopes, package identity,
origin, resolution stratum and every admission check are unchanged.

The acceptance is less allocation/copy work with identical source behavior.
Whole-compiler percentage is not a stopping criterion. This is one ownership
boundary, not a claim that the whole source path allocates nothing.

## Evidence

Apple M4, macOS ARM64, pinned Rust nightly, opt-level 1. The frozen ordinary
baseline and candidate are based on `37539d2c141492d4430498a2f7f187d1ecfe864e`.
[observations.json](observations.json) records producer hashes/profiles, inputs,
commands, allocations, timings, diagnostic counts and runtime observations.

A standalone registration harness compares the old conversion with retained
buffers using the actual source-map API, on 17 files totaling 124,220 bytes.
It asserts equality of the complete maps, counts allocations separately from
timing, keeps inputs and results live with `black_box`, and destroys results
after the timed interval. Mimalloc and the disabled counter branch are present
in both timed paths; this is isolated instrumented registration, not assembly.
Saved ARM64 disassembly retains both registration loops.

| Per registration batch | Copying | Retaining |
| --- | ---: | ---: |
| Allocation/reallocation calls | 72 | 38 |
| Requested allocation bytes | 258,352 | 9,596 |
| Minimum, both execution orders | 4.541 µs | 0.416 µs |
| Mean, first / reversed order | 5.399 / 5.341 µs | 0.574 / 0.579 µs |
| Maximum, first / reversed order | 89.542 / 58.584 µs | 6,991.375 / 5,795.042 µs |

All four runs reached two seconds without a lower minimum. This removes 34
allocation calls and 248,756 requested bytes for that batch; requested bytes
are not a measurement of RSS or peak memory. The large scheduling outliers are
retained, not excluded.

Twenty alternating fresh CLI pairs with warm filesystem state measured assembly
median 5.5285 → 5.362 ms, minimum 5.001 → 4.910 ms, mean 10.7638 → 16.27655 ms,
and maximum 35.462 → 57.144 ms. Host variability is too large to attribute a
precise whole-assembly speedup to this microsecond-scale registration change.
All 40 checks exit 0; both minima reached the five-second quiet interval.
No builds from this session ran during timing. Load averages are retained.
Assembly contains lexing/parsing; their times must not be added.

## Behavior

- Before implementation at `b9dccbfe5917a741d4a013d86c0295c76c0e3413`, the new
  assembly-boundary regression fails at its first `Arc::ptr_eq` assertion.
  Incoming main changes do not touch this route or its test inputs.
- On the candidate, `mbx nextest run -p source -p package-compilation` passes
  all 16 tests. The regression establishes shared buffer identity, distinct
  Build/Product source IDs, preserved provenance/stratum, cross-scope refusal,
  and lifetime after the parser/caller owner is dropped.
- The generated-source corpus comparison against the exact base covers eight
  fixtures with **0 lost, 0 gained**. Filter:
  `build/generated_,build/nested_generated_dependency_dual_purpose,build/captured_source_generated_constant,build/dependency_generated_source_unimported`.
- Baseline and candidate both run `length/local`,
  `applied_requirement_indices_through_local_exit`, and
  `applied_requirement_computed_const_exit` with native 0 and Terminal 0.
- An extra `nested_generated_dependency_dual_purpose` run refuses on both
  producers: an existing 41-byte literal is assigned to `[u8; 38]`. This is
  a baseline refusal, not successful generated-source execution. The raw script
  assertion failure is retained rather than relabeled a green run.
- Scoped Clippy with `--all-targets -- -D warnings` stops in unchanged upstream
  `optimization_selections.rs:152` (`useless_conversion`). This dependency and
  diagnostic expression are unchanged by the source-map/assembly patch; the
  same failure is recorded in the preceding prefix-probe experiment. It is not
  a clean lint pass. Formatting and `git diff --check` pass.
- An initial corpus invocation mistakenly supplied a CLI binary as the corpus
  runner and exited 4 without testing fixtures. The corrected `--compare` run
  builds the dedicated corpus runners and supplies the eight-fixture evidence.

## Next ownership boundary

Temporary counters (removed from production) observe two static-argument pack
attempts in `length/local`, both accepted with zero expression nodes. The applied
requirement witness observes 16 pack attempts / 15 accepted, one value attempt
and one retry. The computed-constant witness observes ten accepted packs and
two accepted value attempts; successful value scratch forests contain three and
one expression nodes. These are attribution counts, not optimized timings or
counts of all parser speculation. All three checks exit 0.

Successful arguments already own their name paths and nested argument buffers.
Transfer those buffers while remapping arena-backed descendants before adding
whole-forest rollback or one arena page per probe. The latter would require
reconciling per-file page identities and every table's rollback obligations.

## Replay

Raw binaries, build/test logs, input hashes, CLI observations, runtime products,
probe patches and commands remain in `build/assembly-ownership` until archived
before worktree cleanup. The committed harness is `registration.rs.txt`; use
an ignored standalone Cargo workspace with edition 2024, dev opt-level 1/debug 1,
`source` as a path dependency and `mimalloc` with default features disabled.
Pass a list of the recorded input paths. The CLI script expects frozen
`omega-baseline` and `omega-candidate` under `build/assembly-ownership`.
[Research](research.md) records the ownership premise.
