# Bounded products

Bounded `u64` factors `a <= 50` and `b <= 50` make the exact product
`a * b` fit in `result <= 2500`; the body stays a plain `a * b`, with no
`Wrapping` domain and no `as` widening.

Every place performs the same operation: multiply two operands whose declared
bound is `<= 50` and return the product under `ensures result <= 2500`. The oracle runs
both the interior `30 * 40 == 1200` and the upper endpoint `50 * 50 == 2500`
and exits 0 only when both hold. Only the transport of the two operands changes.

## Places

| Place | Positive | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | root entry copying bounded operands into locals |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | both bounded parameters are copied into locals |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | bounded record fields carry the operands |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | the multiplying machine reads its bounded parameters |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | calls with declared `ensures result <= 50` supply both operands |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both arms keep the operands in range, and both arms run |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | `const CEILING: u64 = 50` bounds both operands |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a successor machine receives both bounded operands |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | a forwarding machine transports the operands |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each of four target bodies multiplies |

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/bounded_products/main.omg
omega run --both samples/cli/facts/bounded_products/local/main.omg
omega run --both samples/cli/facts/bounded_products/field/main.omg
omega run --both samples/cli/facts/bounded_products/parameter/main.omg
omega run --both samples/cli/facts/bounded_products/call_result/main.omg
omega run --both samples/cli/facts/bounded_products/match_join/main.omg
omega run --both samples/cli/facts/bounded_products/const_name/main.omg
omega run --both samples/cli/facts/bounded_products/other_state/main.omg
omega run --both samples/cli/facts/bounded_products/other_machine/main.omg
omega run --both samples/cli/facts/bounded_products/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- Corrupting the runtime expectation in `main()` (e.g. `probe(30, 40) == 1201`) branches to `machine bad() { crash Trap; }`, causing native execution to terminate with signal 5 and Terminal Psi to terminate with `crash: Trap`, agreeing on exit 255.
- Each invalid twin removes an operand bound (or in `const_name`, widens the ceiling from 50 to 100), causing proof checking to fail closed with `may overflow` (or `cannot prove ensures contract`), preventing execution entirely.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/bounded_products/<place>/invalid/main.omg
```

### Acceptance Gate

```sh
python tools/sample_native_gate.py --cli C:\SoftwareDevelopmentKits\Omega\build\omega-latest.exe --filter cli/facts/bounded_products --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64` (and `macos_arm64`).
- 9/9 intended-invalid controls match their authored rejection.

## Pinned Fixtures

The row is pinned by `tests/omega/pass/facts/bounded_products_through_<place>_exit` and `tests/omega/fail/facts/bounded_products_through_<place>_rejected` fixtures.
The positive fixtures bind `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and record native exit 0.

## Requirements and Evidence

- **Parent outcome:** `CANONICALIZE-SCALAR-RANGE-CONTRACTS` (`TASKS.md`).
- **Requirement established:** End-to-end nine-place fact preservation and native realization for bounded scalar products. Specifically, declared operand bounds (`a <= 50`, `b <= 50`) make the exact integer product `a * b` provably representable under `result <= 2500` across all nine language transport places without domain annotation, integer widening, or post-hoc caller equality guards, with exact agreement between Terminal Psi and native execution across all four hosted targets.

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Declared precondition contracts (`requires a <= 50 && b <= 50`), record standing invariants (`data Factors where left <= 50 && right <= 50`), call postconditions (`ensures result <= 50`), and constant bounds (`const CEILING: u64 = 50`) introduce bounded operands. Concrete caller actuals (`30, 40` and upper boundary `50, 50`) discharge these preconditions directly, introducing and establishing the operand bounds at call sites without requiring any post-hoc caller equality guard (`transition x == 2500`).
  - **Identity and lifetime:** The bounded `u64` values and their static range facts are preserved across storage homes and copies into locals, fields, and parameters without loss or decay. Unrelated mutations leave the operand bounds intact.
  - **Composition:** The operand bounds travel seamlessly across all 9 places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the exact multiplication `a * b`, discharging the static representability obligation `result <= 2500`. In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ only in an omitted operand bound or widened ceiling (`CEILING = 100`).
  - **Soundness boundaries:** Removing an operand bound leaves the exact product unproved against `u64::MAX`, causing proof checking to fail closed with `exact arithmetic in machine ... may overflow \`u64\`` across eight places. In `const_name`, widening the ceiling from 50 to 100 while declaring `ensures result <= 2500` fails closed with `cannot prove ensures contract for machine mul`.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** The range facts and multiplication representability proofs erase cleanly after checking, while exact multiplication and carriers lower into canonical target-neutral Terminal Psi (`PSITERM\0`) integer operations without runtime overflow checking wrappers or widening. All 10 positive entries (root and 9 places) execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) through companion build declarations. Target layout geometry validates `u64` scalars (`size_of == 8`, `align_of == 8`), and `CallPlan`/`StatePlan` lower 64-bit integer operands into ABI register lanes (RCX/RDX on `windows_x86_64`, RDI/RSI on `linux_x86_64`, X0/X1 on `macos_arm64` and `linux_arm64`).
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Target backend lowering selects 64-bit integer multiplication instructions (`imul` on x86-64, `mul` on ARM64) and comparison/branch instructions directly, allocates register homes and disjoint activation stack homes, and inserts standard frame prologues and epilogues honoring the `StatePlan` ceiling.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic stack-frame displacements resolve for locals and arguments, basic blocks lay out with short/near branch relaxation and offset reflow, machine-state footprint evidence validates, and direct PE/ELF/Mach-O executables are emitted. Terminal Psi and native execution agree with exit 0 (`omega run --both`) across hosted targets including `windows_x86_64` and `macos_arm64`.

- **What remains open:** Bounded products involving signed operands with mixed positive/negative ranges (`[-50, 50]`), dynamic interval width bounds parameterized by runtime variables, and multi-factor product chains.
