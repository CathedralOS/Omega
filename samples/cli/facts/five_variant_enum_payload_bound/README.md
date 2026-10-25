# Five-Variant Enum Payload Bound

Fact: matching a 5-variant tagged enum payload Packet { Alpha(u64), Beta(u64), Gamma(u64), Delta(u64), Epsilon(u64) } where every arm satisfies 40u64 <= v <= 80u64 preserves 40u64 <= res <= 80u64 across all nine places so exact subtractions 80u64 - res and res - 40u64 are safe.

Every place performs the same operation: route a 5-variant tagged enum packet `Packet::Epsilon { v: 60u64 }` (or another bounded variant arm) through the place, destructure the payload with transition dispatch over all 5 cases (`Alpha`, `Beta`, `Gamma`, `Delta`, `Epsilon`), establishing that every arm carries `v in [40, 80]`, discharge both headroom subtraction `80u64 - res` and margin subtraction `res - 40u64`, verify `headroom + margin == 40u64`, and exit 0. Only the transport mechanism of the packet and payload extraction changes across places.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | root 5-variant tagged enum matching, headroom subtraction, and margin discharge |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | the 5-variant packet is copied into a local before matching and interval discharge |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the 5-variant packet is stored in a record field before matching and interval discharge |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | an explicit machine parameter receives the 5-variant packet before matching and interval discharge |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | a contracted helper call returns the 5-variant packet before matching and interval discharge |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms yield 5-variant packets and join before matching and interval discharge |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | a named constant supplies the 5-variant packet before matching and interval discharge |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a successor machine receives the 5-variant packet before matching and interval discharge |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | an intermediate forwarding machine receives and passes the 5-variant packet to extraction |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each of four target-scoped machine bodies matches the 5-variant packet and discharges interval |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` and declares
`Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/five_variant_enum_payload_bound/main.omg
omega run --both samples/cli/facts/five_variant_enum_payload_bound/local/main.omg
omega run --both samples/cli/facts/five_variant_enum_payload_bound/field/main.omg
omega run --both samples/cli/facts/five_variant_enum_payload_bound/parameter/main.omg
omega run --both samples/cli/facts/five_variant_enum_payload_bound/call_result/main.omg
omega run --both samples/cli/facts/five_variant_enum_payload_bound/match_join/main.omg
omega run --both samples/cli/facts/five_variant_enum_payload_bound/const_name/main.omg
omega run --both samples/cli/facts/five_variant_enum_payload_bound/other_state/main.omg
omega run --both samples/cli/facts/five_variant_enum_payload_bound/other_machine/main.omg
omega run --both samples/cli/facts/five_variant_enum_payload_bound/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- At runtime, with `p == Packet::Epsilon { v: 60u64 }`, `res == 60u64`, `headroom == 80u64 - 60u64 == 20u64`, `margin == 60u64 - 40u64 == 20u64`, and `headroom + margin == 40u64`. The transition `probe() == 40 { true -> done() _ -> bad() }` exits 0. If 5-variant enum matching or exact interval calculations evaluate incorrectly, execution transitions to `machine bad() { crash Trap; }`, causing a native signal 5 / STATUS_ILLEGAL_INSTRUCTION trap crash (exit 3221225501 / 255).
- Measured discrimination: A 1-token perturbation changing `probe() == 40` to `probe() == 41` was tested under `sample_native_gate.py`, where execution took the `bad()` branch and immediately failed the oracle gate with `exit 3221225501 != 0` (STATUS_ILLEGAL_INSTRUCTION trap). Both execution engines fail closed on incorrect results.
- In each invalid twin, variant `Epsilon`'s bound is widened from `80u64` to `81u64` (`case Epsilon(v: u64) where v >= 40 && v <= 81;`). Variant destructuring establishes `res in [40, 81]`, which fails the required interval `res >= 40 && res <= 80` on `verify_payload`. Proof obligation checking statically detects the violated contract, failing closed with `cannot prove requires contract for call`. None reaches native execution.

### Pinned Corpus Fixtures

The nine positive transport places and nine fact-sensitive reject twins are pinned under:
- `tests/omega/pass/facts/five_variant_enum_payload_bound_through_<place>_exit/`
- `tests/omega/fail/facts/five_variant_enum_payload_bound_through_<place>_rejected/`

where `<place>` is `local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, and `target_body`.

### Acceptance Gate

```sh
python tools/sample_native_gate.py --cli C:\SoftwareDevelopmentKits\Omega\build\omega-latest.exe --filter cli/facts/five_variant_enum_payload_bound --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64` (and `linux_x86_64`).
- 9/9 intended-invalid controls match their authored rejection (`cannot prove requires contract for call`).

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`).
- **Ticket:** `ce7359af86004420a17638c0ee74aa08`.
- **Obligation class:** Precondition contract discharge on `verify_payload` (`requires res >= 40 && res <= 80`) via synthesized compile-time interval bounds from exhaustive 5-variant tagged enum matching, preserving bounds without caller runtime guards across all nine transport places.
- **Requirement established:** Target-neutral compile-time 5-variant tagged enum payload bounds analysis (`Psi 05–07`) and native realization (`Omega 00–09`). Specifically, matching a 5-variant tagged enum payload `Packet` (`Alpha`, `Beta`, `Gamma`, `Delta`, `Epsilon`) where every arm satisfies `40u64 <= v <= 80u64` establishes the synthesized post-extraction interval (`res >= 40u64 && res <= 80u64`), discharging exact 64-bit subtractions `80u64 - res` and `res - 40u64` across all nine places without post-hoc caller equality guards, with exact agreement between Terminal Psi and native execution across hosted targets (`omega run --both` exit 0).

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Compile-time interval analysis synthesizes the range `[40, 80]` from exhaustive variant pattern dispatch over all five cases (`Alpha`, `Beta`, `Gamma`, `Delta`, `Epsilon`), each carrying `where v >= 40 && v <= 80`. The synthesized bounds prove `res >= 40u64` and `res <= 80u64` unconditionally and guarantee the value fits within `u64` (`>= 0 && <= 18446744073709551615`), justifying forming exact subtractions `80u64 - res` and `res - 40u64` directly without requiring post-hoc caller equality guards.
  - **Identity and lifetime:** The 5-variant tagged enum values, discriminant tags, and payload intervals retain their precision, layout geometry, and non-negative invariants across copies into local variables, record fields (`PacketHolder`), explicit machine parameters, helper call returns, match join arms, named constants, state transitions, forwarding machines, and target-scoped bodies.
  - **Composition:** The 5-variant tagged enum payload dispatch composes through all 9 fact places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming exact subtraction operations, evaluating to `40u64` across all 9 places. In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the variant payload bound (`v <= 80u64` vs `v <= 81u64`).
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed during compile-time proof obligation checking when `Epsilon` has `v <= 81u64`: variant destructuring establishes `res in [40, 81]`, so the preconditions `res >= 40 && res <= 80` on `verify_payload` cannot be proved. Compilation strictly rejects with `cannot prove requires contract for call`. No unproved underflow or overflow reaches native execution.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Target-neutral 5-variant tagged enum destructuring and exact subtraction contracts lower into canonical target-neutral Terminal Psi (`PSITERM\0`), materializing explicit variant discriminant tests, payload projection, and integer arithmetic certificates. All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) through companion build declarations without reopening source. The tagged enum layout (tag byte + aligned payload, size 16, align 8) validates against target layout plans. Target `CallPlan`/`StatePlan` assign arguments and return values to register lanes or stack slots per platform ABI.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA discriminant dispatch sequences (jump tables or conditional branch trees), register allocators assign homes and activation spill slots, and entry prologues and epilogues honor the `StatePlan` ceiling.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Target ISA instructions and transition displacements encode into machine bytes, basic blocks relax with offset reflow, machine-state footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates against target host limits, and direct PE/ELF/Mach-O executables are emitted. Terminal Psi interpretation and native execution agree with exit 0 (`omega run --both`) across host platforms including `windows_x86_64` and `macos_arm64`.

- **What remains open:** 64-bit multi-variant enums with non-uniform variant payload counts and recursive enum types.
