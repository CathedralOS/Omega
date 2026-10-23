# signed_8bit_shift_right_bound

// Fact: an 8-bit signed i8 arithmetic right shift of a bounded input (val >= 100i8 && val <= 100i8) by 2u32 bits establishes shifted >= 25i8 && shifted <= 25i8 so exact 8-bit signed subtraction shifted - 25i8 is proved overflow-free across all nine places.

## Places

| Place | Positive Status | Rejection Twin Diagnostic |
|---|---|---|
| root (`main.omg`) | passes `omega run --both` exit 0 | N/A |
| `local` | passes `omega run --both` exit 0 | `cannot prove requires contract for call verify_shifted` from `compute_local` |
| `field` | passes `omega run --both` exit 0 | `cannot prove requires contract for call verify_shifted` from `compute_field` (`container.shifted`) |
| `parameter` | passes `omega run --both` exit 0 | `cannot prove requires contract for call verify_parameter` from `compute_param` |
| `call_result` | passes `omega run --both` exit 0 | `cannot prove ensures contract for exit from shift_right` at statement 0 |
| `match_join` | passes `omega run --both` exit 0 | `cannot prove ensures contract for exit from choose_shifted` at statement 0 |
| `const_name` | passes `omega run --both` exit 0 | `cannot prove requires contract for call verify_shifted` from `compute_const` |
| `other_state` | passes `omega run --both` exit 0 | `cannot prove requires contract for call step` from `verify_state` |
| `other_machine` | passes `omega run --both` exit 0 | `cannot prove requires contract for call forward` from `compute_forward` |
| `target_body` | passes `omega run --both` exit 0 | `cannot prove requires contract for call verify_shifted` from `verify_target` |

Every positive place and the root `main.omg` bind `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64`, declare
`Expected exit: 0`, and carry `expected_native_exit.txt` = 0.

## Pinned Fixtures

The row is pinned under `tests/omega/pass/facts/signed_8bit_shift_right_bound_through_<place>_exit`
and `tests/omega/fail/facts/signed_8bit_shift_right_bound_through_<place>_rejected` for the nine
places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`,
`other_machine`, `target_body`). Each pass fixture is a byte-identical copy of its sample place's
`main.omg`/`build.omg` plus `expected_native_exit.txt` = 0; each fail fixture is a byte-identical copy
of its place twin's `main.omg`/`build.omg`/`expected.txt`. Positive versus twin `git diff` changes only
the fact's own domain — the input bound (`val >= 100i8 && val <= 100i8` becomes `val <= 104i8`), the
probe argument (`100i8` becomes `104i8`), the const value (`VAL = 100i8` becomes `104i8`) or the four
target-body `let val: i8 = 100i8` initializers — plus the header comments naming the place, the invalid
intent and the expected diagnostic.

Measured on macOS arm64 with the factsr1 shared generation producer `gen-9b001b8f6af` (omega sha256
`6d24393125b7847b2f46d2f25e856f18a644f0fa1cedd2ad9b54f316f8b7da74`, corpus_runner sha256
`cee00bbdce17fc300b548aca249c2e9e2a44e59c17e97186055da76c3947444e`):

- `omega run --both` over the root and the nine places: 10/10 report `native exit 0` and
  `interp exit: 0`.
- `omega --check` over the nine twins: 9/9 exit 1 with their authored
  `cannot prove requires contract ...` / `cannot prove ensures contract ...` fragment, and the corpus
  check leg agrees — 9 of 9 twins `rejected` with `expected_satisfied`, `0 rejected without
  their expected fragment`, `0 fail fixtures did not reject`.
- The three corpus legs were recorded with the fixture changes untracked in a working tree whose HEAD
  is the published `644a492d2c3b`, so every row is stamped `@644a492d2c3b+wt`:
  `tests/omega/corpus_outcomes.txt` gained 36 rows (18 `checked` carrying `check-target:macos_arm64`
  and 18 `rejected` with 24 diagnostic continuation lines);
  `tests/omega/corpus_native_macos_arm64.txt` gained 18 `built ... exit:0` rows and
  `tests/omega/corpus_both_macos_arm64.txt` gained 18 `built ... exit:0 interpreter-exit:0
  interpreter:agrees` rows. Every golden diff is a pure addition — 60/0, 18/0 and 18/0 lines, 0
  `timeout` rows.

## Requirements and Evidence

- **Parent outcome:** `CANONICALIZE-SCALAR-RANGE-CONTRACTS` (`TASKS.md`): the settled
  [proposition-based scalar range contract](wiki/spec/language/domains.md#declaration-and-membership),
  where machine/state parameters carry `requires`, results carry `ensures`, and locals take their facts
  from initializers, guards and calls. This row is one customer group's nine-place transport and
  execution of that contract; the underlying numeric-policy realization is `ARITHMETIC-POLICY-REALIZATION`.
- **Requirement established:** An 8-bit signed arithmetic right shift `val >> shift` of a
  single-valued bounded input (`val >= 100i8 && val <= 100i8`) by an exact shift count
  (`shift == 2u32`) establishes the exact shifted interval `[25, 25]`
  (`shifted >= 25i8 && shifted <= 25i8`), which discharges the consuming exact 8-bit subtraction
  `shifted - 25i8` in `verify_shifted` without a post-hoc caller equality guard, across all nine
  places, with exact agreement between Terminal Psi and native execution (`omega run --both` exit 0).
- **Establishment:** Performing an 8-bit signed arithmetic right shift `val >> shift` with bounded
  non-negative input (`val >= 100i8 && val <= 100i8`) and exact shift count `shift == 2u32` establishes
  exact shifted interval bounds `[25, 25]` (`shifted >= 25i8 && shifted <= 25i8`). This interval proves
  that the exact 8-bit signed subtraction `shifted - 25i8` produces `0i8` without overflow or underflow
  risk. In invalid twins, widening the input bound to `val <= 104i8` allows shifted result `26i8`, which
  fails closed at compile time against `requires shifted <= 25i8` (or `ensures result <= 25i8`).
- **Identity and lifetime:** The signed 8-bit shifted scalar retains its exact 8-bit two's complement
  integer value (`0x19`) across local bindings, struct fields (`ShiftContainer`), parameter registers,
  function call return values, match join points, named constants, state transitions, machine
  invocations, and target-scoped bodies without truncation, loss of high-bit sign extension, or storage
  degradation.
- **Composition:** The established interval `[25, 25]` composes across all nine places, safely
  discharging downstream contracts `requires shifted >= 25i8 && shifted <= 25i8` and executing exact
  subtraction `shifted - 25i8` without post-hoc caller guards.
- **Soundness boundaries:** Arithmetic right shift requires a shift count strictly within the bit-width
  of the operand (`shift < 8` for `i8`). Static verification certificates guarantee in-range shift counts
  (`< 8`). Weakened input intervals in invalid controls cannot establish downstream exact value
  preconditions, rejecting deterministically at check time.
- **Checked-to-Terminal Psi lowering (`Psi 05-07`):** Target-neutral Terminal Psi (`PSITERM\0`) receives
  arithmetic right shift operations (`Shr` on signed 8-bit carrier `i8`) with shift-count validity
  certificates and range bounds intact. Erased proof obligations and verified intervals lower without
  loss into executable nodes that execute in the Terminal Psi interpreter (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00-02`):** Signed 8-bit integers
  (`i8`) settle into 1-byte layouts (`size_of = 1`, `align_of = 1`). Standard calling conventions place
  parameters in 8-bit register lanes (`CL`/`DL` on Windows `x86_64`, `DIL`/`SIL` on Linux `x86_64`,
  `W0`/`W1` on `arm64`) and return scalar results in `AL` / `W0`.
- **Instruction selection, register homes, and activation frame realization (`Omega 03-06`):** On
  `x86_64`, backend lowering selects arithmetic right shift `sar r8, imm8` (or `sar r8, cl` with shift
  count masked/truncated to 8-bit shift bounds), preserving the sign bit across the shift. On `arm64`,
  8-bit values are sign-extended into 32-bit registers via `sxtb`, shifted using arithmetic shift right
  `asr w, w, #imm`, and masked or truncated back to 8 bits.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07-09` + emission):**
  Direct PE/ELF/Mach-O image emission resolves all displacements, emits valid native machine code, and
  executes with exit code 0 matching Terminal Psi simulation across all hosted targets.
- **What remains open:** the same fact at wider signedness/width pairs (already carried by sibling
  rows), and any opt-in optimization specialization; nothing in this row's nine places remains.