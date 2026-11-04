# Non-observing single-attempt exchange

A failed non-observing exchange returns the proposal so its caller can recover
it. A committed exchange returns the displaced resident. The compiler now
preserves that complete affine outcome through records, calls and joins; the
frozen field and match examples no longer fail during lowering.

Owning outcome: **ATOMIC-MEMORY-MODEL** in [TASKS.md](../../../../TASKS.md).

Fact: **A non-observing single attempt returns the proposed u32 on failure and the displaced u32 on commit.**

The programs distinguish resident 10, proposal 99 and untouched neighbor 17.
They test matching key 10 and mismatching key 9. A lawful uncommitted attempt
returns proposal 99 and retains resident 10; a mismatch also returns proposal 99.
Commitment returns displaced 10 and leaves resident 99. The runtime checks
follow the operation and introduce no static premise. The outcome remains
`affine` even though its scalar payload is copyable.

| Place | What the example checks |
| --- | --- |
| [Local](local/main.omg) | Bind the direct outcome. |
| [Field](field/main.omg) | Move the outcome into a record and select its field. |
| [Parameter](parameter/main.omg) | Pass it as an owned argument. |
| [Call result](call_result/main.omg) | Return it from an ordinary machine. |
| [Match join](match_join/main.omg) | Observe both owned paths with independent markers 17 and 23. |
| [Const name](const_name/main.omg) | Keep the same key, proposal and neighbor through named constants. |
| [Nested machine](other_state/main.omg) | Transfer the outcome to a nested machine. |
| [Other machine](other_machine/main.omg) | Consume it in a separate free machine. |
| [Selected body](target_body/main.omg) | Return it from one implementation selected by four explicit Build rows. |

Each place retains its `invalid/main.omg` twin and positive/negative corpus pin.
The twin changes only the primitive to observing `compare_exchange_once`,
while keeping the non-observing receiving type and the same oracles. It rejects
the wrong sealed result family. The separate
[observing row](../atomic_compare_exchange_once/README.md) has different payload
roles and cannot establish this requirement.

The surviving [u64 control](../../../../tests/omega/pass/facts/atomic_try_exchange_once/controls/wide/main.omg)
uses distinct values above 32 bits. The
[discarded-result control](../../../../tests/omega/pass/facts/atomic_try_exchange_once/controls/discarded_result/main.omg)
checks once-only evaluation of a counted proposal helper and the unchanged
resident and neighbor. Their wrong-family twins remain pinned. Repository
consolidation removed duplicate sample controls and the ordinary-name,
resident/storage, failure-ordering and duplicate-consumption pins. Their
original inputs and actual results remain in the measured source and linked
reports; this qualification does not recreate those duplicates.

| Requirement | Evidence established |
| --- | --- |
| Establishment | Dedicated AtomicU32/u32 and AtomicU64/u64 use the authenticated builtin identity correspondence. The receiving outcome type grants no atomic permission. |
| Identity and lifetime | Failure returns the proposal, commitment returns the displaced resident, and neighbor 17 remains unchanged. The result has one affine owner. |
| Composition | Complete outcomes cross fields, parameters, calls, both join paths, nested machines and selected public returns without a new caller guard. |
| Soundness boundaries | Wrong result family, resident identity, ordinary storage, illegal failure ordering and duplicate consumption reject in the measured controls. |
| Terminal execution | The event retains its operands, ordering and complete outcome through encoding and interpretation, with no extra read or retry to supply a payload. |
| Layout and native execution | Four explicit hosted entry/provider rows carry the case layouts. Both scalar widths and automatic affine disposal execute through the same recorded result disposition. |

All nine places and their twins are qualified on the four matching hosts.
Each host's complete focused scope has 17 checked positives, 19 intended
refusals, 17 actual native/Terminal exit-zero agreements, and 27 ordinary
commands: 12 zero agreements and 15 intended refusals. These counts include
five neighboring runtime controls; the atomic row itself contributes 12 runtime
programs and 27 checking cases before repository consolidation.

| Host | Actual compiler and input source | Completed report |
| --- | --- | --- |
| Linux x86-64 | `4075afd33d6b` | [41859185711](https://github.com/CathedralOS/Omega/actions/runs/41859185711) |
| Linux ARM64 | `4075afd33d6b` | [41859139676](https://github.com/CathedralOS/Omega/actions/runs/41859139676) |
| Windows x86-64 | `4075afd33d6b` | [41859167816](https://github.com/CathedralOS/Omega/actions/runs/41859167816) |
| macOS ARM64 | `17be13e1a9ff` | [41887658330](https://github.com/CathedralOS/Omega/actions/runs/41887658330) |

The Mac report also contains a separate owned-field scope; the counts above
select only this atomic qualification. All 229 non-document guest inputs are
identical between the two measured sources. The Mac compiler additionally
checks complete empty reference-slot effects without suppressing scalar or
atomic writes. Its callable-binding and local-domain checks preserve this
concrete scalar/outcome route. The earlier three-host executions retain their
actual `4075` identities. These observations do not claim execution at this
metadata revision or at a later compiler revision. Exact sources are retained
with [PR #1822](https://github.com/CathedralOS/Omega/pull/1822) and
[PR #1836](https://github.com/CathedralOS/Omega/pull/1836).

After corpus consolidation, 22 surviving Source entries record their actual
Mac observations: 11 checked positives and 11 intended refusals. The removed
host record slots remain absent. All 75 original owned observations are
retained by the measured reports, with their original source stamps.

Earlier failures remain historical. The fixed
[9924 baseline](https://github.com/CathedralOS/Omega/actions/runs/41824546059)
refused the unresolved primitive without running either engine. The
[33db local probe](https://github.com/CathedralOS/Omega/actions/runs/41832747517)
ran three positive controls. The
[e766 full row](https://github.com/CathedralOS/Omega/actions/runs/41836746210)
stopped in the unchanged field and match examples; that report remains failed.
The shared result-disposition repair at `0253` then passed the
[complete Linux scope](https://github.com/CathedralOS/Omega/actions/runs/41841737327).
The old `0410` Source records were masked by an incomplete `Nat.mod` call-effect
summary; the later Mac observations check the original positive operations.
Earlier Mac requests that never acquired a runner provide no engine evidence.

This closes the measured dedicated builtin single-attempt outcome requirement.
Formal concurrency, forced Uncommitted outcomes, affine/linear residents,
arbitrary-key laws, non-observing-only policy access, contention and formal
target-refinement proofs remain open in the parent task.
