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

| Program | Distinct mechanism exercised |
| --- | --- |
| [local](local/main.omg) | Success, mismatch, exact selected cell, untouched sibling, inline selected-arm atomic loads. |
| [operand_effect](operand_effect/main.omg) | Replacement helper writes 20 before the attempt, which must report mismatch 20; earlier scalar snapshot remains 10. |
| [composition](composition/main.omg) | Whole outcome crosses a call return, record field, parameter/return, match join and another state; named constant supplies replacement. |
| [high_bit](high_bit/main.omg) | Unsigned 32-bit resident with bit 31 set and replacement u32 maximum, testing observed-value zero extension. |
| [wide](wide/main.omg) | 64-bit resident values above 32 bits and whole 16-byte outcome return. |
| [target_body](target_body/main.omg) | The same operation in separately selected hosted-target bodies, with outcome transport. |

Run each positive with `omega run --both <place>/main.omg < /dev/null`.
Each has an `invalid/main.omg` twin changing only the first failure ordering to
`Publish`. [Stronger failure](controls/stronger_failure/main.omg) changes one
ordering pair to `Receive, GlobalOrder`. [Forged outcome](controls/forged_outcome/main.omg)
uses an authored same-shaped result family, and [wrong resident](controls/wrong_resident/main.omg)
receives an AtomicU32 attempt as the u64 outcome family. These nine controls
must reject at their respective ordering or sealed exact-resident obligation.
All programs are pinned under `tests/omega/{pass,fail}/facts/`.

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
