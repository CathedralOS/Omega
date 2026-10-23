# signed_32bit_division_quotient_bound

Fact: a 32-bit signed i32 division of bounded inputs (dividend >= 200i32 && dividend <= 200i32, divisor >= 5i32 && divisor <= 5i32) establishes quotient >= 40i32 && quotient <= 40i32 so exact 32-bit signed subtraction quotient - 40i32 is proved overflow-free across all nine places.

Owning board item: `CANONICALIZE-SCALAR-RANGE-CONTRACTS` / `ARITHMETIC-POLICY-REALIZATION` (`TASKS.md`).
Exact requirement: the quotient fact `quotient >= 40i32 && quotient <= 40i32` established where a bounded signed division is formed transports unchanged through parameters, `ensures` results, locals, record fields, match joins, named constants, successor-state parameters, forwarding machines and target-scoped bodies into the consuming exact subtraction `quotient - 40i32`; each place carries a reject twin that still refuses.

Every positive place and reject twin tests 32-bit signed division (`dividend / divisor`, `i32`).
With `dividend in [200, 200]` and `divisor in [5, 5]`, signed division statically establishes the exact quotient interval `[40, 40]`, so the downstream exact subtraction `quotient - 40i32` is proved overflow-free and evaluates to `0i32`.

## Places

| Place | Positive Program | Reject Twin (invalid) | Transport Mechanism |
| --- | --- | --- | --- |
| root | [`main.omg`](main.omg) | — | canonical root witness dividing 200i32 / 5i32 and discharging the subtraction contract |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | the quotient is copied into a local binding before the contract call |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the quotient is stored into and read from a `DivResult` record field |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | the quotient is passed as an explicit machine parameter |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | a dedicated `divide` machine returns the quotient under `ensures result >= 40i32 && result <= 40i32` |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both arms of a `match` unify the quotient under the same `ensures` |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | named constants `DIVIDEND` / `DIVISOR` / `QUOTIENT` supply the bounded division |
| another state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a `step(quotient)` successor state receives the quotient under its own `requires` |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | a forwarding machine carries the quotient to the verifying machine |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each hosted target provides a `verify_target` body performing the division and verification |

## Execution

Every positive place (and root `main.omg`) binds `ProgramEntry` for all four hosted targets
(`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) in its `build.omg` and declares `Expected exit: 0`.
`omega run --both` verifies Terminal Psi interpreter and native execution agreement:

```sh
omega run --both samples/cli/facts/signed_32bit_division_quotient_bound/main.omg
omega run --both samples/cli/facts/signed_32bit_division_quotient_bound/local/main.omg
omega run --both samples/cli/facts/signed_32bit_division_quotient_bound/field/main.omg
omega run --both samples/cli/facts/signed_32bit_division_quotient_bound/parameter/main.omg
omega run --both samples/cli/facts/signed_32bit_division_quotient_bound/call_result/main.omg
omega run --both samples/cli/facts/signed_32bit_division_quotient_bound/match_join/main.omg
omega run --both samples/cli/facts/signed_32bit_division_quotient_bound/const_name/main.omg
omega run --both samples/cli/facts/signed_32bit_division_quotient_bound/other_state/main.omg
omega run --both samples/cli/facts/signed_32bit_division_quotient_bound/other_machine/main.omg
omega run --both samples/cli/facts/signed_32bit_division_quotient_bound/target_body/main.omg
```

Measured at `4c23cc3391c2e539d4065563ac95de7b18f2fc67` with the `gen-4c23cc3391c` producer (`opt-level 1`, macOS arm64, 2026-10-23):
all ten positive programs report `native exit 0` and `interp exit: 0`.

### The oracle discriminates

In each positive place, evaluating the bounded division with `dividend = 200i32` and `divisor = 5i32` computes `quotient = 40i32`, inside the statically established interval `[40, 40]`.
`verify_quotient` discharges its `requires` with that fact and evaluates `quotient - 40i32 = 0i32`.
`main`'s guard `probe() == 0` then transitions to `done()`, exiting 0.
Corrupting the demanded result by one token (`probe() == 0` to `probe() == 1`) routes execution to `state bad() { crash Trap; }`; measured on both engines: `native signal 5` and `interp crash: Trap (host presentation agrees)`, process exit 255.
A corrupted quotient or bound therefore cannot be read as success.

### Invalid controls and reject twins

Each place has an `invalid/` reject twin that widens only the divisor bound (`divisor >= 5i32` to `divisor >= 4i32`, or the constant `5i32` to `4i32` in `const_name`, or `let divisor: i32 = 5i32` to `4i32` in each target body), admitting `quotient = 50i32` and violating the consuming contract (`requires quotient >= 40i32 && quotient <= 40i32`, or `ensures` where the quotient leaves through a result).
Each twin exits 200 with `native compile FAILED` and the exact diagnostic pinned by its fixture's `expected.txt`:

| Place | Exact recorded diagnostic (message form) |
| --- | --- |
| `local` | `cannot prove requires contract for call verify_quotient from divide_local: quotient >= 40 && quotient <= 40 (needs fact `quotient >= 40 && quotient <= 40` here; establish it before the call via a prior contract guarantee, domain membership, or guard) in state `entry`` |
| `field` | `cannot prove requires contract for call verify_quotient from compute_field: holder.quotient >= 40 && holder.quotient <= 40 (needs fact `quotient >= 40 && quotient <= 40` here; establish it before the call via a prior contract guarantee, domain membership, or guard) in state `entry`` |
| `parameter` | `cannot prove requires contract for call verify_parameter from compute_param: dividend / divisor >= 40 && dividend / divisor <= 40 (needs fact `quotient >= 40 && quotient <= 40` here; establish it before the call via a prior contract guarantee, domain membership, or guard) in state `entry`` |
| `call_result` | `cannot prove ensures contract for exit from divide at statement 0: result >= 40 && result <= 40` |
| `match_join` | `cannot prove ensures contract for exit from choose_quotient at statement 0: result >= 40 && result <= 40` |
| `const_name` | `cannot prove requires contract for call verify_quotient from probe: quotient >= 40 && quotient <= 40 (needs fact `quotient >= 40 && quotient <= 40` here; establish it before the call via a prior contract guarantee, domain membership, or guard) in state `entry`` |
| `other_state` | `cannot prove requires contract for call step from verify_state: dividend / divisor >= 40 && dividend / divisor <= 40 (needs fact `quotient >= 40 && quotient <= 40` here; establish it before the call via a prior contract guarantee, domain membership, or guard) in state `entry`` |
| `other_machine` | `cannot prove requires contract for call forward from compute_forward: dividend / divisor >= 40 && dividend / divisor <= 40 (needs fact `quotient >= 40 && quotient <= 40` here; establish it before the call via a prior contract guarantee, domain membership, or guard) in state `entry`` |
| `target_body` | `cannot prove requires contract for call verify_quotient from verify_target: quotient >= 40 && quotient <= 40 (needs fact `quotient >= 40 && quotient <= 40` here; establish it before the call via a prior contract guarantee, domain membership, or guard) in state `entry`` (reported once per target body, 4 diagnostics) |

The eighteen fixtures are pinned under
[tests/omega/pass/facts/signed_32bit_division_quotient_bound_through_&lt;place&gt;_exit](../../../tests/omega/pass/facts)
and [tests/omega/fail/facts/signed_32bit_division_quotient_bound_through_&lt;place&gt;_rejected](../../../tests/omega/fail/facts)
with their corpus golden rows recorded through `python3 tools/corpus_gate.py --record --filter …`.

## Requirements and Evidence (AGENTS.md)

- **Establishment**: Caller contract preconditions `dividend >= 200i32 && dividend <= 200i32` and `divisor >= 5i32 && divisor <= 5i32` bound the operands; 32-bit signed division statically establishes the exact quotient interval `[40, 40]`, discharging the subsequent exact subtraction `quotient - 40i32` (evaluating to `0i32`) without a post-hoc caller equality guard. The divisor non-zero and `i32::MIN / -1` overflow obligations remain intact.
- **Identity and lifetime**: The computed interval fact `quotient in [40, 40]` is preserved across local bindings, `DivResult` record fields, machine parameters, helper call returns (`ensures`), match join arms, named constants, successor-state parameters, forwarding machines, and target-scoped bodies without semantic loss.
- **Composition**: In each place, the synthesized interval `quotient in [40, 40]` composes directly with `verify_quotient` contract preconditions and the exact subtraction to statically discharge overflow safety.
- **Soundness boundaries**: In every reject twin, widening the divisor bound to `>= 4i32` permits `quotient = 50i32`, which fails closed under the `requires`/`ensures` contract across boundary transmissions, verifying that out-of-interval quotients cannot satisfy the safety bound and that the acceptance is not a caller-side guard.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`)**: Signed 32-bit division lowers to target-neutral Terminal Psi with divisor non-zero certificates and range bounds intact; proof annotations are erased. Both the Terminal Psi interpreter and native execution agree with exit 0 (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`)**: `i32` settles into 4-byte layouts (`size_of = 4`, `align_of = 4`). Standard calling conventions place parameters in 32-bit register lanes (`ECX`/`EDX` on Windows x86_64, `EDI`/`ESI` on Linux x86_64, `W0`/`W1` on arm64) and return quotients in `EAX` / `W0`.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`)**: On x86_64 the backend selects 32-bit signed division `idiv r32` (with the sign-extension of `eax` into `edx:eax`); on arm64 it selects 32-bit signed division `sdiv w0, w0, w1`, maintaining disjoint register homes and activation stack frames without spilling corruption.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission)**: Direct PE/ELF/Mach-O image emission resolves all displacements and emits valid native machine code; native execution exit 0 agrees with Terminal Psi simulation across the hosted targets, and the guard-corruption probe above shows both engines report the Trap outcome (`native signal 5` / `interp crash: Trap`) rather than a false success.
- **Remaining Open**: None for this fact row; all nine places and nine reject twins pass both native and interpreter gates with documented exit 0 and verified negative control rejection, and the eighteen fixtures are pinned with recorded corpus rows.