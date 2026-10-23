# Unsigned 8-bit logical right shift bound

Fact: an 8-bit unsigned u8 logical right shift of a bounded input (val >= 160u8 && val <= 160u8) by 2u32 bits establishes shifted >= 40u8 && shifted <= 40u8 so exact 8-bit subtraction shifted - 40u8 is proved underflow-free across all nine places.

The original root reproduction is preserved in [main.omg](main.omg) and [build.omg](build.omg).

## Places

The nine places carry the 8-bit unsigned logical right shift bound through each language construct,
accompanied by `<place>/invalid/` reject twins:

| Place | Positive Program | Invalid Reject Twin | Carrier / Transport |
| --- | --- | --- | --- |
| root | [main.omg](main.omg) | — | 8-bit logical right shift establishes bound and discharges subtraction directly |
| local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) | shifted result bound to local variable |
| field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) | shifted result stored in and projected from record field |
| parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) | shifted result passed as explicit machine parameter |
| call result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) | shifted result returned across machine call boundary |
| match join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) | shifted result converged across match join arms |
| const name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) | shifted input provided by named compile-time constant |
| another state | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) | shifted result forwarded across state machine transition |
| another machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) | shifted result forwarded through intermediate machine |
| another target body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) | shifted result verified across target-scoped machine bodies |

Run each place with `omega run <place>/main.omg`.

## Execution

Every positive place (and the root `main.omg`) binds `ProgramEntry` for all four hosted targets
(`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) in its `build.omg` and declares
`Expected exit: 0`. `omega run --both` verifies Terminal Psi interpretation against native execution:

```sh
omega run --both samples/cli/facts/unsigned_8bit_shift_right_bound/main.omg
omega run --both samples/cli/facts/unsigned_8bit_shift_right_bound/local/main.omg
omega run --both samples/cli/facts/unsigned_8bit_shift_right_bound/field/main.omg
omega run --both samples/cli/facts/unsigned_8bit_shift_right_bound/parameter/main.omg
omega run --both samples/cli/facts/unsigned_8bit_shift_right_bound/call_result/main.omg
omega run --both samples/cli/facts/unsigned_8bit_shift_right_bound/match_join/main.omg
omega run --both samples/cli/facts/unsigned_8bit_shift_right_bound/const_name/main.omg
omega run --both samples/cli/facts/unsigned_8bit_shift_right_bound/other_state/main.omg
omega run --both samples/cli/facts/unsigned_8bit_shift_right_bound/other_machine/main.omg
omega run --both samples/cli/facts/unsigned_8bit_shift_right_bound/target_body/main.omg
```

All ten positives (the root and the nine places) report `native exit 0` and `interp exit: 0`.

### The oracle discriminates

Each positive place shifts the singly bounded input `160u8` (`val >= 160u8 && val <= 160u8`) right by
`2u32` bits, statically establishing `shifted >= 40u8 && shifted <= 40u8`; the consuming exact 8-bit
subtraction `shifted - 40u8` is proved underflow-free and evaluates to `0u8`, so `transition check == 0`
reaches `done()` and the program exits 0. Perturbing that single token in the root witness
(`transition check == 0` to `transition check == 1`) takes `state bad() { crash Trap; }` instead and
exits 255 reporting `native signal 5` with `interp crash: Trap (host presentation agrees)`, so the
authored 0 discriminates between correct evaluation and faulty execution.

## Pinned Corpus Fixtures

The row is pinned for all nine places under `tests/omega/pass/facts/` and `tests/omega/fail/facts/`:

| Place | Pinned pass fixture | Pinned reject fixture | Transport |
| --- | --- | --- | --- |
| local | `unsigned_8bit_shift_right_bound_through_local_exit` | `unsigned_8bit_shift_right_bound_through_local_rejected` | shifted result bound to local variable |
| field | `unsigned_8bit_shift_right_bound_through_field_exit` | `unsigned_8bit_shift_right_bound_through_field_rejected` | shifted result stored in and projected from record field |
| parameter | `unsigned_8bit_shift_right_bound_through_parameter_exit` | `unsigned_8bit_shift_right_bound_through_parameter_rejected` | shifted result passed as explicit machine parameter |
| call_result | `unsigned_8bit_shift_right_bound_through_call_result_exit` | `unsigned_8bit_shift_right_bound_through_call_result_rejected` | shifted result returned across machine call boundary |
| match_join | `unsigned_8bit_shift_right_bound_through_match_join_exit` | `unsigned_8bit_shift_right_bound_through_match_join_rejected` | shifted result converged across match join arms |
| const_name | `unsigned_8bit_shift_right_bound_through_const_name_exit` | `unsigned_8bit_shift_right_bound_through_const_name_rejected` | shifted input provided by named compile-time constant |
| other_state | `unsigned_8bit_shift_right_bound_through_other_state_exit` | `unsigned_8bit_shift_right_bound_through_other_state_rejected` | shifted result forwarded across state machine transition |
| other_machine | `unsigned_8bit_shift_right_bound_through_other_machine_exit` | `unsigned_8bit_shift_right_bound_through_other_machine_rejected` | shifted result forwarded through intermediate machine |
| target_body | `unsigned_8bit_shift_right_bound_through_target_body_exit` | `unsigned_8bit_shift_right_bound_through_target_body_rejected` | shifted result verified across target-scoped machine bodies |

Each pass fixture is a byte-identical copy of its place's `main.omg` and `build.omg` plus
`expected_native_exit.txt` = `0`; each reject fixture is a byte-identical copy of its place twin's
`main.omg` and `build.omg` plus `expected.txt` carrying the fragment the corpus runner emits. Positive
versus twin `git diff` moves only the construct's own bound: `val >= 160u8 && val <= 160u8` becomes
`val >= 120u8 && val <= 160u8` and the evaluated `val` moves from `160u8` to `120u8`, so
`shifted == 30u8` and the consuming `shifted - 40u8` has no proof.

### Invalid controls and reject twins

Each twin weakens the input bound so the shifted value can be `30u8`; checking then fails closed
(`omega --check` exits 1 on all nine twins) with that place's own diagnostic:

| Place | Reject twin diagnostic |
| --- | --- |
| local | `cannot prove requires contract for call verify_shifted from main: local_shifted >= 40 && local_shifted <= 40` |
| field | `cannot prove requires contract for call verify_shifted from main: carrier.shifted >= 40 && carrier.shifted <= 40` |
| parameter | `cannot prove requires contract for call check_param from main: shifted >= 40 && shifted <= 40` |
| call_result | `cannot prove ensures contract for exit from produce_shifted at statement 0: result >= 40 && result <= 40` |
| match_join | `cannot prove requires contract for call verify_shifted from main: joined >= 40 && joined <= 40` |
| const_name | `cannot prove requires contract for call verify_shifted from main: shifted >= 40 && shifted <= 40` |
| other_state | `cannot prove requires contract for call verify_shifted from main: forwarded >= 40 && forwarded <= 40` |
| other_machine | `cannot prove requires contract for call runner from main: shifted >= 40 && shifted <= 40` |
| target_body | `cannot prove requires contract for call exercise from main: shifted >= 40 && shifted <= 40` (four target bodies) |

## Measured evidence

Measured on macOS arm64 with the factsr1 shared generation producer
gen-d3b0020bf6a (revision d3b0020bf6a72cac2367f3eae2ff1db55cc332b0, omega sha256
`c6dff3ba4be3a38c720ee83b522d251743afde9c174ca6881390c5812920e344`, corpus_runner sha256
`65225cae2bf62dec8f705339afaef606c0bac0e611c005dae984488e67ffa301`):

- `omega run --both` over the root and the nine places: 10/10 report `native exit 0` and
  `interp exit: 0`.
- `omega --check` over the nine twins: 9/9 exit 1 with their authored fragment (table above).
- Perturbing the root oracle (one token) exits 255 with `native signal 5` and
  `interp crash: Trap (host presentation agrees)`.
- The three corpus legs were recorded with the fixture and golden changes staged at published main
  `60453171ca5f`, so every row is stamped `@60453171ca5f+wt`: `tests/omega/corpus_outcomes.txt` gained
  36 rows (18 `checked` carrying `check-target:macos_arm64` and 18 `rejected` with their diagnostics
  and expected fragments satisfied, 24 diagnostic continuation lines),
  `tests/omega/corpus_native_macos_arm64.txt` gained 18 `built ... exit:0` rows and
  `tests/omega/corpus_both_macos_arm64.txt` gained 18 `built ... exit:0 interpreter-exit:0
  interpreter:agrees` rows. Every golden diff is a pure addition -- +60/-0, +18/-0 and +18/-0 with 0
  `timeout` rows.

## Requirements and Evidence

- **Parent outcome:** `CANONICALIZE-SCALAR-RANGE-CONTRACTS` (`TASKS.md`): the settled
  [proposition-based scalar range contract](../../../../wiki/spec/language/domains.md#declaration-and-membership),
  where machine and state parameters carry `requires`, results carry `ensures`, and locals take their
  facts from initializers, guards and calls. This row is one customer group's nine-place transport and
  execution of that contract; the underlying numeric-policy realization is
  `ARITHMETIC-POLICY-REALIZATION`.
- **Requirement established:** 8-bit unsigned integer logical right shift bounds proof and realization (`Psi 05–07` / `Omega 00–09`), verifying that shifting bounded `u8` value (`val >= 160u8 && val <= 160u8`) right by `2u32` bits establishes the interval `shifted >= 40u8 && shifted <= 40u8`, which proves that exact 8-bit subtraction `shifted - 40u8 == 0u8` is provably underflow-free across all nine standard places.

Full 8-layer specification and lowering evidence:

- **Establishment:** The 8-bit unsigned logical right shift `val >> 2u32` on `val: u8` with entry contract `requires val >= 160 && val <= 160` establishes the postcondition interval `[160 >> 2, 160 >> 2] = [40, 40]`. The compiler statically models the zero-extending logical right shift, proving that `shifted >= 40 && shifted <= 40` holds within 8-bit unsigned range without overflow or sign extension. The consuming helper `verify_shifted(shifted: u8) -> u8 requires shifted >= 40 && shifted <= 40` discharges exact unsigned subtraction `shifted - 40` without underflow.
- **Identity and lifetime:** The shifted 8-bit unsigned scalar preserves its invariant `[40, 40]` across all standard value carriers: local bindings, record fields, machine parameters, call return results, match convergence joins, compile-time named constants, state machine transition arguments, intermediate machine forwarding, and target-scoped execution bodies.
- **Composition:** The exact logical right shift bound composes across all 9 standard places to satisfy the consuming requirement `shifted >= 40 && shifted <= 40` and discharge `shifted - 40`:
  - `local`: shifted result is bound to a local variable `local_shifted` before verification.
  - `field`: shifted result is stored in `ShiftCarrier.shifted` record field and projected out.
  - `parameter`: shifted result is passed to machine parameter `check_param(val: u8)`.
  - `call_result`: helper machine `produce_shifted()` computes the shift and returns the result across call boundary.
  - `match_join`: shifted result converges through match arms into a unified value.
  - `const_name`: input `VAL` is declared as a compile-time named constant `const VAL: u8 = 160;`.
  - `other_state`: shifted result is passed across a transition edge into successor state `step(val: u8)`.
  - `other_machine`: shifted result is forwarded through intermediate machine `runner()`.
  - `target_body`: shifted result is evaluated and verified across target-scoped machine bodies for all four hosted targets (`linux_x86_64`, `linux_arm64`, `windows_x86_64`, `macos_arm64`).
  In each place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ only in weakening the input bound to `val >= 120 && val <= 160` (so `val == 120` yields `shifted == 30`), proving that downstream contract verification `requires shifted >= 40 && shifted <= 40` strictly fails closed when the post-shift bound allows values below 40.
- **Soundness boundaries:** All nine reject twins under `<place>/invalid/` reject during compile-time verification (`omega --check`) with `error: cannot prove ... contract ...: ... >= 40`, confirming that the proof engine soundly detects that when `shifted` can be 30, contract satisfaction cannot be established.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Source constructs lower into canonical `PSITERM\0` target-neutral representations. Shift operations are lowered with verified shift count certificates (`count < 8`), preserving interval arithmetic bounds and erasing proof tags. Both Terminal Psi interpretation and native execution agree (`omega run --both` reports exit 0).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Target realization joins Build companion rows (`ProgramEntry`) across the four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Target layout plans validate 1-byte `u8` scalar storage and alignment, while calling plans assign 8-bit register homes (`al`/`dil`/`sil` on x86_64, `w0`/`w1` on arm64) for passing values across calling conventions.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA logical right shift instructions: 8-bit `shr r8, imm8` on `x86_64` and zero-extended `lsr w, w, #imm` on `arm64`. Exact unsigned subtraction lowers to 8-bit `sub`, and register allocation assigns register homes and disjoint stack activation frames honoring the `StatePlan` ceiling.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Direct PE/ELF/Mach-O images are emitted with valid machine-state footprint evidence, executing natively and agreeing with Terminal Psi (`omega run --both` reports `native exit 0` and `interp exit: 0`) across all hosted targets.

The row itself is closed: its nine places and nine reject twins run, reject and are pinned
above. Remaining open under `CANONICALIZE-SCALAR-RANGE-CONTRACTS` / `ARITHMETIC-POLICY-REALIZATION`: dynamic runtime shift counts requiring masking certificates, 128-bit wide shifts across register pairs, and vector SIMD packed logical right shifts.
