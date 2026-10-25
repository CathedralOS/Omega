# Checked assembly addition carry

Adding one to the all-ones u32 establishes addition flags 85 under mask 2261; a structured snapshot carries that image to a label.

Owning requirement: **CHECKED-ASM-LABEL-CONTRACTS**. This row is the u32 `add`
flag production that subtraction's snapshot row does not exercise. Carry is the
unsigned overflow of the sum, and signed overflow is set only when both
operands share a sign that the sum does not. Parity, auxiliary carry, zero and
sign still follow the wrapped result. The governing
[checked assembly contract](../../../../wiki/spec/language/assembly.md) requires
exact edge proofs and executable instruction semantics.
[Numeric values](../../../../wiki/spec/language/numeric_values.md) own the
fixed-carrier bit operations.

## Transport places

The nine standard places and their `<place>/invalid/` reject twins copy the
sentence above. The root witness does too.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [source](local/main.omg) | [cleared carry bit](local/invalid/main.omg) |
| field | [source](field/main.omg) | [subtraction image demanded](field/invalid/main.omg) |
| parameter | [source](parameter/main.omg) | [cleared carry bit](parameter/invalid/main.omg) |
| call result | [source](call_result/main.omg) | [cleared carry bit](call_result/invalid/main.omg) |
| match join | [source](match_join/main.omg) | [one arm clears carry](match_join/invalid/main.omg) |
| const name | [source](const_name/main.omg) | [named image is the subtraction image](const_name/invalid/main.omg) |
| a nested machine | [source](other_state/main.omg) | [cleared carry bit](other_state/invalid/main.omg) |
| another machine | [source](other_machine/main.omg) | [cleared carry bit](other_machine/invalid/main.omg) |
| another target body | [source](target_body/main.omg) | [subtraction image demanded](target_body/invalid/main.omg) |

Each positive is pinned under `tests/omega/pass/facts/checked_asm_add_carry*`.
Each reject twin is pinned under `tests/omega/fail/facts/checked_asm_add_carry*`.
The [root](main.omg) and its [false-image twin](invalid/main.omg) are pinned as
`checked_asm_add_carry` and `checked_asm_add_carry_false_image`. A twin changes
only the carried image: it demands 149, the subtraction image of `0 - 1`, or
clears carry with `^ 1u64`. The addition itself stays `mov eax, 4294967295;
add eax, 1`.

Build entries bind Linux and Windows x86-64 `ProgramEntry` only. `add` is
catalogued `AsmTargetApplicability::X86_64`, so an arm64 entry is other
ISA/register-view work, not a defect in this row. This row does not meet the
four-hosted-target form.

## Requirement coverage

- **Establishment:** `add eax, 1` after `mov eax, 4294967295` establishes the
  flag image. The label `requires (snapshot & 2261u64) == 85u64` consumes that
  image directly. No caller equality guard re-establishes it.
- **Identity and lifetime:** the snapshot is the arithmetic-flag version
  written by this addition. A later `^ 1u64` retires the carry bit, and the
  twin that passes the cleared value cannot discharge the original image.
- **Composition:** the same image is the callee precondition, the call
  `ensures`, the match join of two identical additions, the named `ADD_FLAGS`
  constant, the next state's parameter, and the forwarded machine argument.
  Both target bodies keep the same addition and the same label obligation.
- **Soundness boundaries:** the addition acquires no authority and retires no
  linear claim. Proof-only label obligations do not grant a runtime access.
  The row does not claim hardware-effect retirement.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** the addition lowers to
  ordinary wrapping addition plus the width-total flag image. Terminal
  execution consumes those operations. `omega run --both` reports native exit
  0 and interp exit 0 on every positive place.
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):**
  hosted x86-64 `ProgramEntry` rows select the body. The snapshot is an
  ordinary `u64` place under the existing layout and calling plans.
- **Instruction selection, register homes, and activation frame realization
  (`Omega 03–06`):** the lowered addition and flag arithmetic use the existing
  integer selection, register homes and stack frame. Structured `pushfq` keeps
  its balanced observation envelope.
- **Form encoding, branch relaxation, relocations, and native image emission
  (`Omega 07–09` + emission):** the Linux x86-64 image executes and agrees with
  Terminal Psi. Windows x86-64, macOS and QEMU were not executed on this host.

## Evidence

Frozen baseline: revision `c2e06b875ba2f6a205dd49b408d937536833b190`.
`omega --check samples/cli/facts/checked_asm_add_carry/local/main.omg` rejected
at `local/main.omg:8:3` with `unknown asm instruction add` before the catalog
grew an addition contract. The source and the 85-under-2261 oracle were frozen
first.

After the repair, on Linux 6.12.94+ x86_64, the root and all nine places
completed `omega run --both` with native exit 0 and interp exit 0. All ten
reject twins, including the root, failed `omega --check` with `cannot prove`
on the broken image. The producer was the optimized dev build of this repair.

Other flag-producing instructions (`cmp`, logical updates, shifts), other
ISA/register views, and exact linear hardware-effect retirement remain open
under the parent task. This row does not close that acceptance.
