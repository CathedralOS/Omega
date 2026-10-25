# Twenty-seven-variant enum payload bound

Fact: matching a 27-variant tagged enum payload TwentySevenChoice { V1(u64), V2(u64), V3(u64), V4(u64), V5(u64), V6(u64), V7(u64), V8(u64), V9(u64), V10(u64), V11(u64), V12(u64), V13(u64), V14(u64), V15(u64), V16(u64), V17(u64), V18(u64), V19(u64), V20(u64), V21(u64), V22(u64), V23(u64), V24(u64), V25(u64), V26(u64), V27(u64) } where every arm satisfies 40u64 <= v <= 80u64 preserves 40u64 <= res <= 80u64 across all nine places so exact subtractions 80u64 - res and res - 40u64 are safe.

Owning Task: `CANONICALIZE-SCALAR-RANGE-CONTRACTS` (`TASKS.md`), the `payloads case where` clause of the settled proposition-based scalar range contract; the enum-bound row family's local label is `ENUM-PAYLOAD` / `CASE-PAYLOAD-BOUND`.
Governing Specifications:
- `wiki/spec/layouts/plans.md` (Tagged variant layout plans and alignment)
- `wiki/spec/terminal-psi/control_flow.md` (Pattern matching, variant dispatch, and join obligations)

Every positive place and reject twin tests a 27-variant tagged enum (`data TwentySevenChoice [copy]`) with bounded 64-bit unsigned payloads across all variants (`V1(v: u64)` through `V27(v: u64)` where `v >= 40u64 && v <= 80u64`).
Matching or extracting the active variant payload yields an exact result `res: u64` statically guaranteed to lie in `[40, 80]`.
Casting or transporting `res` across places preserves `40u64 <= res <= 80u64`.
Subsequent exact subtractions `headroom = 80u64 - res` and `margin = res - 40u64` inside `verify_payload` are proved underflow-free and non-negative, evaluating to `20u64 + 20u64 = 40u64` for `v == 60u64` and terminating with exit 0.

## Places

| Place | Positive Program | Reject Twin (invalid) | Transport Mechanism |
| --- | --- | --- | --- |
| root | [`main.omg`](main.omg) | — | canonical root witness matching 27-variant enum payload and evaluating dual margin subtractions |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | extracted 64-bit variant payload is copied into a local binding before verification |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | extracted 64-bit variant payload is stored into and read from a record field before verification |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | extracted 64-bit variant payload is passed as an explicit machine parameter before verification |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | extracted 64-bit variant payload is returned from a dedicated helper machine |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both arms of a match expression unify the extracted 64-bit variant payload |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | named constants supply upper and lower ceilings for variant payload bounds |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a state transition carries the extracted 64-bit variant payload into a successor machine |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | an independent forwarding machine transports the extracted 64-bit variant payload |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each hosted target provides a target-scoped machine body performing payload extraction and verification |

## Execution

Every positive place (and root `main.omg`) binds `ProgramEntry` for all four hosted targets
(`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) in its `build.omg` and declares `Expected exit: 0`.
`omega run --both` verifies Terminal Psi interpreter and native execution agreement:

```sh
omega run --both samples/cli/facts/twentyseven_variant_enum_payload_bound/main.omg
omega run --both samples/cli/facts/twentyseven_variant_enum_payload_bound/local/main.omg
omega run --both samples/cli/facts/twentyseven_variant_enum_payload_bound/field/main.omg
omega run --both samples/cli/facts/twentyseven_variant_enum_payload_bound/parameter/main.omg
omega run --both samples/cli/facts/twentyseven_variant_enum_payload_bound/call_result/main.omg
omega run --both samples/cli/facts/twentyseven_variant_enum_payload_bound/match_join/main.omg
omega run --both samples/cli/facts/twentyseven_variant_enum_payload_bound/const_name/main.omg
omega run --both samples/cli/facts/twentyseven_variant_enum_payload_bound/other_state/main.omg
omega run --both samples/cli/facts/twentyseven_variant_enum_payload_bound/other_machine/main.omg
omega run --both samples/cli/facts/twentyseven_variant_enum_payload_bound/target_body/main.omg
```

Every positive place and the root witness observe `native exit 0` and `interp exit: 0`.

### Pinned fixture family

The 18 fixtures under `tests/omega/{pass,fail}/facts/` mirror this sample tree
byte-for-byte: nine positives
(`twentyseven_variant_enum_payload_bound_through_<place>_exit`: `main.omg`,
`build.omg`, `expected_native_exit.txt`) and nine reject twins
(`twentyseven_variant_enum_payload_bound_through_<place>_rejected`: `main.omg`,
`build.omg`, `expected.txt`). Both files of a twin are the sample's own
`<place>/invalid/` files, and each `expected.txt` carries the twin's observed
rejection text.

The three macOS golden legs were recorded at `9b001b8f6afa+wt` with the shared
generation producer built at that same revision (`gen-9b001b8f6af`; omega
sha256 `6d24393125b7`, runner sha256 `cee00bbdce17`):

- `corpus_outcomes.txt`: +18 keys -- nine `checked` rows carrying
  `check-target:macos_arm64` and nine `rejected ... expected` rows carrying
  their own diagnostics in 12 continuation lines;
- `corpus_native_macos_arm64.txt`: nine `built ... exit:0` rows;
- `corpus_both_macos_arm64.txt`: nine `built ... exit:0 interpreter-exit:0
  interpreter:agrees` rows.

No row records a lost success, no leg wrote a timeout row, and no twin was
recorded as rejected without its expected fragment. The same 18 twin
diagnostics are byte-identical under the earlier producer
`gen-4c23cc3391c`, 42 `.rs` files of compiler movement apart.

### The oracle discriminates

In each positive place, evaluating `probe(TwentySevenChoice::V27 { v: 60u64 })` extracts `res = 60u64`.
Because all twenty-seven variant arms enforce `v >= 40u64 && v <= 80u64`, the exhaustive match statically establishes `res in [40, 80]`.
Subsequent exact subtractions in `verify_payload` (`headroom = 80u64 - res = 20u64` and `margin = res - 40u64 = 20u64`) evaluate to `20u64 + 20u64 = 40u64`.
The transition guard `combined == 40u64` verifies exact arithmetic evaluation, transitioning to `done()` with native exit 0.
If payload extraction or verification arithmetic is corrupted, the guard fails and transitions to `bad() { crash Trap; }`, terminating with exit 255.

### Invalid controls and reject twins

Each place has an `invalid/` reject twin that widens one variant's bound (`V27(v: u64) where v >= 39u64 && v <= 80u64`).
When `v == 39u64`, `res` can fall to `39u64 < 40u64`, which violates the contract precondition `value >= 40u64` required by `verify_payload` (or ensures postcondition on helper extraction).
The compiler strictly fails closed at compile time:

| Place | `omega run --both` | Reject twin diagnostic |
| --- | --- | --- |
| `local` | native 0 / interp 0 | `error: cannot prove requires contract for call verify_payload from probe: local_res >= 40 && local_res <= 80` |
| `field` | native 0 / interp 0 | `error: cannot prove requires contract for call verify_payload from probe: holder.res >= 40 && holder.res <= 80` |
| `parameter` | native 0 / interp 0 | `error: cannot prove requires contract for call consume from probe: res >= 40 && res <= 80` |
| `call_result` | native 0 / interp 0 | `error: cannot prove ensures contract for exit from extract_payload: result >= 40 && result <= 80` |
| `match_join` | native 0 / interp 0 | `error: cannot prove requires contract for call choose from probe: res >= 40 && res <= 80 && 60 >= 40 && 60 <= 80` |
| `const_name` | native 0 / interp 0 | `error: cannot prove requires contract for call verify_payload from probe: res >= 40 && res <= 80` |
| `other_state` | native 0 / interp 0 | `error: cannot prove requires contract for call consume from probe: res >= 40 && res <= 80` |
| `other_machine` | native 0 / interp 0 | `error: cannot prove requires contract for call forward_payload from probe: res >= 40 && res <= 80` |
| `target_body` | native 0 / interp 0 | `error: cannot prove requires contract for call verify_payload from target_probe: res >= 40 && res <= 80` |

## Requirements and Evidence (AGENTS.md)

- **Establishment**: Variant declarations `V1(v: u64)` through `V27(v: u64)` where `v >= 40u64 && v <= 80u64` establish bounded intervals on active variant payloads. Exhaustive pattern matching across all twenty-seven tagged enum variants joins the incoming arm obligations, statically proving that the extracted payload satisfies `[40, 80]` without post-hoc caller equality guards.
- **Identity and lifetime**: The extracted payload interval fact `res in [40, 80]` is preserved across local bindings, carrier record fields, machine parameters, helper call returns, match join arms, named constants, state transitions, helper forwarding machines, and target-scoped bodies without semantic loss.
- **Composition**: In each place, the synthesized interval `res in [40, 80]` composes directly with `verify_payload` contract preconditions and subsequent exact subtractions `headroom = 80u64 - res` and `margin = res - 40u64` to statically discharge underflow safety and non-negativity without post-hoc caller equality guards.
- **Soundness boundaries**: In every reject twin, widening the payload bound on variant `V27` to `v >= 39u64` permits `res` to reach `39u64`, which fails closed under contract requires preconditions or ensures postconditions across boundary transmissions, verifying that out-of-interval variant payloads cannot satisfy safety bounds.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`)**: Tagged enum construction, 27-variant pattern matching, variant discrimination, and payload access lower to target-neutral Terminal Psi tagged case representations (`PSITERM\0`), while proof-only where-clauses are erased. Both Terminal Psi interpreter and native execution agree with exit 0 (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`)**: Provider layout plans represent the 27-variant tagged enum `TwentySevenChoice` with a 1-byte discriminant tag (`u8` range 0..26) and an 8-byte payload (`IntegerAt 8-byte` for `u64`). With natural 8-byte alignment, 7 padding bytes separate the 1-byte discriminant tag from the payload at offset 8, yielding a total layout plan size of 16 bytes and alignment of 8 bytes across all four hosted platforms.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`)**: Backend lowering selects target ISA 27-way variant tag comparison/dispatch and payload extraction instructions alongside 64-bit integer moves and exact subtractions: `sub rax, ...` on `x86_64` and `sub x0, ...` on `arm64`, maintaining disjoint register homes and activation stack frames without spilling corruption.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission)**: Direct binary emission encodes machine instructions, resolves symbolic stack-frame displacements and basic block branches, validates transitive machine-state footprints, and emits PE/ELF/Mach-O images that execute natively and agree with Terminal Psi on exit 0 across all four hosted platforms.
- **Remaining Open**: None for this fact row; all 9 places and 9 reject twins pass both native and interpreter gates with documented exit 0 and verified negative control rejection.
