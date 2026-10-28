# Cathedral compilation baseline

Decision: **inconclusive for the full build**. The original frozen Cathedral
sources do not prepare with either tested compiler. A separate source-migration
diagnostic produces five of the eight native outputs, but three entries still
fail. There is no qualified whole-composition time, cold-build result, runtime
acceptance or sub-second claim. This is a benchmark setup and compatibility
investigation, not an optimization win.

## Workload and identity

The [composition](../../../../tools/benchmark/cathedral.json) follows Cathedral's
Omega UEFI distribution profile and the Omega exports consumed by its default
Rust desktop: kernel, matrix, chooser policy, chooser session, notes, colors,
UI layout and UI frame. The last two are the export probes used by the UI
builder. Cathedral is currently mixed Rust/Omega; this does not compile its
Rust OS, unrelated labs, or every port. It must not be called an entire
Omega-written OS build.

The original snapshot contains 5,060 files and 88,829,756 bytes from Cathedral
`e493d887eee66f1769a79998209db393c832f4ad` plus the captured working changes.
Those totals describe the snapshot, not the compiler's reached input size.
The dirty source checkout was not edited. Its original snapshot, full manifests,
working patch and two diagnostic copies remain at
`C:/SoftwareDevelopmentKits/Omega/.agents/worktree/perf-spill-round/build/cathedral-baseline/`.
The snapshot includes uncommitted inputs, so a fresh checkout of the revision
alone cannot reproduce this workload. [Inputs](inputs.json) pin the retained
files; the source bytes remain local.

The current producer was built with `mbx build --locked --release -p omega`
under one build-slot hold from fixed Omega source
`608307c1ee2e9d6aa20c71d0fc3afcf1210050e5`, opt-level 3. Its SHA-256 is
`0b00c58b83247f820c1c774824aa45336812c9d417ef14d768d6fd4ec00282f8`.
The earlier gallery compiler, source
`05b2e0eab49ba5a4a28ebcced8a2053d91fed08c`, binary SHA-256
`f8ed54c7acd3f4620ca76ddb324bd82b78d5928d4005fcf2802fa18b6319e2ce`,
was used only for a labeled compatibility check. It rejects the same original
Build calls. This does not establish a latest-compiler regression.

## Experiments

1. [Original sources, current compiler](original/result.json): all eight
   entries fail package preparation because provider selections omit the
   explicit Build target. No artifact compilation or full-build timing.
2. [Original sources, earlier compiler](legacy/result.json): same first
   failure. The earlier gallery success was not Cathedral acceptance.
3. [Explicit provider targets](targets/result.json): 32 selections changed
   across 15 Build files in a separate diagnostic snapshot. Each target is
   specific to this measured composition; this is not an all-target migration.
   Preparation then exposes retired nested states and missing boundary ceilings.
4. [Syntax migration](syntax/result.json): a second diagnostic copy converts
   933 nested state declarations in the 27 selected local package roots to
   machines, preserving their explicit signatures, and adds seven empty
   boundary reach ceilings. Across these two syntax changes 75 files differ.
   Empty ceilings impose a constraint which the checker must establish; they
   do not grant extra effects. Source migrations are diagnostic only, have
   not been installed in Cathedral, and have no runtime equivalence claim.
   [Target patch](target-migration.patch.gz), [syntax patch](syntax-migration.patch.gz).

Dependency preparation used the repository's existing `settle_package_review`
local-benchmark ceremony in the diagnostic copy. It accepts generated project
dependency decisions, not a source audit. Preparation and acquisition are
excluded from the compile timings below. The pinned Git revision was retained.

## Partial compile observations

One sequential pass, fresh per-entry output directories, persistent dependency
and compiler caches retained. These are process wall times from
`capture_compile`, not the compiler's shorter internal `total elapsed` figures.
No repeated-run distribution, A/B speedup or cache-empty claim is supported.

| Entry | Process wall | Fresh artifact structural check |
| --- | ---: | --- |
| matrix | 7.536 s | passed |
| chooser | 4.724 s | passed |
| notes | 8.506 s | passed |
| UI layout | 14.016 s | passed |
| UI frame | 12.889 s | passed |

Each successful ELF consumer names exactly one local component. Cathedral's
existing ELF validator accepts that component, and its component description
exists. Exact artifact hashes and peak process memory are in the result.
These observations do not prove execution behavior. Missing entries are never
dropped to manufacture a full-build metric; `complete_compile_ms` is null.

The UI-frame timing log attributes 2.840 s to TypedTrees -> CheckedTrees and
1.447 s to Terminal production, within a 12.889 s process wall time. These
nested timers are not additive, and the checking stage is not proof-only time.
They do not establish proofs as the dominant end-to-end bottleneck. Before
choosing a shared proof cache, attribute the remaining time and repeated package,
provider and native-product work across the selected roots.

## Remaining acceptance

- **Kernel:** online acquisition of pinned Omega revision
  `849161c59ff74176dafc3f8de5ff66148581de22` fails with
  `captured entry path is not canonical: .agents/skills`. The named path is
  a Git tree, not evidence of a symlink. Inspection points to Git inventory
  paths reaching `CapturedSourceTree` with `/` spelling while Windows rebuilds
  canonical components using `\`. This is a source-based diagnosis, not a
  tested repair. Offline retries cannot acquire the still-unsettled pin.
- **Chooser session:** three unmet call contracts: `i < 51` at `next` from
  `appended`, and `i < 6` at `advance` from `name` and `copy_name`.
- **Colors:** exact `u64` arithmetic may overflow at
  `applications/colors/app/rendering/pixels.omg:27:31`.
- Keep the existing source oracles and reject controls while resolving these
  failures; do not replace exact arithmetic with wrapping to get a number.
  Only after the complete composition qualifies should it become the primary
  optimization metric. Keep the already-valid gallery metric meanwhile.
- Boot the fresh kernel through Cathedral's existing marker checks and run
  the corresponding native desktop checks. Neither was performed: the kernel
  artifact is absent and the complete desktop composition does not compile.
- Establish separately controlled cache-empty and incremental-edit runs after
  compatibility and runtime acceptance. A fresh output path is not an empty cache.

## Harness checks and coordination

`python tools/tests/test_cathedral_benchmark.py` passes three focused tests:
failed compilers cannot qualify leftover files, partial compositions have no
full time, and successful exits still require an image with the expected header.
`git diff --check` passes. No full corpus/workspace sweep was run.

The coordination claim attempt timed out twice with an uncertain publication
result. A subsequent successful registry read contained no matching ticket
`e7688d468b0d41c299320f0e5ad9e64c`; no registered claim or release is asserted.
All edits and builds are isolated in `.agents/worktree/cathedral-benchmark`.
