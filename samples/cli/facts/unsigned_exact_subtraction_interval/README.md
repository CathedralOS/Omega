# Unsigned Exact Subtraction Interval

Subtracting two bounded unsigned `u64` values (`minuend >= 80u64 && minuend <= 100u64` and `subtrahend >= 20u64 && subtrahend <= 30u64`) establishes the exact post-subtraction interval (`diff >= 50u64 && diff <= 80u64`), discharging both `diff - 50u64` and `80u64 - diff` across all nine places.

Every place performs the same operation: define bounded unsigned parameters `minuend` (`80..100`) and `subtrahend` (`20..30`), evaluate exact subtraction `diff: u64 = minuend - subtrahend`, transport the difference through the place, discharge both `diff - 50u64` and `80u64 - diff` without overflow or underflow, compute `sum = (diff - 50) + (80 - diff) == 30`, and verify in `main` that `res == 30` exiting 0. Only the transport mechanism of the difference changes across places.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | root unsigned exact subtraction interval evaluation |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | both bounded unsigned u64 operands and their difference are transported through local bindings |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the difference of the unsigned subtraction is stored and loaded through a record field |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | the difference of the unsigned subtraction is forwarded as a machine parameter |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | the difference arrives as the return result of a helper machine call |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms yield the difference so the joined binding preserves the post-subtraction interval |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | a named constant subtrahend subtracts from the bounded minuend |
| another state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a state transition forwards the difference to a successor state |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | a forwarding machine transports the difference through an intermediate machine call |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each hosted target body computes the unsigned exact subtraction and discharges both differences |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` and declares
`Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/unsigned_exact_subtraction_interval/main.omg
omega run --both samples/cli/facts/unsigned_exact_subtraction_interval/local/main.omg
omega run --both samples/cli/facts/unsigned_exact_subtraction_interval/field/main.omg
omega run --both samples/cli/facts/unsigned_exact_subtraction_interval/parameter/main.omg
omega run --both samples/cli/facts/unsigned_exact_subtraction_interval/call_result/main.omg
omega run --both samples/cli/facts/unsigned_exact_subtraction_interval/match_join/main.omg
omega run --both samples/cli/facts/unsigned_exact_subtraction_interval/const_name/main.omg
omega run --both samples/cli/facts/unsigned_exact_subtraction_interval/other_state/main.omg
omega run --both samples/cli/facts/unsigned_exact_subtraction_interval/other_machine/main.omg
omega run --both samples/cli/facts/unsigned_exact_subtraction_interval/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- In every positive place, evaluating `minuend - subtrahend` with inputs `90u64` and `25u64` establishes difference `65u64` within static interval `50u64..80u64`. Because `diff >= 50u64`, the exact subtraction `diff - 50u64` is provably safe and yields `15u64`. Because `diff <= 80u64`, the exact subtraction `80u64 - diff` is provably safe and yields `15u64`. Their sum `15 + 15 == 30` is checked in `main`: `transition res == 30` transitions to `state done() {}`, terminating with native exit 0. If subtraction or contract satisfaction returns an unexpected value, execution branches to `state bad() { crash Trap; }`, causing exit 255.
- Each invalid twin weakens the minuend bound (e.g. `minuend >= 79u64` or `minuend >= 74u64`), so `minuend - subtrahend` can be `49u64`. Evaluating `transported - 50u64` fails closed at compile time with `error: exact arithmetic in machine ... may overflow \`u64\`: the operands are not provably in range`. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/unsigned_exact_subtraction_interval/<place>/invalid/main.omg
```

The same nine twins are also mirrored under `invalid/<place>/` with an authored
`expected.txt`, so the invalid controls are discoverable by both the per-place
route and the batch route. Each mirror is byte-identical to its per-place twin.

### Acceptance Gate

```sh
python tools/sample_native_gate.py --cli C:\SoftwareDevelopmentKits\Omega\build\omega-latest.exe --filter cli/facts/unsigned_exact_subtraction_interval --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64` (and `linux_x86_64`).
- 18/18 intended-invalid controls match their authored rejection (`may overflow \`u64\``).

## Pinned Fixtures

The row is pinned by 18 fixtures:
`tests/omega/pass/facts/unsigned_exact_subtraction_interval_through_<place>_exit` and
`tests/omega/fail/facts/unsigned_exact_subtraction_interval_through_<place>_rejected` for each of the nine
places. Each positive fixture is its place program copied byte-identical, binds `ProgramEntry` for all four
hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`), and carries
`expected_native_exit.txt` = 0; each reject twin is the place's invalid program and `build.omg` copied
byte-identical, with `expected.txt` carrying the row's own `may overflow \`u64\`` fragment. The `git diff`
of a positive place against its twin moves only the construct's own bound and the transport contract that
carries it: `minuend >= 80` vs `minuend >= 79` in seven places, the bounded constant `minuend >= 74` in
`const_name`, and the helper's `ensures result >= 50`/`>= 49` interval in `call_result`.

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
`python tools/sample_gate.py --filter cli/facts/unsigned_exact_subtraction_interval --cli <generation
omega>` reports 28/28 projects checked on Darwin arm64, 18 of them matched intended rejection controls
(the nine per-place twins and their nine `invalid/<place>/` mirrors).

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`).
- **Requirement established:** End-to-end nine-place fact preservation and native realization for unsigned `u64` exact subtraction interval synthesis and consuming bound discharge. Specifically, subtracting two bounded unsigned integers (`80u64 <= minuend <= 100u64` and `20u64 <= subtrahend <= 30u64`) statically establishes the difference interval `50u64 <= diff <= 80u64`, discharging both exact unsigned subtractions `diff - 50u64` (lower bound safety) and `80u64 - diff` (upper bound safety) across all nine places without post-hoc caller equality guards, with exact agreement between Terminal Psi and native execution across hosted targets (`omega run --both` exit 0).

- **Pinned fixtures:** the 18 fixtures named above; nine `checked` and nine `rejected` rows in `tests/omega/corpus_outcomes.txt`, nine `built ... exit:0` rows in `tests/omega/corpus_native_macos_arm64.txt`, and nine `built ... exit:0 interpreter-exit:0 interpreter:agrees` rows in `tests/omega/corpus_both_macos_arm64.txt`, all stamped `@e42d5379cc30+wt`.

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Bounded integer preconditions (`requires minuend >= 80 && minuend <= 100 && subtrahend >= 20 && subtrahend <= 30`) introduce exact interval facts for unsigned operands. The subtraction expression `minuend - subtrahend` synthesizes the exact difference interval `[80 - 30, 100 - 20] = [50, 80]` directly upon evaluation, discharging both downstream exact subtractions `diff - 50` and `80 - diff` without requiring any post-hoc caller equality guard (`transition x == 65`).
  - **Identity and lifetime:** The synthesized difference value and its static interval bounds are preserved across local variables, struct fields, machine parameters, return results, and control flow joins throughout their lifespans.
  - **Composition:** The unsigned difference interval composes through all 9 fact places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming exact subtractions (`diff - 50` and `80 - diff`). In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the minuend bound (`minuend >= 80` vs `minuend >= 79`), leaving the downstream exact subtraction unproved without any caller guard difference.
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed at compile time when `minuend >= 79` is specified: the difference interval expands to `[49, 80]`, which fails the required domain `diff >= 50` for `diff - 50`, strictly rejecting with `exact arithmetic in machine ... may overflow \`u64\`: the operands are not provably in range`. No unproved interval boundary breach reaches native execution.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Exact unsigned subtraction operations lower into canonical target-neutral Terminal Psi (`PSITERM\0`), erasing mathematical proof obligations while preserving the concrete 64-bit unsigned integer subtraction instructions. All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Companion build bindings (`ProgramEntry`) settle across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) without reopening source. The 64-bit unsigned integer layout plan (`IntegerAt`: `u64` size 8, alignment 8) validates, and target `CallPlan`/`StatePlan` assign 64-bit integer arguments and return values across target registers and stack slots per ABI.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA 64-bit subtraction instructions (`sub rax, rdx` on `x86_64`; `sub x0, x0, x1` on `arm64`), assigns register homes and disjoint activation frame storage, and inserts entry prologues and multi-return epilogues honoring the `StatePlan`.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Target ISA subtraction instructions encode into machine bytes (REX.W-prefixed `sub` on `x86_64`), basic blocks and branch offsets relax with offset reflow, machine-state footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates against target host limits, and direct PE/ELF/Mach-O executables are emitted. Terminal Psi interpretation and native execution agree with exit 0 (`omega run --both`) across host platforms including `windows_x86_64` and `macos_arm64`.

- **What remains open:** Non-linear unsigned division interval synthesis under variable divisors, modular arithmetic domain conversions, and SIMD vector exact subtraction interval propagation.
