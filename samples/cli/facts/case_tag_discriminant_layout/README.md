# Case tag discriminant layout

Fact: Constructing and matching a multi-variant case value preserves its exact variant payload bound (payload <= 64u64) alongside its target layout geometry (size_of / align_of), discharging exact subtraction 64u64 - payload across all nine places.

Owning Task: `LAYOUT-PLANS` / `CONSERVATION-CONTRACT` (`TASKS.md`)
Claim Ticket: `c4a52019b8e2444391e6dc698f2b3a10`

## Places

| Place | Positive Program | Reject Twin (invalid) | Transport Mechanism |
| --- | --- | --- | --- |
| root | [`main.omg`](main.omg) | — | canonical root witness matching a tagged sum value alongside layout geometry queries |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | the multi-variant Packet value is copied into a local binding before matching and exact subtraction |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the multi-variant Packet value is stored in and retrieved from a record carrier field |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | the multi-variant Packet value is passed as an explicit machine parameter |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | the multi-variant Packet value is returned from a helper machine call |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | the multi-variant Packet value is joined across conditional branch arms |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | the multi-variant Packet value is referenced through named constant definitions |
| another state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | the multi-variant Packet value is forwarded across an explicit machine state transition |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | the multi-variant Packet value is forwarded through an intermediate helper machine |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | the multi-variant Packet value is realized across four target-scoped machine bodies |

## Execution

Every positive place (and root `main.omg`) binds `ProgramEntry` for all four hosted
targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) in its `build.omg`
and declares `Expected exit: 0`. `omega run --both` reports Terminal Psi interpreter
and native execution agreement:

```sh
omega run --both samples/cli/facts/case_tag_discriminant_layout/main.omg
omega run --both samples/cli/facts/case_tag_discriminant_layout/local/main.omg
omega run --both samples/cli/facts/case_tag_discriminant_layout/field/main.omg
omega run --both samples/cli/facts/case_tag_discriminant_layout/parameter/main.omg
omega run --both samples/cli/facts/case_tag_discriminant_layout/call_result/main.omg
omega run --both samples/cli/facts/case_tag_discriminant_layout/match_join/main.omg
omega run --both samples/cli/facts/case_tag_discriminant_layout/const_name/main.omg
omega run --both samples/cli/facts/case_tag_discriminant_layout/other_state/main.omg
omega run --both samples/cli/facts/case_tag_discriminant_layout/other_machine/main.omg
omega run --both samples/cli/facts/case_tag_discriminant_layout/target_body/main.omg
```

Every positive place and the root witness observe `native exit 0` and `interp exit: 0`.

### The oracle discriminates

In each positive place, `size_of<Packet>()` (16u64) and `align_of<Packet>()` (8u64) evaluate to exactly `24u64`.
Layout geometry verification `geometry_diff: u64 = (s + a) - 24u64` evaluates to `0u64`.
Constructing `Packet::Bounded { payload: 24u64 }` with bound `payload <= 64u64` proves exact subtraction `64u64 - payload` in `inspect` safe against underflow, producing `40u64`.
Matching `Packet::Empty {}` produces `0u64`.
The caller validates `eval(a) == 40u64 && eval(b) == 0u64` and transitions to `done()` with exit 0.
Corrupting the payload to `23u64` or breaking geometry makes both native and interpreter fail the guard and transition to `bad() { crash Trap; }`, terminating with exit 255.

### Invalid controls and reject twins

Each place has an `invalid/` twin that widens the payload bound in `Packet` from `payload <= 64u64` to `payload <= 80u64`.
This leaves the exact subtraction `64u64 - payload` incapable of proving non-negative range (exact arithmetic proof obligation / decision 17).
Checking fails closed at compile time with:

```
exact arithmetic in machine ... return value may overflow `u64`: the operands are not provably in range (decision 17 -- exact arithmetic is a proof obligation)
```

| Place | `omega run --both` | Reject twin diagnostic |
| --- | --- | --- |
| `local` | native 0 / interp 0 | `exact arithmetic in machine \`eval\` state \`inspect\` return value may overflow \`u64\`` |
| `field` | native 0 / interp 0 | `exact arithmetic in machine \`eval\` state \`inspect\` return value may overflow \`u64\`` |
| `parameter` | native 0 / interp 0 | `exact arithmetic in machine \`eval\` state \`inspect\` return value may overflow \`u64\`` |
| `call_result` | native 0 / interp 0 | `exact arithmetic in machine \`eval\` state \`inspect\` return value may overflow \`u64\`` |
| `match_join` | native 0 / interp 0 | `exact arithmetic in machine \`eval\` state \`inspect\` return value may overflow \`u64\`` |
| `const_name` | native 0 / interp 0 | `exact arithmetic in machine \`eval\` state \`inspect\` return value may overflow \`u64\`` |
| `other_state` | native 0 / interp 0 | `exact arithmetic in machine \`eval\` state \`inspect\` return value may overflow \`u64\`` |
| `other_machine` | native 0 / interp 0 | `exact arithmetic in machine \`inspect\` state \`do_match\` return value may overflow \`u64\`` |
| `target_body` | native 0 / interp 0 | `exact arithmetic in machine \`eval\` state \`inspect\` return value may overflow \`u64\`` |

## Requirements and Evidence (AGENTS.md)

- **Establishment**: `Packet::Bounded { payload: 24u64 }` establishes the payload bound `payload <= 64u64` and `Packet::Empty {}` establishes the nullary variant. Layout geometry queries `size_of<Packet>()` and `align_of<Packet>()` evaluate target layout constants 16u64 and 8u64.
- **Identity and lifetime**: Multi-variant tagged case representations preserve active variant discriminant and payload bound invariants across value copies, carrier struct fields, parameter passing, return values, branch joins, named constants, state transitions, helper machines, and target-scoped bodies without semantic degradation.
- **Composition**: Destructuring `Packet::Bounded { payload }` introduces the static assumption `payload <= 64u64` into the match arm environment, discharging the non-negative range proof obligation for unsigned subtraction `64u64 - payload` under postcondition `ensures result <= 64u64`.
- **Soundness boundaries**: In every reject twin, widening the payload bound to `payload <= 80u64` causes the static checker to refuse compilation under decision 17 (exact arithmetic proof obligation), proving that unsafe underflows are caught before executable generation.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`)**: The sum constructor, discriminant tag matching, and layout geometry intrinsics lower cleanly into target-neutral Terminal Psi (`PSITERM\0`) and execute identically in the Terminal Psi interpreter (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`)**: Backend provider planning joins target-specific layout plans (`size_of` = 16, `align_of` = 8) with discriminant tagging (8-byte tag, 8-byte payload) across all four hosted ABI calling plans.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`)**: Backend code generation emits target ISA instructions (`sub`, conditional branches, and discriminant checks), allocating register homes and activation stack slots without live-variable spilling corrupting variant discriminants.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission)**: Direct PE/ELF/Mach-O image generation resolves entry symbols, basic block branches, and image relocations, producing native binaries that agree with Terminal Psi on exit 0 across all hosted targets.
- **Remaining Open**: None for this fact row; all 9 places and 9 reject twins pass both native and interpreter gates with documented exit 0 and verified negative control rejection.
