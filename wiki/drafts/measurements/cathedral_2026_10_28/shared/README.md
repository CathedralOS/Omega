# UI layout and frame: one checked compilation

The selected change asks the existing compiler to produce both independent UI
exports from one Build. Cathedral previously compiled two probe executables and
then discarded both after extracting their exports. The new builder requests one
probe, preserves both export bodies, and packages them through the existing pure
ELF validator. No stage, proof obligation, or component is omitted.

## Measurements

These are Windows x86-64 process-wall measurements of Linux x86-64 export
production. Each invocation has a fresh output directory. Ordinary persistent
compiler caches remain enabled; these are not cache-empty builds. Dependency
preparation is outside the measured interval. Both `.so` images and both
`.so.component` descriptions must match between the two modes within each series.
The discarded probe executable is intentionally different.

| Controlled series | Separate layout + frame | Combined exports | Median reduction |
| --- | ---: | ---: | ---: |
| Original `608307c1ee2` producer, frozen migrated snapshot, after explicit warm-ups | 17.054, 16.216 s | 12.081, 12.322 s | 26.65% |
| Candidate with export index and fresh-source fix, isolated Cathedral inputs, ordinary retained cache | 24.703, 23.257 s | 8.709, 9.194 s | 62.67% |

Each row is its own A/B comparison. Do not compare times across rows as another
speedup: producer identity, source paths and retained cache history differ.
There are only two measured observations per mode; these are local screening
results, not confidence intervals or a cross-machine performance guarantee.
The original series excludes initial pair/cohort warm-ups of 26.005/12.991 s.
The earlier profiled 13.353 versus 26.708 s comparison was preliminary and is
not the selected speedup claim.

The candidate's combined runs report approximately 21 ms in Stage 05; its
separate layout builds still report approximately 3.2-3.3 s there. Retention
behavior is part of the measured benefit. This does not mean every proof or
component-admission operation costs only 21 ms. Full phase counters below are
inclusive/nested, so their times cannot be summed.

Records: [original matched run](cohort-comparison-02/result.json),
[candidate matched run](cohort-comparison-final/result.json),
[initial combined profile](cohort-01/result.json), and
[prototype profile](shared-profile-02/result.json). Adjacent `.log.gz` files
retain timings and counters. The measured drivers preserve explicit inputs.

## Rejected alternative and repeated work

The first experiment retained `CompilationSession` across two fresh project
admissions. Reusing a whole `BuildSession` would be incorrect because closing a
project revokes its package grants. A compiler-only shared session reduced the
second source assembly count from three to one and settled checking requests
from four to two, but saved only about 5%: 24.185/23.969 s separately versus
22.900/22.900 s shared, after an excluded first warm-up. Every output matched.
See [its complete record](shared-comparison-01/result.json).

That prototype is removed. Its patch and example are retained as compressed
experiment evidence, not supported APIs. An additional in-memory component
request memo was sketched but never measured and is also removed. The selected
single-root request already exists in the compiler facade and eliminates more
work with less new code.

The initial profiled combined request performed one component-cache recovery
(~1.077 s), one native-product scope (~3.005 s), typed-tree lowering (~2.645 s),
and source-graph load/store (~1.042/0.746 s). Source persistence attempted to
encode 512 MiB and then refused its own size limit. These observations point at
Terminal/native product materialization and retained-record cost as subsequent
experiments. They do not justify assuming theorem proving dominates.

## Implementation and checks

Omega publishes `omega-components.tsv` from its retained native products in the
same staged file set as the images and descriptions. Each row names the full
requirement, component filename and exact symbol; this is descriptive metadata,
not new trust authority. Cathedral verifies the expected layout/frame roster,
local dependencies and actual symbols, then uses the existing pure-image checks.
The current composition benchmark requires seven roots and eight component/root
products in the same customer scope; UI still requires two components. Previous
records keep their original eight-root composition.

The reuse experiment exposed an existing no-disk session bug: a changed anonymous
source dependency could reuse a stale prepared graph. Fresh anonymous source
capture now precedes lookup even without a disk retention lane. The regression
checks both roots, tightens a dependency contract, requires both to reject, then
restores it and requires both to pass in reverse order. The initial failing and
fixed test logs are retained.

Validated:
- Seven Cathedral packaging tests, including shuffled order, missing/duplicate
  exports, wrong symbols, nonlocal paths, substituted dependency and schema.
- Seven native UI tests on Windows, executing the extracted SysV x86-64 bodies:
  controls/editing, instance separation, rejected-frame recovery, dynamic geometry,
  clipping, malformed packets and stale node identity.
- The new invalidation regression passes.
- The native `independent_imported_data_exit` fixture compiles, executes with
  exit 0, and checks the published index against both retained component products.
- Four benchmark tests retain failure/partial-build rejection and require both
  combined components and their descriptions.
- Cathedral source-layout validation passes (74 Omega and 151 Rust edges).
- Scoped Clippy passes on retry. Its first attempt was discarded by mbx because
  build-script inputs changed during compilation; that attempt is not a pass.

The builder's extracted layout/frame blobs are byte-identical to the two-build
experiment (SHA-256 `564d15f1534ec8ca5a7e64d2c3a9c56de68dddc592a62c151ced8f785e1ce1f2`
and `f8b2b951355482c8a36b281d97c64a611ee3c66a01cf4727e714361676bfc1ca`).
The final builder receipt and native logs identify the actual loaded artifacts.
The qualification rebuild changed only test-source inputs relative to the measured
candidate, which still changes the compiler's cache identity. Its actual builder
run took 20.262 s including update, cache-miss compilation, extraction and receipt;
that is not comparable to the warm compiler-only A/B interval. A subsequent fresh
output with the qualified producer took **7.974 s**, with byte-identical components
and descriptions to the runtime-tested builder. Its phase profile attributes
2.865 s to native product production, 1.330 s to Terminal selection/lowering, and
0.914 s to component recovery (nested scopes). See
[qualified producer and final validation](final-validation.json).


## Scope and remaining acceptance

Under one second is not reached. Even the warm combined UI pair is about nine
seconds, and it is only one part of Cathedral. The previous kernel, chooser-session
and Colors blockers remain; there is still no qualified full-composition metric
or new OS boot observation. No whole-corpus/workspace sweep was run.

The live dirty Omega and Cathedral checkouts were preserved. Implementation is
in isolated worktrees. Cathedral local baseline `c6132f79` records the previously
frozen user inputs and diagnostic syntax migration separately from this change;
it is not an optimization commit and must not be presented as newly authored UI
work. Publication to remote/main is not performed. The claim attempt timed out;
a subsequent successful registry read contained no matching ticket
`8441669af77246ac96925521fa4683dc`, so no registered claim or release is asserted.

Local implementation commits: Omega `cfec2168c0fb7934d1b73f0bc8ead875740633d6`
and Cathedral `0737c5da1c1af2384b75edf407900b279a182a4e` (on the separately
identified frozen-input baseline). After a whitespace-only adapter cleanup, the
actual builder was rerun and its receipt refreshed; every export and packaged
output hash still matches the native-tested build. The Cathedral implementation
patch is retained here for the paired change.
