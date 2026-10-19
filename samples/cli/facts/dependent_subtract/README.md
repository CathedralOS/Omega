# dependent_subtract

The relational subtraction rule: `count - used` under `used <= count` proves
exact and non-negative in exact `u32`, so the difference also fits exact
`as i32`. A `count <= 11` premise keeps the destination conversion standing.

The row is pinned by `tests/omega/{pass,fail}/facts/dependent_subtract_through_*`;
every positive records native exit 0.

## Places

| Place | Positive | Fact-sensitive reject twin |
| --- | --- | --- |
| root | [main.omg](main.omg) | — |
| local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) |
| field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) |
| parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) |
| call_result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) |
| match_join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) |
| const_name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) |
| other_state | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) |
| other_machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) |
| target_body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) |

Every positive (including the root witness) binds `ProgramEntry` for `windows_x86_64`,
`linux_x86_64`, `macos_arm64` and `linux_arm64` in its `build.omg`, declares
`Expected exit: 0`, and carries an authored `expected_native_exit.txt` of zero.
Each place copies the same fact sentence and the same subtraction operation
(`count - used`); only how the two operands reach that operation changes.

Run one place or root directly:

```sh
omega --check samples/cli/facts/dependent_subtract/main.omg
omega run --both samples/cli/facts/dependent_subtract/main.omg
```

`omega run --both` reports the native exit and the Terminal Psi exit; agreement
at zero is the place's runtime oracle.

## Reject twins

Each twin keeps the operation, the record and the construction shape identical
and removes only the `used <= count` premise at the subtraction site. Every twin
rejects with exit 1 at the `subtracted` call:

```
error: exact arithmetic in machine `subtracted` ... local `remaining` may
overflow `u32`: the operands are not provably in range
```

The `target_body` twin reports it once for each of its four target bodies.
Check a twin with `omega --check samples/cli/facts/dependent_subtract/<place>/invalid/main.omg`.

## Requirements and Evidence

- **Parent Outcome:** `CONSERVATION-CONTRACT` / `CANONICALIZE-SCALAR-RANGE-CONTRACTS` (`TASKS.md`).
- **Requirement Established:** A relational bound (`used <= count`) proves exact integer subtraction non-negative (`count - used >= 0`) and in range (`as i32` under `count <= 11`) across source checking, Terminal Psi representation, backend code generation, and native execution across all nine places.

### Full 8-Layer Specification and Lowering Evidence

- **Establishment:** The relational/dependent bound `requires used <= count && count <= 11` (or local/field/call contract) establishes that the exact subtraction `count - used` cannot underflow in `u32`, without any post-hoc caller equality guard (`transition x == 100`). The precondition introduces the relational constraint between the two operands directly into the proof environment prior to evaluating the subtraction expression.
- **Identity and lifetime:** The relational fact between `count` and `used` tracks the exact scalar values or field/parameter versions across value copies, record construction, parameter passing, and state hops. The subtraction consumes those exact scalar values; intervening writes or re-assignments to either operand would invalidate the relational fact unless re-established.
- **Composition:** The dependent subtraction fact travels through all 9 places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) to prove the consuming subtraction, and `<place>/main.omg` vs. `<place>/invalid/main.omg` differ only in the construct's own bound or operand relationship (removing the `used <= count` premise).
- **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed with underflow/contract diagnostics (`error: exact arithmetic in machine ... local `remaining` may overflow `u32`: the operands are not provably in range`) when the relational bound does not hold on every path, refusing to compile rather than admitting potential arithmetic underflow.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** The relational proof obligation and integer subtraction certificate lower into canonical target-neutral Terminal Psi (`PSITERM\0`) exact integer subtraction without runtime trap overhead where proved, and execute in the Terminal Psi interpreter (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across the four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`), record/scalar layout geometry validates (`data Main where count <= 11 { count: u32; j: u32; out: i32; }`), and `CallPlan`/`StatePlan` place operands and results across registers and stack slots according to target ABIs.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects native integer subtraction (`sub`) and comparison instructions, assigns register homes and disjoint stack-frame storage with live-across-call preservation, and inserts entry prologues and multi-return epilogues.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic stack-frame displacements resolve, basic blocks and branches relax with offset reflow, machine-state footprint evidence validates, and direct PE/ELF/Mach-O emission agrees with Terminal Psi (`omega run --both` exit 0) across hosted targets including `windows_x86_64` and `macos_arm64`.

### Verification Evidence

- `sample_native_gate.py --filter cli/facts/dependent_subtract`: 10/10 samples meet their documented oracle on windows_x86_64 (root + 9 places pass with exit 0); 9/9 intended-invalid controls match their authored rejection (`may overflow `u32``).
- The oracle discriminates: corrupting the subtraction expectation in `entry()` triggers the `bad()` state and traps with `crash Trap`, proving the exit 0 oracle actively validates the computation.
- Remaining open: relational/dependent arithmetic across variable array slice lengths and non-linear multi-variable constraints under `CONSERVATION-CONTRACT`.
