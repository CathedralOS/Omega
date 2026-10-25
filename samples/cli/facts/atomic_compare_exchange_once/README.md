# Single-attempt observing atomic exchange

Parent: `ATOMIC-MEMORY-MODEL`. One exact-cell observing
`CompareExchangeOnce` event establishes the canonical `Mismatched(observed)`,
`Exchanged`, or `Uncommitted(observed)` result without changing its location,
operands, ordering or observation identity.

The unchanged [local witness](local/main.omg) attempts expected 10 → replacement
99 at resident 10 and checks that sibling storage stays 17. Its expected-9
attempt must return `Mismatched(10)` without writing. A legitimate uncommitted
attempt must carry observed 10 and leave both cells unchanged. Serial execution
and bounded x86 CMPXCHG / admitted ARM LSE CAS need not produce spurious failure;
these programs never invent one.

Fresh baseline `53cd2938437a29356a48d6eaf29f14de07a4bb99`, optimized Rust
producer (level 3), Linux x86-64: `omega run --both local/main.omg` exited 200
because neither `compare_exchange_once` call resolved. Neither engine ran.
The frozen source SHA-256 is
`feb3b6a54389d3bdd0312936e908a89284ee63db337ed1d00310f71183027636`.
Both engines exit 0 on all six positive programs with the integrated optimized producer.

| Place / Program | Witness (`main.omg`) | Reject twin (`invalid/main.omg`) | Distinct mechanism exercised |
| --- | --- | --- | --- |
| root | [`main.omg`](main.omg) | - | Root package probe exercising single-attempt `CompareExchangeOnce` (`Exchanged`, `Mismatched`, `Uncommitted`) and sibling preservation. |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | Success, mismatch, exact selected cell, untouched sibling, inline selected-arm atomic loads. |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | Whole `AtomicCompareExchangeOnceOutcome<u32>` carried through nominal record field `Held.outcome`. |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | Whole `AtomicCompareExchangeOnceOutcome<u32>` passed as a machine parameter to `inspect_param`. |
| call_result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | Helper machine `attempt` returns `AtomicCompareExchangeOnceOutcome<u32>` to caller. |
| match_join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | `AtomicCompareExchangeOnceOutcome<u32>` joined across conditional `match` branches before inspection. |
| const_name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | Named constants (`EXPECTED_MATCH`, `EXPECTED_MISMATCH`, `REPLACEMENT`, `SIBLING_INIT`) supply operands and expected observations. |
| other_state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | State transition carries `AtomicCompareExchangeOnceOutcome<u32>` into successor `state inspect`. |
| other_machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | Cross-machine call `Inspector::check` receives and verifies `AtomicCompareExchangeOnceOutcome<u32>`. |
| target_body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | The same operation in separately selected hosted-target bodies, with outcome transport. |
| operand_effect | [`operand_effect/main.omg`](operand_effect/main.omg) | [`operand_effect/invalid/main.omg`](operand_effect/invalid/main.omg) | Replacement helper writes 20 before the attempt, which must report mismatch 20; earlier scalar snapshot remains 10. |
| composition | [`composition/main.omg`](composition/main.omg) | [`composition/invalid/main.omg`](composition/invalid/main.omg) | Whole outcome crosses a call return, record field, parameter/return, match join and a nested machine; named constant supplies replacement. |
| high_bit | [`high_bit/main.omg`](high_bit/main.omg) | [`high_bit/invalid/main.omg`](high_bit/invalid/main.omg) | Unsigned 32-bit resident with bit 31 set and replacement u32 maximum, testing observed-value zero extension. |
| wide | [`wide/main.omg`](wide/main.omg) | [`wide/invalid/main.omg`](wide/invalid/main.omg) | 64-bit resident values above 32 bits and whole 16-byte outcome return. |

Run each positive with `omega run --both <place>/main.omg < /dev/null`.
Perturbing a single token in the root oracle (`cells.selected.load(NoOrdering) == 99` -> `cells.selected.load(NoOrdering) == 98`) causes both engines to fail closed with `crash Trap` (`interp crash: Trap`, `native exit -1073741795`).
Each has an `invalid/main.omg` twin changing only the first failure ordering to
`Publish`. [Stronger failure](controls/stronger_failure/main.omg) changes one
ordering pair to `Receive, GlobalOrder`. [Forged outcome](controls/forged_outcome/main.omg)
uses an authored same-shaped result family, and [wrong resident](controls/wrong_resident/main.omg)
receives an AtomicU32 attempt as the u64 outcome family. All controls
must reject at their respective ordering or sealed exact-resident obligation.
All programs are pinned under `tests/omega/{pass,fail}/facts/`.

## Pinned fixtures

- Standard 9-place positive fixtures (9):
  - `tests/omega/pass/facts/atomic_compare_exchange_once_through_local_exit`
  - `tests/omega/pass/facts/atomic_compare_exchange_once_through_field_exit`
  - `tests/omega/pass/facts/atomic_compare_exchange_once_through_parameter_exit`
  - `tests/omega/pass/facts/atomic_compare_exchange_once_through_call_result_exit`
  - `tests/omega/pass/facts/atomic_compare_exchange_once_through_match_join_exit`
  - `tests/omega/pass/facts/atomic_compare_exchange_once_through_const_name_exit`
  - `tests/omega/pass/facts/atomic_compare_exchange_once_through_other_state_exit`
  - `tests/omega/pass/facts/atomic_compare_exchange_once_through_other_machine_exit`
  - `tests/omega/pass/facts/atomic_compare_exchange_once_through_target_body_exit`
- Standard 9-place reject fixtures (9):
  - `tests/omega/fail/facts/atomic_compare_exchange_once_through_local_rejected`
  - `tests/omega/fail/facts/atomic_compare_exchange_once_through_field_rejected`
  - `tests/omega/fail/facts/atomic_compare_exchange_once_through_parameter_rejected`
  - `tests/omega/fail/facts/atomic_compare_exchange_once_through_call_result_rejected`
  - `tests/omega/fail/facts/atomic_compare_exchange_once_through_match_join_rejected`
  - `tests/omega/fail/facts/atomic_compare_exchange_once_through_const_name_rejected`
  - `tests/omega/fail/facts/atomic_compare_exchange_once_through_other_state_rejected`
  - `tests/omega/fail/facts/atomic_compare_exchange_once_through_other_machine_rejected`
  - `tests/omega/fail/facts/atomic_compare_exchange_once_through_target_body_rejected`
- Additional mechanism & control fixtures:
  - `tests/omega/pass/facts/atomic_compare_exchange_once_{local,operand_effect,composition,high_bit,wide,target_body}_exit`
  - `tests/omega/fail/facts/atomic_compare_exchange_once_{local,operand_effect,composition,high_bit,wide,target_body,stronger_failure,forged_outcome,wrong_resident}_rejected`

| Obligation Class | How This Fact Row Establishes It |
|---|---|
| **Establishment** | `compare_exchange_once(expected, replacement, ReceivePublish, Receive)` on an exact `AtomicU32`/`AtomicU64` cell establishes the canonical sealed `AtomicCompareExchangeOnceOutcome<T>` (`Exchanged`, `Mismatched { observed }`, `Uncommitted { observed }`) with exact resident type and ordering obligations ([concurrency](../../../../wiki/spec/language/concurrency.md), [placed access](../../../../wiki/spec/resources/placed_access.md#atomic-compare-exchange-outcomes)). |
| **Identity and lifetime** | The `AtomicCompareExchangeOnceOutcome<T>` value carrier is copyable (`copyable`) and independent of the borrowed `Cells` storage lifetime; writes to `cells.selected` never disturb `cells.untouched` ([ownership](../../../../wiki/spec/language/ownership.md), [structural access](../../../../wiki/spec/terminal-psi/structural_access.md)). |
| **Composition** | Across all nine standard transport places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`), the tagged outcome payload preserves its discriminant and observed scalar value through calls, record fields, branch joins, and state/machine transitions ([expressions](../../../../wiki/spec/language/expressions.md)). |
| **Soundness boundaries** | Each `<place>/invalid/main.omg` changes only the failure ordering to `Publish` and rejects with `failure cannot publish or be stronger than success`; `stronger_failure`, `forged_outcome`, and `wrong_resident` reject invalid ordering pairs and unsealed/mismatched outcome carriers. |
| **Checked-to-Terminal Psi lowering (`Psi 05–07`)** | `CompareExchangeOnce` lowers into canonical `PSITERM\0` atomic observation instructions and executes in the Terminal Psi interpreter (`omega run --both`). |
| **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`)** | `ProgramEntry` settles for `windows_x86_64`, `linux_x86_64`, `linux_arm64`, and `macos_arm64`; `Cells` and tagged `AtomicCompareExchangeOnceOutcome<T>` layout plans and aggregate calling/return plans validate without reopening source. |
| **Instruction selection, register homes, and activation frame realization (`Omega 03–06`)** | Backend lowering selects atomic compare-exchange (`lock cmpxchg` on `x86_64`, LL/SC or admitted LSE CAS on `arm64`) with zero-extended 32-bit and full 64-bit observed values and bounded activation frames. |
| **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission)** | Native PE/ELF/Mach-O images emit and execute on `windows_x86_64` and `linux_x86_64` with exit code `0`, agreeing with Terminal Psi (`omega run --both`). |

The repair preserves a structural event from parsing and checked evaluation
through Terminal semantics, serialization/admission, abstract coherence,
legalization and native selection. Runtime operands execute once before the
instruction observes the resident. The canonical library outcome explicitly
requests checked copyability. General compositional loads retain their authored
evaluation point; scalar-only owned outcomes do not lend separate atomic
backing merely because disposal authority permits descriptor retirement.

Governing contracts are [concurrency](../../../../wiki/spec/language/concurrency.md),
[canonical result/custody](../../../../wiki/spec/resources/placed_access.md#atomic-compare-exchange-outcomes),
[evaluation order](../../../../wiki/spec/language/expressions.md),
[declared copyability](../../../../wiki/spec/language/ownership.md), and
[structural storage/descriptor authority](../../../../wiki/spec/terminal-psi/structural_access.md).
The qualification packet retains the exact passages and source blob identities.

Runtime evidence is from Linux x86-64 only. Windows x86-64 and Apple Silicon
ARM64 image validation exercise their distinct ABI/instruction paths without
foreign execution. Linux ARM64's existing catalog does not admit LSE atomic
forms without explicit feature custody; its refusal remains a limitation.
Signed and 8/16-bit observing source atomics are not admitted by the current source vocabulary; those runtime paths are not claimed.
Broader concurrent relation axioms and contention execution, placed/bounded-field
production, non-observing try-exchange, and additional target refinement proof
remain requirements of the parent outcome.
