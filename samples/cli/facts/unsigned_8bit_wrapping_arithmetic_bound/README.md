# Unsigned 8-bit wrapping arithmetic bound

Fact: 8-bit unsigned wrapping addition lhs +% rhs with bounded interval 50u8 <= sum <= 100u8 establishes 50u8 <= sum <= 100u8 across all nine places so exact subtractions 100u8 - sum and sum - 50u8 are safe.

The original root reproduction is preserved in [main.omg](main.omg) and [build.omg](build.omg).

## Places

The nine places carry the 8-bit unsigned wrapping arithmetic bound through each language construct,
accompanied by `<place>/invalid/` reject twins:

| Place | Positive Program | Invalid Reject Twin | Carrier / Transport |
| --- | --- | --- | --- |
| root | [main.omg](main.omg) | — | 8-bit unsigned wrapping addition establishes bound and discharges subtractions directly |
| local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) | wrapping addition result bound to local variable |
| field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) | wrapping addition result stored in and projected from record field |
| parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) | wrapping addition result passed as explicit machine parameter |
| call result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) | wrapping addition result returned across machine call boundary |
| match join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) | wrapping addition result converged across match join arms |
| const name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) | wrapping inputs provided by named compile-time constants |
| another state | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) | wrapping addition result forwarded across state machine transition |
| another machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) | wrapping addition result forwarded through intermediate machine |
| another target body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) | wrapping addition result verified across target-scoped machine bodies |

Run each place with `omega run <place>/main.omg`.

## Execution

Every positive place (and the root `main.omg`) binds `ProgramEntry` for all four hosted targets
(`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) in its `build.omg` and declares
`Expected exit: 0`. `omega run --both` verifies Terminal Psi interpretation against native execution:

```sh
omega run --both samples/cli/facts/unsigned_8bit_wrapping_arithmetic_bound/main.omg
omega run --both samples/cli/facts/unsigned_8bit_wrapping_arithmetic_bound/local/main.omg
omega run --both samples/cli/facts/unsigned_8bit_wrapping_arithmetic_bound/field/main.omg
omega run --both samples/cli/facts/unsigned_8bit_wrapping_arithmetic_bound/parameter/main.omg
omega run --both samples/cli/facts/unsigned_8bit_wrapping_arithmetic_bound/call_result/main.omg
omega run --both samples/cli/facts/unsigned_8bit_wrapping_arithmetic_bound/match_join/main.omg
omega run --both samples/cli/facts/unsigned_8bit_wrapping_arithmetic_bound/const_name/main.omg
omega run --both samples/cli/facts/unsigned_8bit_wrapping_arithmetic_bound/other_state/main.omg
omega run --both samples/cli/facts/unsigned_8bit_wrapping_arithmetic_bound/other_machine/main.omg
omega run --both samples/cli/facts/unsigned_8bit_wrapping_arithmetic_bound/target_body/main.omg
```

All ten positives (the root and the nine places) report `native exit 0` and `interp exit: 0`.

### The oracle discriminates

Each positive place wraps `60u8 + 15u8` in the `Wrapping` domain, statically establishing
`sum >= 50u8 && sum <= 100u8`; the consuming exact subtractions `100u8 - sum` and `sum - 50u8` are
proved underflow-free and evaluate to `25u8` each, so `transition check == 0` reaches `done()` and the
program exits 0. Perturbing that single token in the root witness (`transition check == 0` to
`transition check == 1`) takes `state bad() { crash Trap; }` instead and exits 255 reporting
`native signal 5` with `interp crash: Trap (host presentation agrees)`, so the authored 0
discriminates between correct evaluation and faulty execution.

## Pinned Corpus Fixtures

The row is pinned for all nine places under `tests/omega/pass/facts/` and `tests/omega/fail/facts/`:

| Place | Pinned pass fixture | Pinned reject fixture | Transport |
| --- | --- | --- | --- |
| local | `unsigned_8bit_wrapping_arithmetic_bound_through_local_exit` | `unsigned_8bit_wrapping_arithmetic_bound_through_local_rejected` | wrapping addition result bound to local variable |
| field | `unsigned_8bit_wrapping_arithmetic_bound_through_field_exit` | `unsigned_8bit_wrapping_arithmetic_bound_through_field_rejected` | wrapping addition result stored in and projected from record field |
| parameter | `unsigned_8bit_wrapping_arithmetic_bound_through_parameter_exit` | `unsigned_8bit_wrapping_arithmetic_bound_through_parameter_rejected` | wrapping addition result passed as explicit machine parameter |
| call_result | `unsigned_8bit_wrapping_arithmetic_bound_through_call_result_exit` | `unsigned_8bit_wrapping_arithmetic_bound_through_call_result_rejected` | wrapping addition result returned across machine call boundary |
| match_join | `unsigned_8bit_wrapping_arithmetic_bound_through_match_join_exit` | `unsigned_8bit_wrapping_arithmetic_bound_through_match_join_rejected` | wrapping addition result converged across match join arms |
| const_name | `unsigned_8bit_wrapping_arithmetic_bound_through_const_name_exit` | `unsigned_8bit_wrapping_arithmetic_bound_through_const_name_rejected` | wrapping inputs provided by named compile-time constants |
| other_state | `unsigned_8bit_wrapping_arithmetic_bound_through_other_state_exit` | `unsigned_8bit_wrapping_arithmetic_bound_through_other_state_rejected` | wrapping addition result forwarded across state machine transition |
| other_machine | `unsigned_8bit_wrapping_arithmetic_bound_through_other_machine_exit` | `unsigned_8bit_wrapping_arithmetic_bound_through_other_machine_rejected` | wrapping addition result forwarded through intermediate machine |
| target_body | `unsigned_8bit_wrapping_arithmetic_bound_through_target_body_exit` | `unsigned_8bit_wrapping_arithmetic_bound_through_target_body_rejected` | wrapping addition result verified across target-scoped machine bodies |

Each pass fixture is a byte-identical copy of its place's `main.omg` and `build.omg` plus
`expected_native_exit.txt` = `0`; each reject fixture is a byte-identical copy of its place twin's
`main.omg` and `build.omg` plus `expected.txt` carrying the fragment the corpus runner emits. Positive
versus twin `git diff` moves only the construct's own operands: `lhs = 60u8, rhs = 15u8` becomes
`lhs = 85u8, rhs = 20u8` (or the named constants `LHS`/`RHS`), so `sum == 105u8 > 100u8` and the
consuming `100u8 - sum` has no proof. The place directories also now declare the per-place
application names (`unsigned_8bit_wrapping_arithmetic_bound_<place>` and its `_invalid` twin) their
sibling rows carry, so each fixture's refusal names its own place.

### Invalid controls and reject twins

Each twin widens the inputs so the wrapping sum can reach `105u8`; checking then fails closed
(`omega --check` exits 1 on all nine twins) with that place's own diagnostic:

| Place | Reject twin diagnostic |
| --- | --- |
| local | `cannot prove requires contract for call verify_sum from main: local_sum >= 50 && local_sum <= 100` |
| field | `cannot prove requires contract for call verify_sum from main: carrier.sum >= 50 && carrier.sum <= 100` |
| parameter | `cannot prove requires contract for call check_param from main: sum >= 50 && sum <= 100` |
| call_result | `cannot prove ensures contract for exit from produce_sum at statement 1: result >= 50 && result <= 100` |
| match_join | `cannot prove requires contract for call verify_sum from main: joined >= 50 && joined <= 100` |
| const_name | `cannot prove requires contract for call verify_sum from main: sum >= 50 && sum <= 100` |
| other_state | `cannot prove requires contract for call verify_sum from main: forwarded >= 50 && forwarded <= 100` |
| other_machine | `cannot prove requires contract for call runner from main: sum >= 50 && sum <= 100` |
| target_body | `cannot prove requires contract for call exercise from main: sum >= 50 && sum <= 100` (four target bodies) |

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
- **Requirement established:** 8-bit unsigned integer wrapping addition arithmetic bounds proof and realization (`Psi 05–07` / `Omega 00–09`), verifying that wrapping addition `lhs + rhs` (denoted `lhs +% rhs`) on bounded `u8 in Wrapping` operands produces a bounded sum `50u8 <= sum <= 100u8`, which proves that exact 8-bit unsigned subtractions `100u8 - sum` and `sum - 50u8` are provably underflow-free across all nine standard places.

Full 8-layer specification and lowering evidence:

- **Establishment:** The 8-bit unsigned wrapping addition `lhs + rhs` on `lhs: u8 in Wrapping` and `rhs: u8 in Wrapping` allows modular arithmetic without trap obligations or overflow assertions. With bounded operands `lhs = 60` and `rhs = 15`, the evaluated sum `75u8` resides strictly in `[50, 100]`. The consuming helper `verify_sum(sum: u8) -> i32 requires sum >= 50 && sum <= 100` discharges exact unsigned subtractions `100 - sum` (evaluating to 25) and `sum - 50` (evaluating to 25), verifying `headroom + margin == 50` without post-hoc caller guards.
- **Identity and lifetime:** The evaluated 8-bit unsigned scalar preserves its invariant `[50, 100]` across all standard value carriers: local bindings, record fields, machine parameters, call return results, match convergence joins, compile-time named constants, state machine transition arguments, intermediate machine forwarding, and target-scoped execution bodies.
- **Composition:** The wrapping addition bound composes across all 9 standard places to satisfy the consuming requirement `sum >= 50 && sum <= 100` and discharge `100 - sum` and `sum - 50`:
  - `local`: wrapping addition result is bound to a local variable `local_sum` before verification.
  - `field`: wrapping addition result is stored in `SumCarrier.sum` record field and projected out.
  - `parameter`: wrapping addition result is passed to machine parameter `check_param(val: u8)`.
  - `call_result`: helper machine `produce_sum()` computes wrapping addition and returns the result across call boundary with ensures contract.
  - `match_join`: wrapping addition result converges through match arms into a unified value.
  - `const_name`: inputs `LHS` and `RHS` are declared as compile-time named constants `const LHS: u8 in Wrapping = 60; const RHS: u8 in Wrapping = 15;`.
  - `other_state`: wrapping addition result is passed across a transition edge into successor state `step(forwarded: u8)`.
  - `other_machine`: wrapping addition result is forwarded through intermediate machine `runner()`.
  - `target_body`: wrapping addition result is evaluated and verified across target-scoped machine bodies for all four hosted targets (`linux_x86_64`, `linux_arm64`, `windows_x86_64`, `macos_arm64`).
  In each place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ only in widening inputs to `lhs = 85, rhs = 20` (where `sum` reaches `105u8 > 100u8`), proving that downstream contract verification `requires sum >= 50 && sum <= 100` strictly fails closed when the post-wrapping sum exceeds 100.
- **Soundness boundaries:** All nine reject twins under `<place>/invalid/` reject during compile-time verification (`omega --check`) with `error: cannot prove ... contract ...: ... <= 100`, confirming that the proof engine soundly detects that when `sum` can be 105, contract satisfaction cannot be established.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Source constructs lower into canonical `PSITERM\0` target-neutral representations with explicit `WrappingIntegerAdd` operations without overflow trapping assertions. Both Terminal Psi interpretation and native execution agree (`omega run --both` reports exit 0).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Target realization joins Build companion rows (`ProgramEntry`) across the four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Target layout plans validate 1-byte `u8` scalar storage and alignment, while calling plans assign 8-bit register homes (`al`/`dil`/`sil` on x86_64, `w0`/`w1` on arm64).
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA wrapping addition instructions: 8-bit `add r8, r8` on `x86_64` and `add w, w, w` on `arm64` without overflow trapping certificates. Register allocation assigns register homes and activation frames honoring the `StatePlan` ceiling.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Direct PE/ELF/Mach-O images are emitted with valid machine-state footprint evidence, executing natively and agreeing with Terminal Psi (`omega run --both` reports `native exit 0` and `interp exit: 0`) across all hosted targets.

The row itself is closed: its nine places and nine reject twins run, reject and are pinned
above. Remaining open under `CANONICALIZE-SCALAR-RANGE-CONTRACTS` / `ARITHMETIC-POLICY-REALIZATION`: dynamic runtime modular arithmetic overflow trapping certificates, 16-bit to 64-bit multi-precision modular residue arithmetic, and SIMD packed modular arithmetic intrinsics.
