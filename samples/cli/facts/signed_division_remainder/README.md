# Signed Division Remainder

Fact: computing the signed remainder (%) of a non-negative bounded i64 dividend by a positive bounded i64 divisor (divisor == 4i64) establishes the remainder bound (0 <= rem < 4) to safely index a 4-element array across all nine places.

Every place performs the same operation: compute the signed remainder of non-negative dividend `14i64` by divisor `4i64` (`14 % 4 == 2`), cast the remainder to `u64` index `2u64`, use it to index a 4-element fixed array `[10, 20, 30, 40]`, yielding `30u8`, verify `probe() == 30`, and exit 0. Only the transport mechanism of the operands and divisor bound changes across places.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | root signed remainder indexing fixed array |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | the dividend and divisor are held in local variables before remainder and indexing |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | a record field holds the dividend and divisor before remainder and indexing |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | explicit machine parameters receive the operands and compute remainder |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | a contracted helper call returns the divisor before remainder and indexing |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms return the contracted divisor and join before remainder |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | a named constant supplies the divisor before remainder and indexing |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a successor machine receives the operands and indexes the array |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | an intermediate machine forwards the operands to the indexing machine |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each of four target bodies performs remainder and indexes the array |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` and declares
`Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/signed_division_remainder/main.omg
omega run --both samples/cli/facts/signed_division_remainder/local/main.omg
omega run --both samples/cli/facts/signed_division_remainder/field/main.omg
omega run --both samples/cli/facts/signed_division_remainder/parameter/main.omg
omega run --both samples/cli/facts/signed_division_remainder/call_result/main.omg
omega run --both samples/cli/facts/signed_division_remainder/match_join/main.omg
omega run --both samples/cli/facts/signed_division_remainder/const_name/main.omg
omega run --both samples/cli/facts/signed_division_remainder/other_state/main.omg
omega run --both samples/cli/facts/signed_division_remainder/other_machine/main.omg
omega run --both samples/cli/facts/signed_division_remainder/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- At runtime, `14i64 % 4i64 == 2i64`, selecting element `arr[2] == 30u8`. The transition `probe() == 30 { true -> done() _ -> bad() }` exits 0. If remainder evaluation is corrupted (producing an incorrect index) or returns an element other than 30, execution transitions to `machine bad() { crash Trap; }`, causing a native signal 5 / interpreter `Trap` crash (exit 255).
- In each invalid twin, `divisor` is widened to `5i64`, so the remainder interval is `[0, 4]`, which can produce index `4`. Indexing a 4-element array `[u8; 4]` with an index that may equal 4 violates array bounds safety at compile time, strictly failing closed during proof checking with `cannot prove index \`idx\` is within length 4`. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/signed_division_remainder/<place>/invalid/main.omg
```

### Acceptance Gate

```sh
python tools/sample_native_gate.py --cli C:\SoftwareDevelopmentKits\Omega\build\omega-latest.exe --filter cli/facts/signed_division_remainder --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64` (and `linux_x86_64`).
- 9/9 intended-invalid controls match their authored rejection (`cannot prove index \`idx\` is within length 4`).

## Pinned Fixtures

The row is pinned by `tests/omega/pass/facts/signed_division_remainder_through_*_exit` and
`tests/omega/fail/facts/signed_division_remainder_through_*_rejected` fixtures: every positive
place records native exit 0 with Terminal Psi execution agreement, and every reject twin records
its `cannot prove index \`idx\` is within length 4` diagnostic.

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`).
- **Ticket:** `7ded45bde7c64d8e81894fdc953dda48`.
- **Requirement established:** End-to-end nine-place fact preservation and native realization for signed division remainder. Specifically, computing the signed remainder (`%`) of a non-negative bounded `i64` dividend (`dividend >= 0i64`) by a positive bounded `i64` divisor (`divisor == 4i64`) establishes the exact remainder interval (`0 <= rem < 4`), discharging the array index bound check `idx < 4` directly across all nine language places without post-hoc caller equality guards, with exact agreement between Terminal Psi and native execution across hosted targets (`omega run --both` exit 0).

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** The precondition `dividend >= 0i64 && divisor == 4i64` establishes non-negative quotient and remainder properties, ensuring `0 <= dividend % divisor < divisor` (i.e. `0 <= rem < 4`). Casting `rem as u64` preserves the exact interval `[0, 3]`, which discharges the fixed-size array indexing obligation `idx < 4` on `arr: [u8; 4]` directly without requiring any post-hoc caller equality guard (`transition x == 30`).
  - **Identity and lifetime:** The signed dividend, divisor, remainder value, and array element retain their exact scalar values and static bounds across local copy, record field projection, explicit parameter, call result return, match-expression join, named constant bound, state transition, forwarding machine, and target-scoped machine bodies.
  - **Composition:** The signed remainder bound composes through all 9 fact places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming array indexing operation (`arr[idx]`). In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the divisor contract bound (`divisor == 4i64` vs `divisor == 5i64`), while preserving all other signatures, array definitions, and caller structures.
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed during compile-time proof checking when the divisor is widened to `5i64`: the remainder interval widens to `[0, 4]`, which fails the 4-element array length proof obligation, strictly rejecting compilation with `cannot prove index \`idx\` is within length 4`. No out-of-bounds array access reaches native execution.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Signed remainder arithmetic (`%`), integer conversions, and fixed-size array indexing lower into canonical target-neutral Terminal Psi (`PSITERM\0`), erasing mathematical proof obligations while preserving the signed modulo operation and concrete array bounds. All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) through companion build declarations without reopening source. The array layout (`[u8; 4]`: size 4, align 1) and scalar layouts (`i64`: size 8, align 8) validate, and target `CallPlan`/`StatePlan` assign arguments and return homes to target register lanes and stack slots per ABI.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA signed remainder sequences: on `x86_64`, the signed remainder is computed via sign-extending `RAX` into `RDX:RAX` via `cqo`, executing signed divide `idiv`, and reading the remainder from `RDX`; on `arm64`, the signed remainder is computed via signed division `sdiv` followed by multiply-subtract `msub` (`rem = dividend - quotient * divisor`). Array indexing lowers to scaled base+index address loads (`movzx` on `x86_64`, `ldrb` on `arm64`). Register homes and activation stack-frame slots are assigned, and entry prologues and multi-return epilogues honor the `StatePlan` ceiling.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Target ISA instructions (`cqo`, `idiv`, `sdiv`, `msub`, memory address displacements) encode into machine bytes, basic blocks and jump targets relax with offset reflow, machine-state footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates against target host limits, and direct PE/ELF/Mach-O executables are emitted. Terminal Psi interpretation and native execution agree with exit 0 (`omega run --both`) across host platforms including `windows_x86_64` and `macos_arm64`.

- **What remains open:** Truncation vs. floor rounding discrimination for negative dividends with non-zero remainder, dynamic divisor interval proofs, and non-constant slice indexing bounds.
