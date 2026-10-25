# Scalar Width Shifts

A bounded shift count `shift < 4` on a `Wrapping` `u32` value preserves
in-width shift validity and recovers the original scalar across all nine places.

Every place performs the same operation: shift a `u32 in Wrapping` value (`21`) left
by a bounded shift count (`1`), verify the shifted scalar is `42`, shift right by the
same count to recover the original scalar `21`, and exit 0 only when both hold.
Only the transport of the operands changes.

## Places

| Place | Positive | Fact-sensitive reject twin | Transport |
| --- | --- | --- | --- |
| root | [main.omg](main.omg) | — | root wrapping u32 scalar value and shift count |
| local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) | both the Wrapping u32 value and bounded shift count are copied into locals |
| field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) | bounded record fields carry the Wrapping u32 value and shift count |
| parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) | the shift machine reads its parameters directly |
| call_result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) | calls with declared types and bounds supply both operands |
| match_join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) | both arms keep the operands inside Wrapping and bounds, so the joined operands shift safely |
| const_name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) | `const SHIFT: u32 = 1` provides the named shift count |
| other_state | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) | a successor machine receives the operands and shifts there |
| other_machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) | a forwarding machine transports operands to the shift machine |
| target_body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) | each of four target bodies shifts the Wrapping u32 value and shift count |

Each positive place and the root `main.omg` binds `ProgramEntry` for all four
hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`),
declares `Expected exit: 0`, and passes `omega run --both` — Terminal Psi and
native execution agreeing:

```sh
omega run --both samples/cli/facts/scalar_width_shifts/main.omg
omega run --both samples/cli/facts/scalar_width_shifts/<place>/main.omg
```

## Reject twins

Each place has an `invalid/` child that breaks the shift/width contract by using an
unqualified `u32` value without `in Wrapping`; that twin differs from its positive
only in the construct's own domain. It carries its authored rejection expectation
in `invalid/sample.json` (`rejection_contains`) and `invalid/expected.txt`, so the
control rejects with `may overflow `u32`` rather than for an unrelated reason.
Check a twin with `omega --check samples/cli/facts/scalar_width_shifts/<place>/invalid/main.omg`.

| Place | Reject Twin | Authored diagnostic fragment |
| --- | --- | --- |
| local | [local/invalid/main.omg](local/invalid/main.omg) | `may overflow `u32`` |
| field | [field/invalid/main.omg](field/invalid/main.omg) | `may overflow `u32`` |
| parameter | [parameter/invalid/main.omg](parameter/invalid/main.omg) | `may overflow `u32`` |
| call_result | [call_result/invalid/main.omg](call_result/invalid/main.omg) | `may overflow `u32`` |
| match_join | [match_join/invalid/main.omg](match_join/invalid/main.omg) | `may overflow `u32`` |
| const_name | [const_name/invalid/main.omg](const_name/invalid/main.omg) | `may overflow `u32`` |
| other_state | [other_state/invalid/main.omg](other_state/invalid/main.omg) | `may overflow `u32`` |
| other_machine | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) | `may overflow `u32`` |
| target_body | [target_body/invalid/main.omg](target_body/invalid/main.omg) | `may overflow `u32`` |

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`)
- **Owning requirement:** `wiki/spec/language/numeric_values.md#shift-counts`
- **Reserved ticket:** `8bfead69f1114e37831c300710f126ca`
- **Requirement established:** In-width scalar shifts (`val << shift`, `shifted >> shift`) preserve validity and recover the original scalar under bounded shift count (`shift < 4` on 32-bit width) across source checking, Terminal Psi representation, backend code generation, and native execution across all nine places.

### Full 8-Layer Specification and Lowering Evidence

- **Establishment:** The shift-amount bound `requires shift < 4` (provably strictly less than the scalar bit width 32) together with the `u32 in Wrapping` domain establish well-defined left and right shift semantics and post-shift interval bounds directly without any post-hoc caller equality guard (`transition x == 42`). The preconditions guarantee in-width shift validity (`shift < 32`) and discharge left-shift representability prior to evaluating the shift expressions.
- **Identity and lifetime:** The scalar operands (`val: u32 in Wrapping` and `shift: u32`), their static bounds (`shift < 4`), and the shifted/recovered result facts stay bound to the exact scalar subjects across value copies, struct fields, machine parameters, call return results, match join bindings, named constants, state transitions, machine forwarding, and target-scoped bodies.
- **Composition:** Scalar width shift facts compose through all 9 places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming shift operations (`<<`, `>>`) and identity comparisons. Each positive place `<place>/main.omg` vs. its reject twin `<place>/invalid/main.omg` differs only in the construct's own domain (omitting `in Wrapping` while retaining the identical structure and shift operations).
- **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed with compile-time verification diagnostics (`error: exact arithmetic in machine ... may overflow `u32``) when the operand lacks `in Wrapping`, refusing compilation rather than admitting unproven left-shift representability or potential overflow.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Proved shift-width certificates and signed/unsigned wrapping shift operations lower without semantic loss into canonical target-neutral Terminal Psi (`PSITERM\0`), erasing proof-only obligations while retaining integer certificates, state transitions, and target-scoped bodies, and executing with full agreement in the Terminal Psi interpreter (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across the four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). `IntegerAt` 32-bit scalar layout geometry and record layout (`data ShiftState where shift < 4 { val: u32 in Wrapping; shift: u32; }`) validate, and `CallPlan`/`StatePlan` lower 32-bit scalar and aggregate carriers across registers and stack slots according to target ABIs.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target shift instructions (`shl`/`shr`/`sar` with count in `CL` or immediate on `x86_64`; `lsl`/`lsr`/`asr` on `arm64`), assigns register homes and disjoint stack-frame storage with live-across-call spill slots, and inserts entry prologues and multi-return epilogues.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Narrow/wide shift instruction forms and symbolic stack-frame displacements encode, basic blocks and branch offsets relax with offset reflow, machine-state footprint evidence validates, and direct PE/ELF/Mach-O emission agrees with Terminal Psi (`omega run --both` exit 0) across hosted targets including `windows_x86_64` and `macos_arm64`.

### Verification Evidence

- `sample_native_gate.py --filter cli/facts/scalar_width_shifts`: 10/10 samples meet their documented oracle on `windows_x86_64` (root + 9 places pass with exit 0); 9/9 intended-invalid controls match their authored rejection (`may overflow `u32``).
- Discrimination: corrupting the shift amount (`1` -> `3`, so `21 << 3 = 168` is no longer `42`) or corrupting the expected shifted value (`42` -> `84`) triggers the `bad()` state and traps with `crash Trap`, proving the exit 0 oracle actively discriminates computation errors.
- Pinned fixtures: `tests/omega/pass/facts/scalar_width_shifts_through_*` and `tests/omega/fail/facts/scalar_width_shifts_through_*` pin this row.
- Remaining open: full interval-set proof propagation and theorem propagation for non-wrapping arithmetic on dynamic counts, and cross-carrier shift count reduction proofs on composite structures under `CONSERVATION-CONTRACT`.

