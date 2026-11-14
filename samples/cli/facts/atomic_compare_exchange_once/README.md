# Single-attempt observing atomic exchange

A single compare-and-exchange attempt must report whether its store committed.
These programs keep that result together with the value the instruction
observed, while preserving the selected cell and its untouched neighbor.

Owning task: **ATOMIC-MEMORY-MODEL** in [TASKS.md](../../../../TASKS.md).
Every standard place copies this fact:

> A single-attempt exchange reports success or the instruction-observed failure value at its exact atomic cell.

The selected cell starts at 10 and its sibling at 17. An expected-10 attempt
may exchange 99 or report a legitimate failed commit; an expected-9 attempt
must report `Mismatched(10)` without writing. The serial programs check the
resident and sibling after every outcome. They allow `Uncommitted(10)` when
appropriate rather than inventing a spurious failure. Concurrent observation
and contention remain separate acceptance.

## Places and controls

| Place | Positive | Fact-sensitive twin |
| --- | --- | --- |
| Local | [program](local/main.omg) | [reject](invalid_local/main.omg) |
| Field | [program](field/main.omg) | [reject](invalid_field/main.omg) |
| Parameter | [program](parameter/main.omg) | [reject](invalid_parameter/main.omg) |
| Call result | [program](call_result/main.omg) | [reject](invalid_call_result/main.omg) |
| Match join | [program](match_join/main.omg) | [reject](invalid_match_join/main.omg) |
| Const name | [program](const_name/main.omg) | [reject](invalid_const_name/main.omg) |
| Nested machine | [program](other_state/main.omg) | [reject](invalid_other_state/main.omg) |
| Other machine | [program](other_machine/main.omg) | [reject](invalid_other_machine/main.omg) |
| Selected target body | [program](target_body/main.omg) | [reject](invalid_target_body/main.omg) |

Every twin changes only the first failure ordering from `Receive` to `Publish`.
Failure cannot publish or exceed the success ordering. The matching pins are
`tests/omega/{pass,fail}/facts/atomic_compare_exchange_once_through_<place>_{exit,rejected}`.
Each positive has four hosted entry bindings and expects exit 0.

The match program runs both Boolean choices for success and mismatch. One arm
writes an independently checked path marker, so always choosing one arm cannot
satisfy the oracle. The complete outcome still crosses the join.

The [root](main.omg), [composition](composition/main.omg),
[operand-effect](operand_effect/main.omg), [high-bit](high_bit/main.omg) and
[wide](wide/main.omg) programs retain the original runtime checks. The wide
case crosses the 64-bit payload and aggregate-return boundary. The
[bounded production](bounded/README.md) preserves the standing resident bound,
including replacement helpers and earlier/later operand snapshots.
[Stronger failure](controls/stronger_failure/main.omg),
[forged outcome](controls/forged_outcome/main.omg) and
[wrong resident](controls/wrong_resident/main.omg) reject their own ordering,
sealed-family or exact-resident obligation.

## Compiler requirements

The native form carries two outputs: the observed resident and the actual
commit outcome. Result construction consumes both; matching values alone do
not prove that an exclusive store committed. ARM64 uses one acquire-exclusive
load and at most one release-exclusive store, with full-system barriers and no
retry loop or optional LSE extension requirement. A failed store produces
`Uncommitted`; a mismatch clears the reservation without writing. x86-64
records the actual CMPXCHG result. Both independent instruction decoders check
the complete sequence and register effects.

A call that writes a separate ordinary scalar must not erase the atomic cell's
known preceding write. The common analysis uses checked simultaneous argument
compatibility and the scalar storage's complete type. Reference-bearing,
atomic-bearing, unknown and indirect storage stay conservative. This changes
neither access authority nor the requirement for a unanimous coherence frontier.

| Requirement | Evidence supplied here |
| --- | --- |
| Establishment | Dedicated atomic cells, exact operands and legal orderings introduce the canonical three-case result. Ordering, forged-family and wrong-resident controls reject. |
| Identity and lifetime | Exact root/path/field, observed-value checks, untouched sibling 17 and operand-effect snapshots preserve the intended cell and value versions. |
| Composition | The complete outcome crosses all nine places, nested calls and the selected target body. The match oracle distinguishes both paths. |
| Soundness | Source checks retain atomic storage authority and resident validity. Existing shared non-atomic and unreadable stored-pointer controls remain rejected; the latter rejects at address formation. |
| Psi 05–07 | The same target-neutral `AtomicAccess` and canonical outcome execute in Terminal Psi. No target choice is moved into source checking. |
| Omega 00–02 | Explicit Build rows select the target-body providers. Tags 17 and 29 challenge a wrong provider. Checked layouts retain the complete 32/64-bit result and ordinary calling plans. |
| Omega 03–06 | Both result registers, input separation and exact clobbers survive selection and allocation. Narrow expected representations are compared at their stored width. |
| Omega 07–09 | Independent decoded forms, native exit validation and actual image execution preserve all outputs. The existing Boolean-store row exercises baseline ARM byte loads/stores, and a native-layout neighbor checks the shared artifact route. |

The governing contracts are [concurrency](../../../../wiki/spec/language/concurrency.md),
[atomic outcomes](../../../../wiki/spec/resources/access.md#atomic-compare-exchange-outcomes),
[ownership](../../../../wiki/spec/language/ownership.md),
[loan compatibility](../../../../wiki/spec/terminal-psi/loans.md),
[structural access](../../../../wiki/spec/terminal-psi/structural_access.md) and
[machine-state evidence](../../../../wiki/spec/build/machine_state_evidence.md).

## Validation and remaining scope

All results below used optimized source `1ee74504bebfd3a945fc250a58abdfeb474f4a5a` and real native/Terminal execution.

| Target | Hosted run | Source checks | Pinned engine agreements | Ordinary projects |
| --- | --- | --- | --- | --- |
| `linux_arm64` | [run 41770758593](https://github.com/CathedralOS/Omega/actions/runs/41770758593) | 39 checked; 43 intended rejections | 39 native 0 / Terminal 0 | 40 engine agreements; 29 intended rejections |
| `linux_x86_64` | [run 41772734408](https://github.com/CathedralOS/Omega/actions/runs/41772734408) | 39 checked | 39 native 0 / Terminal 0 | 40 engine agreements; 29 intended rejections |
| `macos_arm64` | [run 41772737441](https://github.com/CathedralOS/Omega/actions/runs/41772737441) | 39 checked | 39 native 0 / Terminal 0 | 40 engine agreements; 29 intended rejections |
| `windows_x86_64` | [run 41772740528](https://github.com/CathedralOS/Omega/actions/runs/41772740528) | 39 checked | 39 native 0 / Terminal 0 | 40 engine agreements; 29 intended rejections |

The corpus scope includes all nine places and twins, the existing bounded and
operand-snapshot productions, the Boolean-store row and authority controls.
The ordinary projects also include both roots and composition programs. Ten
pure ISA encoder/catalog tests passed in the preceding focused probe; their
implementation bytes are unchanged in this source.

The exact source is preserved in the published archive linked from
[PR #1784](https://github.com/CathedralOS/Omega/pull/1784). Final documentation
and owned records do not change compiler, library, program, Build or oracle
bytes. Recorded rows retain the source that actually produced them.

The source vocabulary here admits observing `u32` and `u64` attempts. The pure
ISA encoder/vector checks also cover the existing 8/16-bit target forms; they
do not claim signed narrow source execution. Serial execution does not prove
concurrent relation axioms, contention behavior or a mechanical target-refinement
theorem. Non-observing exchanges and the remaining operations and task runtime
requirements stay with **ATOMIC-MEMORY-MODEL**; the parent task is open.

<details>
<summary>Baseline and earlier observations</summary>

At fixed source `b53634abd0c42b8d175d5d99ddf32010845f14c7`,
[run 41759507931](https://github.com/CathedralOS/Omega/actions/runs/41759507931)
checked all nine standard positives and rejected all nine invalid orderings.
Every Linux ARM64 execution attempt stopped at missing instruction selection,
with neither engine run. That baseline did not measure the bounded production.

Earlier source `53cd2938437a29356a48d6eaf29f14de07a4bb99` and the original
Linux x86-64 and selected-target migration records remain historical. The
intermediate byte-count, catalog-output, native-exit and match-coherence failures
are retained in the PR evidence with their actual source identities. None is
reported as a successful execution.

</details>
