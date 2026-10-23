# Current-main integration of resolved storage changes

The normalization and transactional-rebase changes integrate cleanly with current
main, including the other session's parser-copy reduction. All affected integration
checks pass on the combined compiler; no production adjustment was necessary.

Measured compiler source: `886df83704dd8c4153ec36c92f3fa6c1ad518142`.
Integrated main: `595562a958b4d026652d51f35d4f921220252e34`.
Target: macOS arm64. Optimized producer SHA-256:
`eac28419cd65c879890059b5f35019e9e4a665ff808ac1b2ff52fc98b9fc8af0` (opt-level 1).
This record preserves the measured source separately from later evidence-only
commits and any dependency-equivalent publication rebase.

## Fresh evidence

- All 25 representation tests pass, including late-store rebase rejection and
  within-table normalization equality controls.
- Five standalone native programs exit 0; the same five pass native/Terminal
  agreement with both engine exits observed.
- Five invalid controls retain the previous candidate's exact cleaned diagnostics.
- All five native images are byte-identical to the previous validated candidate.
- Pinned qualification completes with exit 0 and no input/artifact invalidations.
  The conservative dependency closure covers compiler sources, bundled library,
  manifests/toolchain, the ten consumers, the historical output oracles, validator,
  frozen optimized compiler, generated native images and accepted project locks.

The subjects are length, dynamic dispatch, nested record, returned placed view,
and forwarded placed view; refusals cover wrong backing, overlapping loans, exact
policy ownership, source-authority upgrade, and the invalid length bound. No sample,
library, language contract or oracle is changed. The earlier full-carrier oracles
remain historical evidence, not reruns relabeled as current-main results.

[Summary](summary.json), [tests](tests.log), [producer receipt](candidate-producer.json),
[compile/setup observations](setup.json), [runtime/rejection observations](validation.json),
[qualification summary](qualification-run.log), [raw validator output](qualification.stdout),
and [compressed immutable qualification receipt](qualification-receipt.json.gz).
The receipt's uncompressed SHA-256 is recorded in the summary. The full active
qualification state and build transcripts are preserved with the replay artifacts.

## Integration and setup history

Main first integrated at `ba0bef6091e403e7fd9329636b2fd09c4edd18b7`.
The 25 tests passed and an optimized compiler was frozen after a 19m20s build.
A parser change landed during that build, so the compiler was rebuilt on the
newer integrated base before runtime qualification. That warm rebuild took
42.69s. These are build observations, not compiler-runtime speedups.

The first qualification setup listed a nonexistent `.cargo` directory. The runner
refused before executing the validator. The configuration was amended to remove
that absent dependency; no real input, diagnostic or expected output changed.
The [initial configuration](qualification-config-initial.json),
[refusal](qualification-run-initial.log) and [amendment](qualification-amend.log)
are retained. The successful capture uses [this closure](qualification-config.json).

The first landing preflight also refused an environment mismatch: the warm
Cargo target and job limit were selected only for `verify`. The same validator
was replayed with `CARGO_TARGET_DIR=build/gate-cli` (absolute in the recorded
environment) and `CARGO_BUILD_JOBS=2`, matching verification/publication. All five
native/interpreter pairs and five rejections passed again with no invalidations.
The compiler and native image identities are unchanged. This observation belongs
to the evidence-only descendant `4bf4f360f71bdaab57316e000d435a6c3309e611`, whose compiler inputs
match the measured producer. Retain the [refusal](verify-initial.log),
[matched-environment run](qualification-matched-environment.log), and its separate
[immutable receipt](publication-qualification-receipt.json.gz). The source and
compressed-receipt identities are in the summary. Match these environment settings
before qualification and verification when replaying.

## Remaining optimization question

The [discovery dependency audit](discovery-audit.md) identifies the smaller product
to test: ordinary call selection with exact receivers, static actuals, preceding
local scope and invocation identities. Header-only selection is insufficient;
filtering to generic calls also misses concrete-return wrappers. Normalization and
routing dependencies need a differential experiment before replacing the full
resolver. No discovery bypass or new discovery implementation is included here.

The two retained ownership changes removed five of eight full expression-arena
copies in the original measurements. This integration establishes compatibility
of those changes with the newer parser/arena/checking/backend code. It does not
establish new copy counts, end-to-end speedup, cross-host acceptance, or saturation.

## Replay artifacts

Frozen compilers, initial and final build logs, qualification state/receipts,
project products, and the original pre-rebase source bundle are retained under
`/Volumes/Crucial/CathedralOS/Omega/build/measurement-archives/resolution-boundary-2026-10-22`.
Original absolute paths remain in immutable receipts; archive directory names map
to the old `build/` children. The original source bundle requires commit
`114906953e11e283b8082f03d8da99fbf39dff7e`, which is in main's history.

The scripts here retain their recorded `build/resolution-integration-final`
output path. In a replay worktree at the measured candidate, prepare a fresh output
directory and base/main files, build/freeze with the ordinary two-slot protocol,
run `setup.py`, configure the current absolute artifact paths with `configure.py`,
then start/run pinned qualification with the reviewed configuration. Never overwrite
a frozen producer or relabel a replay as the original observation.
