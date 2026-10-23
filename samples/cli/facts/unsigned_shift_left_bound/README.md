# Unsigned Shift Left Bound

Fact: logical left shift (<<) on a bounded unsigned u64 value (25u64 << 2) establishes the exact post-shift bound (100u64), discharging exact subtraction shifted - 100u64 across all nine places.

Every place performs the same operation: shift a bounded unsigned `u64` operand (`25u64`) left by `2` (`25u64 << 2 == 100u64`), perform exact subtraction `shifted - 100u64`, verify `probe() == 0`, and exit 0. Only the transport mechanism of the operands and shift computation changes across places.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | root unsigned logical left shift and exact subtraction |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | the operand and shift count are copied into locals before left shift and subtraction |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | a record field holds the operand and shift count before left shift and subtraction |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | explicit machine parameters receive the bounded operand and shift count |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | a contracted helper call returns the bounded operand before left shift and subtraction |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms return the bounded operand and join before left shift and subtraction |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | a named constant supplies the bounded operand before left shift and subtraction |
| another state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a successor state receives the bounded operand and shift count before left shift and subtraction |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | an intermediate machine forwards the bounded operands to the shift machine |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each of four target bodies shifts the bounded operand and subtracts 100u64 |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` and declares
`Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/unsigned_shift_left_bound/main.omg
omega run --both samples/cli/facts/unsigned_shift_left_bound/local/main.omg
omega run --both samples/cli/facts/unsigned_shift_left_bound/field/main.omg
omega run --both samples/cli/facts/unsigned_shift_left_bound/parameter/main.omg
omega run --both samples/cli/facts/unsigned_shift_left_bound/call_result/main.omg
omega run --both samples/cli/facts/unsigned_shift_left_bound/match_join/main.omg
omega run --both samples/cli/facts/unsigned_shift_left_bound/const_name/main.omg
omega run --both samples/cli/facts/unsigned_shift_left_bound/other_state/main.omg
omega run --both samples/cli/facts/unsigned_shift_left_bound/other_machine/main.omg
omega run --both samples/cli/facts/unsigned_shift_left_bound/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- At runtime, `25u64 << 2 == 100u64`, and exact subtraction `100u64 - 100u64 == 0u64`. The transition `probe() == 0 { true -> done() _ -> bad() }` exits 0. If left shift evaluation is corrupted (e.g. failing to shift or shifting by the wrong count) or produces a non-zero diff, execution transitions to `state bad() { crash Trap; }`, causing a native signal 5 / interpreter `Trap` crash (exit 255).
- In each invalid twin, the operand is bounded to `24u64`, so `24u64 << 2 == 96u64`. Exact subtraction `shifted - 100u64` (`96u64 - 100u64`) underflows below zero, causing compile-time proof obligation checking to fail closed with `exact arithmetic in machine ... may overflow \`u64\``. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/unsigned_shift_left_bound/<place>/invalid/main.omg
```

### Acceptance Gate

```sh
python tools/sample_native_gate.py --cli C:\SoftwareDevelopmentKits\Omega\build\omega-latest.exe --filter cli/facts/unsigned_shift_left_bound --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64` (and `linux_x86_64`).
- 9/9 intended-invalid controls match their authored rejection (`may overflow \`u64\``).

## Pinned Fixtures

The row is pinned by 18 fixtures:
`tests/omega/pass/facts/unsigned_shift_left_bound_through_<place>_exit` and
`tests/omega/fail/facts/unsigned_shift_left_bound_through_<place>_rejected` for each of the nine places.
Each positive fixture is its place program copied byte-identical, binds `ProgramEntry` for all four hosted
targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`), and carries
`expected_native_exit.txt` = 0; each reject twin is the place's invalid program and `build.omg` copied
byte-identical, with `expected.txt` carrying the row's own `may overflow \`u64\`` fragment. The `git diff`
of a positive place against its twin moves only the construct's own operand bound (`val >= 25 && val <= 25`
vs `24u64`, `packet.val` in `field`, the helper's `ensures result >= 25` in `call_result`, `match_join`,
`const VAL: u64 = 25u64` in `const_name`, the forwarded argument in `other_machine`/`target_body`) plus the
call sites that feed it.

Every row was recorded with the shared generation producer (gen-d3b0020bf6a, revision d3b0020bf6a, macOS
arm64 host, jobs 4) on 2026-10-23, re-measured against current main's compiler and recorded with the
fixture and golden changes staged at that published base, so each row is stamped `@e42d5379cc30+wt`:

- `tests/omega/corpus_outcomes.txt`: nine `checked` rows carrying `check-target:macos_arm64` and nine
  `rejected` rows carrying each twin's own diagnostic line with its `may overflow \`u64\`` fragment
  satisfied (12 diagnostic continuation lines; `target_body` reports four target-scoped diagnostics per
  twin and the other eight one);
- `tests/omega/corpus_native_macos_arm64.txt`: nine `built` rows with `exit:0`;
- `tests/omega/corpus_both_macos_arm64.txt`: nine `built` rows with `exit:0`, `interpreter-exit:0` and
  `interpreter:agrees`.

Measured with the same base producer before the record: the root and all nine places run `omega run --both`
with `native exit 0` and `interp exit: 0` (10/10), and all nine twins reject with that fragment (9/9).
`python tools/sample_gate.py --filter cli/facts/unsigned_shift_left_bound --cli <generation omega>` reports
19/19 projects checked on Darwin arm64, nine of them matched intended rejection controls.

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`).
- **Ticket:** `8bfead69f1114e37831c300710f126ca`.
- **Requirement established:** End-to-end nine-place fact preservation and native realization for logical left shift (`<<`) bounds. Specifically, logical left shift on a bounded unsigned `u64` value (`25u64 << 2`) establishes the exact post-shift interval (`100u64`), discharging exact subtraction `shifted - 100u64` directly across all nine language places without post-hoc caller equality guards, with exact agreement between Terminal Psi and native execution across hosted targets (`omega run --both` exit 0).
- **Pinned fixtures:** the 18 fixtures named above; nine `checked` and nine `rejected` rows in `tests/omega/corpus_outcomes.txt`, nine `built ... exit:0` rows in `tests/omega/corpus_native_macos_arm64.txt`, and nine `built ... exit:0 interpreter-exit:0 interpreter:agrees` rows in `tests/omega/corpus_both_macos_arm64.txt`, all stamped `@e42d5379cc30+wt`.

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** The precondition contract `val >= 25 && val <= 25 && shift == 2` bounds the unsigned operand and specifies an in-range shift count strictly less than 64 (`shift < 64`). The logical left shift operation (`val << shift`) directly establishes the post-shift interval `[100, 100]` (`100u64`), discharging exact subtraction `shifted - 100u64` without requiring any post-hoc caller equality guard (`transition x == 0`).
  - **Identity and lifetime:** The unsigned scalar operand, shift count, and shifted result retain their exact 64-bit unsigned scalar identity and static interval across local variables, record fields, explicit parameters, call return results, match join bindings, named constants, state transitions, forwarding machines, and target-scoped machine bodies.
  - **Composition:** The left-shifted bound composes through all 9 fact places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming exact subtraction operation (`shifted - 100u64`). In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the operand bound (`25u64` vs `24u64`), while preserving all other signatures, shift operations, and caller structures.
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed during compile-time proof checking when the operand is weakened to `24u64` (so `24 << 2 == 96`): exact subtraction `96u64 - 100u64` underflows below zero, strictly rejecting compilation with `exact arithmetic in machine ... may overflow \`u64\``. No unproved underflow reaches native execution.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Unsigned logical left shift arithmetic (`<<`), shift amount validation, and exact subtraction lower into canonical target-neutral Terminal Psi (`PSITERM\0`), erasing proof obligations while retaining concrete bitwise operations and certified intervals. All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) through companion build declarations without reopening source. The 64-bit scalar layout (`u64`: size 8, align 8) and 32-bit shift count (`u32`: size 4, align 4) validate, and target `CallPlan`/`StatePlan` assign arguments and return homes to target register lanes and stack slots per ABI.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA logical left shift instructions (`shl` with count in `CL` or immediate on `x86_64`; `lsl` on `arm64`) and integer subtraction (`sub`). Low-level register allocators assign register homes and disjoint activation stack-frame slots, and entry prologues and multi-return epilogues honor the `StatePlan` ceiling.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Target ISA instructions (`shl`, `lsl`, `sub`, branch displacements) encode into machine bytes, basic blocks and jump targets relax with offset reflow, machine-state footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates against target host limits, and direct PE/ELF/Mach-O executables are emitted. Terminal Psi interpretation and native execution agree with exit 0 (`omega run --both`) across host platforms including `windows_x86_64` and `macos_arm64`.

- **What remains open:** Dynamic out-of-range shift count masking, 128-bit multi-word shift chains, and SIMD vector shift lanes.
