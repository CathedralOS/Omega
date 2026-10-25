# Named conformance dispatch

Owning task: **NAMED-IMPLEMENTATION-CONFORMANCE-MIGRATION** in
[TASKS.md](../../../../TASKS.md). The requirement is that a named
implementation's inline member preserves its public ensures through generic
dispatch.

**Fact: Selected satisfies Arithmetic<i32> and inline combine preserves result == left + right.**

`Selected` is code, not a data subject. `apply` takes an evidence binder
`Operations satisfies Arithmetic<i32>` and returns `Operations::combine`.
Specialization substitutes `Selected`. The caller's ensures is the member's
public ensures after that substitution. Both operands stay in `0..=50`, and
the runtime oracle is `20 + 22 = 42`.

## Canonical transport row

Every positive below, including the root, declares `Expected exit: 0` and binds
`ProgramEntry` for `windows_x86_64`, `linux_x86_64`, `macos_arm64`, and
`linux_arm64`. The nine place sources are pinned as
`tests/omega/pass/facts/named_conformance_dispatch_through_<place>_exit`. Each
reject twin is pinned as
`tests/omega/fail/facts/named_conformance_dispatch_through_<place>_rejected`.

| Place | Positive | Reject twin |
| --- | --- | --- |
| Root | [main](main.omg) | — |
| Local | [main](local/main.omg) | [invalid](local/invalid/main.omg) |
| Field | [main](field/main.omg) | [invalid](field/invalid/main.omg) |
| Parameter | [main](parameter/main.omg) | [invalid](parameter/invalid/main.omg) |
| Call result | [main](call_result/main.omg) | [invalid](call_result/invalid/main.omg) |
| Match join | [main](match_join/main.omg) | [invalid](match_join/invalid/main.omg) |
| Const name | [main](const_name/main.omg) | [invalid](const_name/invalid/main.omg) |
| Another state | [main](other_state/main.omg) | [invalid](other_state/invalid/main.omg) |
| Another machine | [main](other_machine/main.omg) | [invalid](other_machine/invalid/main.omg) |
| Another target body | [main](target_body/main.omg) | [invalid](target_body/invalid/main.omg) |

`omega run --both <place>/main.omg` reports Terminal Psi and native exit 0.
Each twin drops only the right operand's `<= 50` obligation and refuses with
`cannot prove requires contract for call apply`.

## Requirements and evidence

1. **Establishment.** `Selected`'s inline `combine` establishes
   `result == left + right` under the trait's requires. `apply` returns that
   call, so its ensures is the published member contract after evidence
   substitution, not a caller equality guard after the call.
2. **Identity and lifetime.** The operands are the same `i32` values at the
   dispatch: locals, record fields, parameters, call results, match arms, a
   const ceiling, a successor state, a forwarding machine, or a target body.
   No write sits between the bound and the call.
3. **Composition.** Every place copies the same sentence and consumes
   `result == left + right` from the dispatched call. Each twin changes only
   the right operand's bound, so the call's requires no longer holds.
4. **Soundness boundaries.** A specialized body keeps the public requirement
   contract. The selected member body does not replace that contract. An
   ensures of `result == left + right + 1` still refuses. A missing operand
   bound still refuses.
5. **Checked-to-Terminal Psi lowering (Psi 05–07).** The evidence dispatch
   lowers to the selected inline member. Proof obligations are checked and
   erased. `omega run --both` agrees on exit 0 for the root and all nine places.
6. **Provider settlement, layout geometry, and ABI calling plans (Omega 00–02).**
   Each positive binds `ProgramEntry` for all four hosted targets. The `i32`
   operands and result use the ordinary integer calling plan.
7. **Instruction selection, register homes, and activation frame realization
   (Omega 03–06).** Native execution of the addition uses the host ISA integer
   add and the ordinary frame for `main`. Windows x86-64 is the host that
   executed this row.
8. **Form encoding, branch relaxation, relocations, and native image emission
   (Omega 07–09 + emission).** `omega run --both` emits and runs the Windows
   image. The other three targets are bound and checked; their native images
   were not executed on this host.

Matching-host execution for this repair is Windows x86-64. Linux x86-64,
macOS ARM64, and Linux ARM64 were not re-executed after the ensures regression.

## Run record

The check-tier ensures regression stamped `@5f668826adf3` is repaired on main
by reading the caller's public requirement contract in
`selected_call_site_contract`. This row does not add a second lookup.

Fresh: Windows x86-64, producer at the parent of this record.
`omega run --both` on the root and all nine places returns native exit 0 and
Terminal Psi exit 0. The nine reject twins still refuse
`cannot prove requires contract for call apply`. A caller ensures of
`result == left + right + 1` still refuses.

Retained: producer `7fc0404c1bcc`, target `macos_arm64`, recorded native and
Terminal exit 0 for the root and nine places before that check-tier regression.
That host was not re-executed here.

Remaining on **NAMED-IMPLEMENTATION-CONFORMANCE-MIGRATION**: const-expression
closure and package projection outside the qualified scalar inline path;
calling-policy and member kinds beyond the closed inline path; product-producer
parity; retirement of `Trait::member = Existing::machine` reference rows,
including boundary roots; other hosts for this row and the earlier binder,
open-const, and calling-policy rows. `open_record_const_implementation` is a
separate witness.
