# Erased shared-reference contracts

An erased `&i32` witness whose referent is 7 discharges `witness == 7` at
`keep` without a runtime witness argument, read, storage or cleanup.

Every place copies that sentence and performs the same `keep` contract call;
only how the erased witness reaches that call changes. Both match arms
execute, so the ordinary runtime result is 41 or 42 and the oracle is exit 0.

## Places

| Place | Positive | Fact-sensitive reject twin |
| --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) (`controls/invalid_local/main.omg`) |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) (`controls/invalid_field/main.omg`) |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) (`controls/invalid_parameter/main.omg`) |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) (`controls/invalid_call_result/main.omg`) |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) (`controls/invalid_match_join/main.omg`) |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) (`controls/invalid_const_name/main.omg`) |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) (`controls/invalid_other_state/main.omg`) |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) (`controls/invalid_other_machine/main.omg`) |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) (`controls/invalid_target_body/main.omg`) |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64` and `linux_arm64` and declares
`Expected exit: 0`. Run one place directly:

```sh
omega --check samples/cli/facts/erased_reference_contract/<place>/main.omg
omega run --both samples/cli/facts/erased_reference_contract/<place>/main.omg
```

`omega run --both` reports the native exit and the Terminal Psi exit;
agreement at zero is the place's runtime oracle. Checking also validates the
foreign target bodies on every host.

## Reject twins

Each twin keeps the same program shape and changes only the referent from 7
to 8, so the contract call's precondition no longer holds at its operation.
Every twin rejects with exit 1:

```
error: cannot prove requires contract for call <machine> from <caller>:
value == 7 (needs fact `witness == 7` here; ...) in state `entry`
```

(the `field` twin names `holder.value == 7`, and the `target_body` twin
reports the same refusal once per target body). Check a twin with
`omega --check <place>/invalid/main.omg` (or `omega --check controls/invalid_<place>/main.omg`).

The row is pinned by
`tests/omega/{pass,fail}/facts/erased_reference_contract_through_*` fixtures —
nine `*_exit` positives and nine `*_rejected` twins — plus the related
`fail/facts/erased_reference_foreign_body_rejected` control. Every positive
records native exit 0 with Terminal Psi agreement on the hosts that ran it.

Canonical authority:
[explicit erased bindings](../../../../wiki/spec/proofs/contracts.md#explicit-erased-bindings),
including binding identity, no runtime contribution, exact argument
substitution, scope and write invalidation. Reviewed contract blob:
`0551b01c0d931150f979d6ea295d3bafd99629bf`.

## Requirements and Evidence

- **Establishment:** Each place establishes the referent fact `witness == 7` from closed primitive construction (`let value: i32 = 7`, `Holder { value: 7 }`, `const SEVEN: i32 = 7`, `ensures result == 7`, or a covered match join whose arms both yield `7`) without any post-hoc caller equality guard.
- **Identity and lifetime:** The erased `&i32` reference names the exact immutable referent value (`value` or `holder.value`) throughout its loan scope; immutable descriptor aliases and closed constructor/scalar leaves retain exact declarations while mutable initializers, captured calls, and recasts are not unfolded.
- **Composition:** Exact actual witness expressions (`&value`, `&holder.value`, `witness`) are substituted into the callee's `requires witness == 7` contract across explicit parameter calls (`parameter`), completed call results (`call_result`), covered match joins (`match_join`), state transitions (`other_state`), forwarding machines (`other_machine`), and all four target-scoped machine bodies (`target_body`). Each `<place>/invalid/main.omg` (and `controls/invalid_<place>/main.omg`) differs from `<place>/main.omg` only in the witness referent (`7` vs `8`).
- **Soundness boundaries:** Erased `[erased]` parameters remain in source checking and proof obligation verification (`requires witness == 7`) while respecting the boundary between erased mathematics and executable values: no erased witness can be read at runtime, and an invalid referent (`8`) is rejected at compile time with exit 1.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Erased reference parameters (`witness [erased]: &i32`) are completely stripped from executable signatures during checked-to-Terminal Psi lowering (`proof-term-tags`), ensuring `keep(runtime, witness)` lowers into canonical `PSITERM\0` as a unary runtime operation operating strictly on `runtime` with zero runtime witness argument, slot, read, or cleanup. All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native exit 0.
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) through Build companion declarations. Calling plans (`CallPlan`/`StatePlan`) assign ABI registers and stack slots exclusively to executable arguments (`runtime: i32` in ECX/EDI/W0), allocating exactly zero register or stack lanes to erased reference parameters.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects 32-bit integer instructions solely for `runtime` and branch checks without generating instructions, address-of calculations, or spill slots for erased witnesses. Disjoint activation storage houses only executable locals and call frames, and standard prologues and epilogues maintain worst-case stack bounds (WCSU).
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Basic blocks and match branches lay out with short/near branch relaxation and offset reflow, machine-state footprint evidence validates against permitted limits, and direct PE/ELF/Mach-O executables are emitted without witness relocations or byte-pool artifacts. Terminal Psi interpretation and native execution agree with exit 0 (`omega run --both`) across hosted targets including `windows_x86_64` and `macos_arm64`.

## What this row establishes and what stays open

This row discharges the explicit-erased-binding requirement that an erased
binding "remains in checking and semantic identity but contributes no runtime
field, address, read, or cleanup" while supplying "proof computation or static
authorization": a primitive referent observation `witness == 7` is established
at each witness construction and discharged at the `keep` precondition through
all nine transport places — a local, a record field, an explicit parameter, a
completed call result, a covered match join, a const name, a successor machine,
another machine, and each of the four target bodies. Composition is covered by
the parameter, call result and other-machine places, which substitute the
exact actual witness into the callee's contract. Soundness of erasure is
covered by the runtime evidence itself: no witness argument reaches native or
Terminal execution, so agreement at exit 0 on all four hosted targets
demonstrates the binding's zero runtime contribution. The reject twins hold
establishment honest: a referent of 8 cannot discharge `witness == 7` at the
same call, and each twin refuses only that fact.

The original compiler refused the unchanged root because `witness` was not
scalar. The repair preserves its reference declaration while carrying a
primitive referent observation in the existing erased contract lane. Exact
source admission remains responsible for type, scope, borrow, validity and
preconditions. Immutable descriptor aliases and closed constructor/scalar
leaves retain exact declarations; mutable initializers, captured calls and
recasts are not unfolded.

Still open: erased witnesses whose referent is not a closed primitive — erased
record/array contracts and evidence through dynamic dispatch — and transitive
assumption tracking for erased terms beyond this direct construction lane.
Native execution rows for `windows_x86_64` and `linux_arm64` record on their
own hosts' goldens; the bindings are declared for all four.
