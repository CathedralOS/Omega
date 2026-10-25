# Write-only borrow struct destination

Fact: Initializing a destination storage slot through a write-only borrow (&write) establishes definite initialization and the post-write upper bound (written <= 95u64), discharging exact subtraction 95u64 - written across all nine places.

Owning Task: `CONSERVATION-CONTRACT` / `LAYOUT-PLANS` (`TASKS.md`)
Claim Ticket: `0f97c9ed8ebe4f85a776303e1d14b298`

## Places

| Place | Positive Program | Reject Twin (invalid) | Transport Mechanism |
| --- | --- | --- | --- |
| root | [`main.omg`](main.omg) | — | canonical root witness initializing destination slot via `&write` before exact subtraction |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | The initialized slot value is bound to a local variable before exact subtraction. |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | The initialized slot value is stored in and retrieved from a record carrier field before exact subtraction. |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | The initialized slot value is passed as an explicit machine parameter before exact subtraction. |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | The initialized slot value is returned from a helper call before exact subtraction. |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | The initialized slot values are unified across reconverging match branches before exact subtraction. |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | A named constant supplies the subtrahend ceiling for the initialized destination slot. |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | The initialized slot value is transported across an explicit state transition before exact subtraction. |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | Another machine forwards the initialized destination slot before exact subtraction. |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | Target-scoped machine bodies initialize and consume the write-only borrow destination across all hosted targets. |

## Pinned Fixtures

The nine positive places and their nine reject twins are pinned as corpus fixtures and golden rows:

- `tests/omega/pass/facts/write_only_borrow_struct_destination_through_<place>_exit` (`build.omg`, `main.omg`, `expected_native_exit.txt`)
- `tests/omega/fail/facts/write_only_borrow_struct_destination_through_<place>_rejected` (`build.omg`, `main.omg`, `expected.txt`)
- Check-side golden rows in `tests/omega/corpus_outcomes.txt` (`checked` for all 9 positive places and `rejected` with the decision-17 diagnostic for all 9 reject twins)
- Native execution golden rows in `tests/omega/corpus_native_windows_x86_64.txt` (`built ... exit:0` across all 9 positive places)

## Execution

Every positive place (and root `main.omg`) binds `ProgramEntry` for all four hosted
targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) in its `build.omg`
and declares `Expected exit: 0`. `omega run --both` reports Terminal Psi interpreter
and native execution agreement:

```sh
omega run --both samples/cli/facts/write_only_borrow_struct_destination/main.omg
omega run --both samples/cli/facts/write_only_borrow_struct_destination/local/main.omg
omega run --both samples/cli/facts/write_only_borrow_struct_destination/field/main.omg
omega run --both samples/cli/facts/write_only_borrow_struct_destination/parameter/main.omg
omega run --both samples/cli/facts/write_only_borrow_struct_destination/call_result/main.omg
omega run --both samples/cli/facts/write_only_borrow_struct_destination/match_join/main.omg
omega run --both samples/cli/facts/write_only_borrow_struct_destination/const_name/main.omg
omega run --both samples/cli/facts/write_only_borrow_struct_destination/other_state/main.omg
omega run --both samples/cli/facts/write_only_borrow_struct_destination/other_machine/main.omg
omega run --both samples/cli/facts/write_only_borrow_struct_destination/target_body/main.omg
```

Every positive place and the root witness observe `native exit 0` and `interp exit: 0`.

### Measured Discrimination and Obligation Class

In each positive place, caller storage starts at `DestinationSlot { written: 0u64 }` and is lent via `&write slot` to `initialize_destination(destination: &write DestinationSlot)`, which stores `35u64` into `destination.written` under domain `WrittenBound(v) requires v <= 95u64`.
Upon loan completion, owner read authority is restored with static guarantee `written <= 95u64`.
Exact subtraction `95u64 - slot.written` discharges cleanly at compile time and evaluates to `60u64` at runtime (whereas an unexecuted `&write` store would leave `written == 0u64`, evaluating to `95u64` and trapping).
The caller verifies `diff == 60u64`, transitioning to `done()` with exit 0.

Measured on `windows_x86_64` with `omega-latest.exe run --both`:

| Probe | Perturbation | Native (`windows_x86_64`) | Terminal Psi (`interp`) |
| --- | --- | --- | --- |
| Baseline (`main.omg`) | none (`written = 35u64`, `diff == 60u64`) | `native exit 0` | `interp exit: 0` |
| 1-token demanded-value perturbation | `diff == 60u64` -> `diff == 61u64` | `native exit -1073741795` (`0xC000001D` trap) | `interp crash: Trap (host presentation agrees)` |
| 1-token stored-value perturbation | `destination.written = 35u64` -> `destination.written = 36u64` | `native exit -1073741795` (`0xC000001D` trap) | `interp crash: Trap (host presentation agrees)` |

**Obligation class**: This fact combines a **compile-time loan/definite-initialization gate** (`&write` write-only borrow authority and post-loan readability restoration in `wiki/spec/terminal-psi/loans.md` and `wiki/spec/build/calling_plans.md`) with a **value-bound exact arithmetic proof obligation** (`WrittenBound(v) requires v <= 95u64` discharging `95u64 - written` under decision 17). Reading through `&write` or failing to initialize the borrowed destination is rejected statically by loan/initialization checking, and widening the post-write domain bound from `v <= 95u64` to `v <= 96u64` is rejected statically by exact-arithmetic proof checking across all nine places.

### Invalid controls and reject twins

Each place has an `invalid/` twin that widens the domain from `v <= 95u64` to `v <= 96u64`.
Under this widened domain, the exact subtraction `95u64 - written` cannot be proved safe against unsigned underflow (decision 17 -- exact arithmetic proof obligation).
The compiler strictly fails closed at compile time with:

```
exact arithmetic in machine ... may overflow `u64`: the operands are not provably in range (decision 17 -- exact arithmetic is a proof obligation)
```

| Place | `omega run --both` | Reject twin diagnostic |
| --- | --- | --- |
| `local` | native 0 / interp 0 | `exact arithmetic in machine \`probe\` state \`entry\` terminal expression may overflow \`u64\`` |
| `field` | native 0 / interp 0 | `exact arithmetic in machine \`probe\` state \`entry\` terminal expression may overflow \`u64\`` |
| `parameter` | native 0 / interp 0 | `exact arithmetic in machine \`subtract_parameter\` state \`entry\` terminal expression may overflow \`u64\`` |
| `call_result` | native 0 / interp 0 | `exact arithmetic in machine \`probe\` state \`entry\` terminal expression may overflow \`u64\`` |
| `match_join` | native 0 / interp 0 | `exact arithmetic in machine \`probe\` state \`entry\` terminal expression may overflow \`u64\`` |
| `const_name` | native 0 / interp 0 | `exact arithmetic in machine \`probe\` state \`entry\` terminal expression may overflow \`u64\`` |
| `other_state` | native 0 / interp 0 | `exact arithmetic in machine \`probe\` state \`consume\` terminal expression may overflow \`u64\`` |
| `other_machine` | native 0 / interp 0 | `exact arithmetic in machine \`evaluate_destination\` state \`entry\` terminal expression may overflow \`u64\`` |
| `target_body` | native 0 / interp 0 | `exact arithmetic in machine \`target_probe\` state \`entry\` terminal expression may overflow \`u64\`` |

## Requirements and Evidence (AGENTS.md)

- **Establishment**: Calling `initialize_destination(destination: &write DestinationSlot)` lends destination storage exclusively via write-only borrow authority, executing a verified store `destination.written = 35u64;` satisfying the `WrittenBound(v) requires v <= 95u64` domain predicate.
- **Identity and lifetime**: The write-only loan suspends caller readability and preserves frame storage identity. Normal return concludes the loan window, restoring owner read authority along with the refined post-write domain fact `written <= 95u64` across value bindings, carrier fields, parameter passing, return values, branch joins, named constants, state transitions, helper machines, and target-scoped bodies.
- **Composition**: In each place, the restored domain fact `written <= 95u64` composes directly with exact subtraction `95u64 - written` under contract `ensures result <= 95u64`, proving non-negative range without post-hoc caller equality guards.
- **Soundness boundaries**: In every reject twin, widening the post-write bound to `v <= 96u64` causes the static checker to refuse compilation under decision 17 (exact arithmetic proof obligation), proving that potential underflow cannot bypass semantic checks.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`)**: The write-only borrow, indirect store, and post-loan owner readability restoration lower cleanly into canonical target-neutral Terminal Psi (`PSITERM\0`) with explicit `WriteOnlyBorrow` access and execute identically in the Terminal Psi interpreter (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`)**: Backend provider planning maps destination record homes into target layout plans, lowering `&write DestinationSlot` into direct pointer passing across ABI register conventions (RCX on `windows_x86_64`, RDI on `linux_x86_64`, X0 on `macos_arm64` and `linux_arm64`).
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`)**: Backend code generation realizes the indirect 64-bit store and subsequent frame reload, maintaining disjoint stack activation storage and bounded stack frames without live-across-call spill corruption.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission)**: Native image emission links entry symbols and branch displacements into complete PE/ELF/Mach-O binaries that execute natively and agree with Terminal Psi on exit 0 across all four hosted platforms.
- **Remaining Open**: None for this fact row; all 9 places and 9 reject twins are pinned under `tests/omega/{pass,fail}/facts/` and pass both native and interpreter gates with documented exit 0 and verified negative control rejection.
