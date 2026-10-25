# Eight-variant enum payload bound

Fact: matching an 8-variant tagged enum payload Packet { V1(u64), V2(u64), V3(u64), V4(u64), V5(u64), V6(u64), V7(u64), V8(u64) } where every arm satisfies 40u64 <= v <= 80u64 preserves 40u64 <= res <= 80u64 across all nine places so exact subtractions 80u64 - res and res - 40u64 are safe.

Owning Task: `ENUM-PAYLOAD` / `CASE-PAYLOAD-BOUND` (`TASKS.md`)
Obligation Class: Tagged case payload layout plans and variant discriminant tags (`wiki/spec/layouts/plans.md`) — an 8-variant enum tagged case payload preserves payload interval bounds `40u64 <= payload <= 80u64` to discharge exact subtractions `80u64 - payload` and `payload - 40u64` across all 9 transport places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`).

Every positive place and reject twin tests an 8-variant tagged enum (`data Packet [copy]`) with bounded 64-bit unsigned payloads across all variants (`V1(v: u64)`, `V2(v: u64)`, `V3(v: u64)`, `V4(v: u64)`, `V5(v: u64)`, `V6(v: u64)`, `V7(v: u64)`, and `V8(v: u64)` where `v >= 40u64 && v <= 80u64`).
Matching or extracting the active variant payload yields an exact result `res: u64` statically guaranteed to lie in `[40, 80]`.
Casting or transporting `res` across places preserves `40u64 <= res <= 80u64`.
Subsequent exact subtractions `headroom = 80u64 - res` and `margin = res - 40u64` inside `verify_payload` are proved underflow-free and non-negative, evaluating to `20u64 + 20u64 = 40u64` for `v == 60u64` and terminating with exit 0.

## Places

| Place | Positive Program | Reject Twin (invalid) | Transport Mechanism |
| --- | --- | --- | --- |
| root | [`main.omg`](main.omg) | — | canonical root witness matching 8-variant enum payload and evaluating dual margin subtractions |
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
omega run --both samples/cli/facts/eight_variant_enum_payload_bound/main.omg
omega run --both samples/cli/facts/eight_variant_enum_payload_bound/local/main.omg
omega run --both samples/cli/facts/eight_variant_enum_payload_bound/field/main.omg
omega run --both samples/cli/facts/eight_variant_enum_payload_bound/parameter/main.omg
omega run --both samples/cli/facts/eight_variant_enum_payload_bound/call_result/main.omg
omega run --both samples/cli/facts/eight_variant_enum_payload_bound/match_join/main.omg
omega run --both samples/cli/facts/eight_variant_enum_payload_bound/const_name/main.omg
omega run --both samples/cli/facts/eight_variant_enum_payload_bound/other_state/main.omg
omega run --both samples/cli/facts/eight_variant_enum_payload_bound/other_machine/main.omg
omega run --both samples/cli/facts/eight_variant_enum_payload_bound/target_body/main.omg
```

Every positive place and the root witness observe `native exit 0` and `interp exit: 0`.

### The oracle discriminates

In each positive place, evaluating `probe(Packet::V8 { v: 60u64 })` extracts `res = 60u64`.
Because all eight variant arms enforce `v >= 40u64 && v <= 80u64`, the exhaustive match statically establishes `res in [40, 80]`.
Subsequent exact subtractions in `verify_payload` (`headroom = 80u64 - res = 20u64` and `margin = res - 40u64 = 20u64`) evaluate to `20u64 + 20u64 = 40u64`.
The transition guard `combined == 40u64` verifies exact arithmetic evaluation, transitioning to `done()` with native exit 0.
If payload extraction or verification arithmetic is corrupted, the guard fails and transitions to `bad() { crash Trap; }`, terminating with exit 255.

Measured discrimination: A 1-token perturbation altering the demanded value from `combined == 40u64` to `combined == 41u64` in `main.omg` was executed against `omega run --both`:
- Native execution: trapped immediately with `native exit -1073741795` (STATUS_ILLEGAL_INSTRUCTION / Trap crash).
- Terminal Psi interpreter: trapped with `interp crash: Trap (host presentation agrees)`.
Both engines trapped consistently on the corrupted value, verifying bidirectional discrimination.

## Pinned Corpus Fixtures

All 18 corpus fixtures are pinned under `tests/omega/`:

### Pass Fixtures (`tests/omega/pass/facts/`)
- `eight_variant_enum_payload_bound_through_local_exit`
- `eight_variant_enum_payload_bound_through_field_exit`
- `eight_variant_enum_payload_bound_through_parameter_exit`
- `eight_variant_enum_payload_bound_through_call_result_exit`
- `eight_variant_enum_payload_bound_through_match_join_exit`
- `eight_variant_enum_payload_bound_through_const_name_exit`
- `eight_variant_enum_payload_bound_through_other_state_exit`
- `eight_variant_enum_payload_bound_through_other_machine_exit`
- `eight_variant_enum_payload_bound_through_target_body_exit`

### Fail Fixtures (`tests/omega/fail/facts/`)
- `eight_variant_enum_payload_bound_through_local_rejected`
- `eight_variant_enum_payload_bound_through_field_rejected`
- `eight_variant_enum_payload_bound_through_parameter_rejected`
- `eight_variant_enum_payload_bound_through_call_result_rejected`
- `eight_variant_enum_payload_bound_through_match_join_rejected`
- `eight_variant_enum_payload_bound_through_const_name_rejected`
- `eight_variant_enum_payload_bound_through_other_state_rejected`
- `eight_variant_enum_payload_bound_through_other_machine_rejected`
- `eight_variant_enum_payload_bound_through_target_body_rejected`

### Invalid controls and reject twins

Each place has an `invalid/` reject twin that widens one variant's bound (`V8(v: u64) where v >= 39u64 && v <= 80u64`).
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

- **Establishment**: Variant declarations `V1(v: u64)`, `V2(v: u64)`, `V3(v: u64)`, `V4(v: u64)`, `V5(v: u64)`, `V6(v: u64)`, `V7(v: u64)`, and `V8(v: u64)` where `v >= 40u64 && v <= 80u64` establish bounded intervals on active variant payloads. Exhaustive pattern matching across all eight tagged enum variants joins the incoming arm obligations, statically proving that the extracted payload satisfies `[40, 80]` without post-hoc caller equality guards.
- **Identity and lifetime**: The extracted payload interval fact `res in [40, 80]` is preserved across local bindings, carrier record fields, machine parameters, helper call returns, match join arms, named constants, state transitions, helper forwarding machines, and target-scoped bodies without semantic loss.
- **Composition**: In each place, the synthesized interval `res in [40, 80]` composes directly with `verify_payload` contract preconditions and subsequent exact subtractions `headroom = 80u64 - res` and `margin = res - 40u64` to statically discharge underflow safety and non-negativity without post-hoc caller equality guards.
- **Soundness boundaries**: In every reject twin, widening the payload bound on variant `V8` to `v >= 39u64` permits `res` to reach `39u64`, which fails closed under contract requires preconditions or ensures postconditions across boundary transmissions, verifying that out-of-interval variant payloads cannot satisfy safety bounds.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`)**: Tagged enum construction, variant discrimination, and payload access lower to target-neutral Terminal Psi tagged case representations (`PSITERM\0`), while proof-only where-clauses are erased. Both Terminal Psi interpreter and native execution agree with exit 0 (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`)**: Provider layout plans represent the 8-variant tagged enum `Packet` with its discriminant tag and 8-byte payload (`IntegerAt 8-byte` for `u64`), respecting layout alignment and calling conventions across all four hosted platforms.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`)**: Backend lowering selects target ISA variant tag comparison and branch instructions alongside 64-bit integer moves and exact subtractions: `sub rax, ...` on `x86_64` and `sub x0, ...` on `arm64`, maintaining disjoint register homes and activation stack frames without spilling corruption.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission)**: Direct binary emission encodes machine instructions, resolves symbolic stack-frame displacements and basic block branches, validates transitive machine-state footprints, and emits PE/ELF/Mach-O images that execute natively and agree with Terminal Psi on exit 0 across all four hosted platforms.
- **Remaining Open**: None for this fact row; all 9 places and 18 reject twins pass both native and interpreter gates with documented exit 0 and verified negative control rejection.
