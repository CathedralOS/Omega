# Source ingress ownership — 2026-10-22

Retain all three changes: share checkpoint payload pages, construct accepted
speculative syntax directly, and recycle lexical storage inside parsing tasks.
Across 289 programs, all-worker assembly makes **11.54% fewer allocation
requests** and requests **6.98% fewer bytes**. In ten paired CLI trials, median
assembly time moves from **21.385 to 13.309 ms**. Whole-compile time and peak RSS
remain inconclusive. This continues [retained syntax ownership](../retained_syntax/README.md).

## Mechanisms and limits

A real checkpoint fork freezes nonempty arena pages. Siblings share payloads;
an edit detaches only its page, while appended pages stay owned. Unique
checkpoints avoid publication allocations. Source authority, independent child
admission and existing handle coordinates remain unchanged.

Static arguments and generic receiver/operand probes construct rows in their
destination forest. Acceptance keeps those rows; rejection drops the appended
suffix across all tables and roots while retaining capacity. The attempt may
edit its own new rows, never earlier rows or page identities, and rejected
handles must not escape. This is not general mutable-state rollback. A rare
ambiguous shift alternative still reparses after discarding a superseded application.

The Psi facade loads the ordered frontier. Each parsing task then lexes, parses
and recycles token/range capacity source by source. Decoded bytes retained by
syntax cannot recycle. Source order fixes page identities. Even after a parse
failure, the task checks later sources for lexical failures, preserving lexical
error priority. The observer labels the fused interval `Source ingress`.

## Allocation experiments

These are allocation requests and requested capacity, not resident memory or
measured bytes transferred through the memory bus. All-worker counters include
lexer/parser threads; calling-thread counters alone miss most parser work.
The final comparison uses the same executable invocation path on both sides.

| All-worker assembly, 289 inputs | Requests | Requested bytes |
| --- | ---: | ---: |
| Original baseline | 6,179,197 | 2,353,281,666 |
| Shared pages + direct parser construction, old batch route | 5,487,743 | 2,275,530,109 |
| Complete candidate | 5,466,038 | 2,189,122,254 |

Fusion alone removes **21,705 requests and 86,407,855 requested bytes**. Its
289 complete output hashes match the separate-stage variant exactly. The
combined change removes **713,159 requests and 164,159,412 requested bytes**.
[Assembly counts](assembly-comparison.json), [isolated fusion](fusion-comparison.json).

The first checkpoint experiment eagerly froze every checkpoint. It added 6,910
requests to unique assembly and was rejected. Freezing only at a real fork
leaves unique request counts unchanged. Shared-checkpoint calling-thread requests
fall from 1,198,459 to 1,177,528, with approximately 9.73 MB less allocation
traffic over 289 programs. This earlier byte comparison includes a small harness
invocation-name difference; the final all-worker comparison above removes it.

On 463 parser files, direct construction reduces requests from 230,629 to
209,568 and requested bytes from 66,440,408 to 64,084,521. The sum of per-file
live requested bytes changes from 30,692,680 to 30,700,096; summing per-file
peaks would not establish whole-workload peak. Retained source references fall
from 766 to 607. [Parser comparison](parser-summary.json).

## Output equivalence and actual execution

Complete debug observations retain spans, detached rows, metadata and custody.
The adapter resolves actual source-page handles through their owners. Only
source-owner bookkeeping and arena mutation-revision counters are excluded from
the assembly comparison; capacities and handle coordinates are not erased.

After excluding revision counters, 462 of 463 complete parser observations
match. `runtime_named_extent_generic_copy_exit` retains one otherwise unused
`Count` expression from an accepted fixed-array type; this shifts a generated
transition name from `#18` to `#19`. Its authored structure is unchanged, and its
native and Terminal Psi executions both produce the authored exit **70**.

Against the original assembly baseline, 287 of 289 complete outputs match.
The differing Build fixtures are `selected_const_array_length_exit` and
`structural_equation_conflicting_argument`. Both retain an extra literal extent
leaf and shift later handles. Their parsed root snapshots and source spans
match exactly. The invalid control still rejects with the same explicit
`Capacity` 7 versus structural extent 8 diagnostic. The larger positive already
fails in the baseline on an unrelated Optional zero-representation obligation.
A smaller saved Build program isolates `capacity<[u8; 8], 8>()`; baseline and
candidate both execute it with native/interpreter exit **0**. The complete raw
diffs and runnable probe are retained; these are not claimed as exact full-arena
baseline matches. [Root comparison](extent-root-comparison.json),
[isolated execution](extent-runtime-final.json).

Validation records retain actual outcomes:

- Arena/syntax/parser: 350/357 pass. All seven failures reproduce on untouched
  baseline `7893a3222666c0cb0a3f72798c14c5956a2c091a`: generic binder admission,
  prefix-call acknowledgement, two Boolean-arm shape checks, retired atomic
  admission, fixed-array equation rejection, and a missing sample `.gitignore`.
- Thirty focused lexical/ownership/rollback/fusion tests pass. The integrated
  selection passes 265/265, and final package/source-facade consumers pass 154/154.
- Scoped Clippy completes successfully with warnings. Existing static-lifetime
  annotations and the large `PreparedSourceFrontier` enum remain warnings.
- The final producer matches the published ancestor baseline on all **16 check
  observations and 10 native/interpreter observations**, including complete
  diagnostics and facts. No losses or gains occur. Five runtime positives execute;
  five Build positives already refuse: two empty generated files, two 41-byte
  literals declared as 38 bytes, and the Optional representation case above.
- `length/local` runs natively and in Terminal Psi with exit 0; its invalid twin
  rejects with the same baseline diagnostic. CLI Build comparisons also retain
  the two existing generated-source refusals and successful dependency query.

## Timing and physical memory

Apple M4, 10 logical CPUs, 16 GiB RAM, macOS ARM64; both compilers use opt-level 1.
Ten paired trials alternate A/B order, retain every observation, and record
resource usage and host load. Other work was active on this host.

| Milliseconds | Baseline min / median / mean / max | Candidate min / median / mean / max |
| --- | --- | --- |
| Assembly | 14.009 / 21.385 / 27.526 / 55.470 | 6.755 / 13.309 / 15.217 / 28.830 |
| Whole compiler | 1796.523 / 1930.014 / 1945.119 / 2218.646 | 1681.395 / 1899.754 / 1986.732 / 2445.376 |

Median peak RSS is 340,656,128 versus 342,908,928 bytes. Mean RSS is nearly
unchanged. Whole-compile mean and maximum worsen while the median improves;
there is **no supported whole-compile or RSS improvement claim**. Major faults
and host contention are retained in [all paired observations](cli-observations.json).

## Integration, provenance and replay

Allocation/parser experiments use base `7893a3222666c0cb0a3f72798c14c5956a2c091a`.
Paired CLI measurements use base `1bd52dbc0ce50978690850c94d16097293a2a63f` and
candidate `64b05972a6c887ac1b367c4e52060c6bcee1e93e`. Final runtime observations
come from candidate `666fe3e1dc7b355ae26d96439e159d8ea03f5a13` on integrated base `cdc409f3384a934e9153176d2f21f9bfd163bd00`. The final
publication commit adds this evidence record without changing the measured
production implementation.

Incoming changes affect later checking/lowering and remove unused resolved-tree
accessors/carriers; they change no ingress implementation or selected input.
The source-ingress measurements retain their original identities. Fresh final
consumer binaries, runtime replays and package tests establish compatibility on
the integrated tree. The published ancestor baseline remains `1bd52dbc0ce5`:
this is an observed no-difference integration comparison, not attribution of a
new failure to the immediate parent. Exact-SHA landing verification binds the
publication commit; the claim note and local landing receipt retain its final ID.

The last rebase integrates four further backend cleanups through
`760089b56ac8d9742f4a387d286f89f91facd397`. Inspection shows that the deleted
private direct-storage helper has no callers; the removed manifest text renderer
has no consumers (the remaining `render_text` calls belong to a different
register-homes type); and `BoundaryReferenceDestination` has only its `Parameter`
variant, so the deleted `else` is unreachable. The fourth change is documentation.
These changes alter neither the executed consumer paths nor measured source,
input, codec or IR contracts. The runtime observations above remain applicable;
a fresh product-closure check verifies the rebased publication tree.

A subsequent integration through `0c812a438d5ef436da4ce81c755ee1609bdca775`
removes two unused fact-handle aliases, two uncalled arm-call accessors, and the
unconstructed `FactOrigin::ExitEnsures` variant. Searches of the parent tree find
only the removed definitions/reexports and the accessor-to-accessor call. The
origin has no explicit representation or numeric wire encoding; no accepted
origin case or producer changes. These removals do not change the measured
ingress path or the runtime evidence's consumer inputs. Verification checks the
new product closure.

[Research](research.md), preserved harness/probe source, command scripts,
input/source/binary hashes, complete raw captures, rejected eager-page patch and
frozen executables are retained in the local archive
`build/measurement-archives/ingress-ownership-2026-10-22/`. Restore its scratch
layout as `build/ingress-ownership/` in the chosen checkout when rebuilding;
point the baseline harness dependencies at the recorded baseline checkout.
`replay_assembly.py` uses frozen binaries at an identical invocation path;
`isolate_fusion.py` restores the recorded old facade only for its diagnostic
build and restores production source in `finally`. No diagnostic switch ships.

## Remaining local opportunities

These three passes are complete; the layer is not declared saturated. Float
spellings still retain per-occurrence `SourceText` ownership, composed/generated
names still own distinct bytes, and the operator/domain declaration-boundary
Boolean lookahead still constructs a disposable forest. Page sharing removes
checkpoint copies of these payloads, but their original construction and that
last lookahead remain measurable work. Stage 05 package rechecking stays outside
this layer-0 assignment.
