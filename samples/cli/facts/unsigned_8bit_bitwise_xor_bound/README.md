# Unsigned 8-Bit Bitwise XOR Bound

Fact: 8-bit unsigned bitwise XOR `raw ^ 15u8` with bounded interval `40u8 <= xored <= 63u8` establishes `40u8 <= xored <= 63u8` across all nine places so exact subtractions `63u8 - xored` and `xored - 40u8` are safe.

Every place performs the same operation: define bounded unsigned 8-bit input `raw` (`59..59`), evaluate exact 8-bit bitwise XOR `xored: u8 = raw ^ 15`, transport the xored value through the place, pass `xored` to `verify_xored(res: u8)` which requires `res >= 40 && res <= 63`, execute exact subtractions `63 - res` and `res - 40`, sum their values and subtract 23 returning `0i32`, and verify in `main` that `res == 0` exiting 0. Only the transport mechanism of the xored value changes across places.

- **Claim ticket:** `f1e5bb93ffbb4485a456b686089d9e22` (row closure, `factsr1-pins-fd`); the row content was authored at `a84ad785c92`.
- **Obligation class:** Contract precondition (`requires res >= 40 && res <= 63`) and proved exact 8-bit subtraction discharge (`63 - res`, `res - 40`).
- **Specification references:** [numeric values](wiki/spec/language/numeric_values.md), [integer certificates](wiki/spec/terminal-psi/integer_certificates.md), [storage](wiki/spec/resources/storage.md).

## Places

| Place | Positive Project | Reject Twin | Pinned Corpus Pass | Pinned Corpus Fail | Transport Mechanism |
| --- | --- | --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | — | — | root 8-bit unsigned bitwise XOR evaluation, subtraction discharge and oracle |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | `tests/omega/pass/facts/unsigned_8bit_bitwise_xor_bound_through_local_exit` | `tests/omega/fail/facts/unsigned_8bit_bitwise_xor_bound_through_local_rejected` | both the bounded unsigned u8 operand and its bitwise XOR result are transported through local bindings |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | `tests/omega/pass/facts/unsigned_8bit_bitwise_xor_bound_through_field_exit` | `tests/omega/fail/facts/unsigned_8bit_bitwise_xor_bound_through_field_rejected` | the bitwise XOR result is stored in and projected from `Carrier.xored` |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | `tests/omega/pass/facts/unsigned_8bit_bitwise_xor_bound_through_parameter_exit` | `tests/omega/fail/facts/unsigned_8bit_bitwise_xor_bound_through_parameter_rejected` | the bitwise XOR result is forwarded as a machine parameter |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | `tests/omega/pass/facts/unsigned_8bit_bitwise_xor_bound_through_call_result_exit` | `tests/omega/fail/facts/unsigned_8bit_bitwise_xor_bound_through_call_result_rejected` | the bitwise XOR result is returned from `xor_helper` with its own `requires`/`ensures` |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | `tests/omega/pass/facts/unsigned_8bit_bitwise_xor_bound_through_match_join_exit` | `tests/omega/fail/facts/unsigned_8bit_bitwise_xor_bound_through_match_join_rejected` | both match arms yield the bitwise XOR result so the joined binding preserves the bounds |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | `tests/omega/pass/facts/unsigned_8bit_bitwise_xor_bound_through_const_name_exit` | `tests/omega/fail/facts/unsigned_8bit_bitwise_xor_bound_through_const_name_rejected` | a named constant operand (`const RAW: u8`) is bitwise XORed with `15u8` |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | `tests/omega/pass/facts/unsigned_8bit_bitwise_xor_bound_through_other_state_exit` | `tests/omega/fail/facts/unsigned_8bit_bitwise_xor_bound_through_other_state_rejected` | a state transition forwards the bitwise XOR result to successor machine `step(s: u8)` |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | `tests/omega/pass/facts/unsigned_8bit_bitwise_xor_bound_through_other_machine_exit` | `tests/omega/fail/facts/unsigned_8bit_bitwise_xor_bound_through_other_machine_rejected` | a forwarding machine transports the bitwise XOR result through an intermediate machine call |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | `tests/omega/pass/facts/unsigned_8bit_bitwise_xor_bound_through_target_body_exit` | `tests/omega/fail/facts/unsigned_8bit_bitwise_xor_bound_through_target_body_rejected` | each hosted target body computes the 8-bit unsigned bitwise XOR and verifies the bounds |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` and declares
`Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/unsigned_8bit_bitwise_xor_bound/main.omg
omega run --both samples/cli/facts/unsigned_8bit_bitwise_xor_bound/local/main.omg
omega run --both samples/cli/facts/unsigned_8bit_bitwise_xor_bound/field/main.omg
omega run --both samples/cli/facts/unsigned_8bit_bitwise_xor_bound/parameter/main.omg
omega run --both samples/cli/facts/unsigned_8bit_bitwise_xor_bound/call_result/main.omg
omega run --both samples/cli/facts/unsigned_8bit_bitwise_xor_bound/match_join/main.omg
omega run --both samples/cli/facts/unsigned_8bit_bitwise_xor_bound/const_name/main.omg
omega run --both samples/cli/facts/unsigned_8bit_bitwise_xor_bound/other_state/main.omg
omega run --both samples/cli/facts/unsigned_8bit_bitwise_xor_bound/other_machine/main.omg
omega run --both samples/cli/facts/unsigned_8bit_bitwise_xor_bound/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- In every positive place, evaluating `raw ^ 15` with input `59u8` establishes xored value `52u8` within static interval `40u8..63u8`. Passing `xored` to `verify_xored(res)` satisfies its contract `res >= 40 && res <= 63`. `verify_xored` evaluates exact subtractions `63 - 52 == 11` and `52 - 40 == 12`, sums their values (23), and subtracts 23 returning `0i32`. In `main`, `transition res == 0` transitions to `machine done() {}`, terminating with native exit 0. Perturbing the root comparison (`transition res == 0` -> `transition res == 1`) exits 255 with `native signal 5` and `interp crash: Trap (host presentation agrees)`, so the authored 0 discriminates.
- Each invalid twin sets `raw >= 64 && raw <= 64` (or `const RAW: u8 = 64`), so `raw ^ 15` yields `79u8`. Calling `verify_xored(xored)` fails closed at compile time with `cannot prove requires contract for call verify_xored`. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/unsigned_8bit_bitwise_xor_bound/<place>/invalid/main.omg
```

The same nine twins are also mirrored under `invalid/<place>/` with an authored
`expected.txt`, so the invalid controls are discoverable by both the per-place
route and the batch route. Each mirror is byte-identical to its per-place twin.

### Acceptance Gate

```sh
python tools/sample_native_gate.py --cli /Volumes/Crucial/CathedralOS/Omega/build/swarm/factsr1/producers/gen-5648483032b/omega --filter cli/facts/unsigned_8bit_bitwise_xor_bound --jobs 4
```

`sample_native_gate.py` reports on the recording host (macOS arm64): 10/10
samples meet their documented oracle and 18/18 intended-invalid controls
(the nine per-place twins and their nine byte-identical `invalid/<place>` mirrors)
match their authored rejection `cannot prove requires contract for call verify_xored`.

## Pinned Fixtures

The row is pinned under `tests/omega/pass/facts/unsigned_8bit_bitwise_xor_bound_through_<place>_exit` and
`tests/omega/fail/facts/unsigned_8bit_bitwise_xor_bound_through_<place>_rejected` across all nine
places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`,
`target_body`). Every positive fixture binds `ProgramEntry` for all four hosted targets (`windows_x86_64`,
`linux_x86_64`, `macos_arm64`, `linux_arm64`) and declares `Expected exit: 0`; every reject fixture carries its
authored `cannot prove requires contract for call verify_xored` fragment.

Measured on macOS arm64 with the shared generation producer for this base (`gen-5648483032b`, revision
`5648483032b`, omega sha256 `77ec27e01b1762895702486b0875eb949b4a4a9af8d68b91e7cc4ca68ffeac98`):
- 10/10 programs (root plus nine places) report `native exit 0` and `interp exit: 0` under `omega run --both`.
- 9/9 `<place>/invalid` twins reject under `omega --check` with `cannot prove requires contract for call verify_xored`.
- The fixtures record 18 checked / 18 rejected in `tests/omega/corpus_outcomes.txt`, 18 built `exit:0` in
  `tests/omega/corpus_native_macos_arm64.txt`, and 18 `exit:0` + `interpreter-exit:0` + `interpreter:agrees` in
  `tests/omega/corpus_both_macos_arm64.txt`; no compiler build was consumed.

## Requirements and Evidence

- **Parent outcome:** `ARITHMETIC-POLICY-REALIZATION` (`TASKS.md`): "Close remaining numeric-policy realization and full program acceptance" — this row is one customer group's nine-place transport and execution of the settled numeric-policy bitwise bound; `CANONICALIZE-SCALAR-RANGE-CONTRACTS` owns the proposition-based range-contract spelling (`requires`, `ensures`, guards, locals established from initializers and calls) the carriers use.
- **Requirement established:** End-to-end nine-place fact preservation and native realization for 8-bit unsigned (`u8`) bitwise XOR bounds (`[numeric values](wiki/spec/language/numeric_values.md)`). Specifically, exact 8-bit unsigned bitwise XOR of bounded inputs (`raw >= 59u8 && raw <= 59u8`) with `15u8` statically establishes `xored >= 40u8 && xored <= 63u8`, discharging downstream contract requirement `res >= 40 && res <= 63` and exact subtractions `63u8 - res` and `res - 40u8` across all nine places without post-hoc caller equality guards, with exact agreement between Terminal Psi and native execution across hosted targets (`omega run --both` exit 0).

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Bounded integer preconditions (`requires raw >= 59 && raw <= 59`) and literal bitwise operand `15u8` introduce static interval facts for 8-bit unsigned operands. The bitwise XOR expression `raw ^ 15` synthesizes the exact xored value and interval `[59 ^ 15, 59 ^ 15] = [52, 52]` directly upon evaluation, falling completely within the declared contract domain `[40, 63]`. This discharges downstream precondition `res >= 40 && res <= 63` and underflow-free exact subtractions `63 - res` and `res - 40` without requiring any post-hoc caller equality guard (`transition x == 52`).
  - **Identity and lifetime:** The synthesized 8-bit xored value and its static interval bounds are preserved across local variables, struct fields, machine parameters, return results, and control flow joins throughout their lifespans.
  - **Composition:** The 8-bit unsigned xored interval composes through all 9 fact places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming downstream call (`verify_xored(xored)`). In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the input bound (`raw >= 59` vs `raw >= 64`), leaving the downstream precondition unproved without any caller guard difference.
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed at compile time when `raw >= 64` is specified: the xored value becomes `79`, which violates the required domain `res >= 40 && res <= 63`, strictly rejecting with `cannot prove requires contract for call verify_xored`. No unproved interval boundary breach reaches native execution.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Exact 8-bit unsigned bitwise XOR operations lower into canonical target-neutral Terminal Psi (`PSITERM\0`), erasing mathematical proof obligations while preserving concrete 8-bit unsigned bitwise operation semantics. All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Companion build bindings (`ProgramEntry`) settle across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) without reopening source. The 8-bit unsigned integer layout plan (`IntegerAt`: `u8` size 1, alignment 1) validates, and target `CallPlan`/`StatePlan` assign 8-bit integer arguments and return values across target registers and stack slots per ABI.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA 8-bit bitwise XOR instructions (`xor r8, imm8` on `x86_64`; `eor w, w, #imm` on `arm64`), assigns register homes and disjoint activation frame storage, and inserts entry prologues and multi-return epilogues honoring the `StatePlan`.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Target ISA instructions encode into machine bytes (opcode `xor` on `x86_64`), basic blocks and branch offsets relax with offset reflow, machine-state footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates against target host limits, and direct PE/ELF/Mach-O executables are emitted. Terminal Psi interpretation and native execution agree with exit 0 (`omega run --both`) across host platforms including `windows_x86_64` and `macos_arm64`.

- **What remains open:** Dynamic non-constant bitwise operand interval propagation across non-power-of-two bitmasks, and SIMD vector bitwise XOR operations (`pxor`).