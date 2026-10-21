# Move consumed source metadata into the assembly frontier

The source forest already adopts parser arena pages. The remaining batch join
cloned every per-source root-handle vector and cloned the path a second time,
then dropped both original buffers. `SourceStorage::extend_in_stratum` now moves
those buffers into `SourceFile`, after import discovery has borrowed the batch.
`SourceMap` still owns its separate path and shares the original immutable text.
Source IDs, scope, origin, stratum, handle identity and item order do not change.
No new index, arena, cache or source-shape exception is introduced.

Base: `30fbbc91026a740ce3e709b51e8bd7b41a1c123e`. Apple M4/macOS ARM64,
nightly 2026-09-04, optimized dev profile 1. Exact binary, file and environment
identities are in [observations.json](observations.json). The
[audit](audit.md) records the full route and the work already eliminated.

## Work removed and correctness

The standalone public prepare/assemble harness covers 289 selected module,
package and Build roots. All 218 assembled outputs and 71 discovery/parse
refusals match exactly, both when consuming a unique checkpoint and when a
second owner forces its source storage clone. Hashes cover complete syntax,
source metadata/text, Build identity, application, import bindings, generated
custody and sorted Build-scope IDs. These are assembly observations, not claims
that all programs type-check or run. Package-aware integration is separate.

Each pass removes 8,816 caller-thread allocation/reallocation calls and 995,592
requested bytes, across 226 affected roots. No input increases these counts.
For `samples/cli/facts/length/local`, it removes 40 calls and 4,363 requested
bytes. The thread-local counter excludes Rayon parser workers; the changed
registration runs on the measured calling thread. Allocation size is requested
traffic, not resident memory. Output hashing and the diagnostic capacity census
run outside the counting interval; outputs remain consumed and black-boxed.
The one-shot harness durations are not used as performance claims.

Moving the original vector retains its growth capacity until the frontier is
dropped or cloned. For the length source set, root capacity is 3,936 bytes for
2,512 useful bytes: 1,424 bytes of spare capacity, versus the old tight clones.
The largest such spare capacity in an assembled source set is 6,704 bytes.
This is a deliberate copy/allocation reduction with no RSS improvement claim.

Optimized ordinary-CLI disassembly confirms removal of the root-vector clone
and one path clone in `extend_in_stratum`. The function grows from 2,488 to
2,556 bytes; the mutable arena-span bookkeeping and moved buffers are not a
claim of smaller code. All source/root ownership remains local to compilation.

## Ordinary CLI timing

Eleven alternating pairs run fresh `omega --check --timings` processes on
length/local, with warm filesystem state, no concurrent builds from this
session, a five-second no-new-minimum window and a 120-second cap. No valid
trial is discarded. Full output, load averages and identities are retained.

| Interval | Baseline min / median / mean / max ms | Candidate min / median / mean / max ms |
| --- | --- | --- |
| Source assembly | 4.841 / 5.189 / 5.425 / 8.094 | 4.829 / 5.263 / 5.251 / 5.636 |
| Whole command | 639.066 / 646.723 / 658.990 / 764.461 | 616.332 / 646.025 / 649.715 / 710.075 |

These mixed timing results do not establish an end-to-end speedup. Retain the
change for the measured work elimination and simpler ownership handoff.
The user explicitly values small load reductions within each compiler layer.

## Validation and replay

The seven package-compilation tests pass, including source byte sharing and
independent checked scopes. The ordinary length sample reports native exit 0
and Terminal exit 0 from both frozen producers. Initial runtime attempts on
three other fixtures refuse identically: direct-file import requires normal
package-binding setup; the two dependency-generated cases contain fixed-array
literal length mismatches (41 vs 38 and 182 vs 179). Their failures are retained,
not reclassified as runtime passes or repaired inside this ownership change.

The fixture runner supplies the declared package acceptance for its own test
projects. Its six both-engine rows and ten checking rows compare against the
base with zero lost/gained outcomes. Direct-file imports execute with exit 7
in both engines; public generic imports execute with exit 0 in both engines.
The four invalid import controls reject with their expected diagnostics.
The four generated-source positives retain their baseline refusals: two fixed
byte-length errors and two generated files reaching EOF. This is unchanged
baseline evidence, not successful generated-source execution. No fixture or
oracle was rewritten. See [validation.json](validation.json) for exact rows.

Scoped Clippy (`mbx clippy -p package-compilation -- -D warnings`) exits 101 on
the unchanged dependency `optimization_selections.rs:152`, whose `digest.finish().into()`
is a useless conversion. Its source is byte-identical to the base. Scoped
rustfmt and `git diff --check` pass. The initial pre-landing fetch still named the measured base. At reservation,
main had advanced to `383ae909dafe916a0eefb87919ee345043567edf` through two
fixture/documentation commits. The reservation was released for assessment.
Their new float sample and slice fixtures are outside every measured selection;
compiler, library, harness and consumed source inputs are unchanged. They add
no library imports affecting the package census. Reuse preserves the original
producer identities; the rebased commit still receives exact-SHA verification.

For replay, recreate `build/import-assembly/harness/{Cargo.toml,src/main.rs}`
from the adjacent text files, and use the original `inputs.txt` and frozen
binaries from the raw archive. `compare.py` captures baseline, applies the
ownership change, rebuilds, compares exact outputs and counts; `measure_cli.py`
compares ordinary executables. The complete raw directory is preserved outside
the managed worktree before cleanup.

The next bounded question is repeated custody snapshots and duplicate-path
queue allocation. Shared-checkpoint syntax cloning remains a separate ownership
problem; this result does not claim source assembly is exhausted.
