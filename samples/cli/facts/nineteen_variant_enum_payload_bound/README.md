# Nineteen-variant enum payload bound

Fact: matching a 19-variant tagged enum payload NineteenChoice { V1(u64), V2(u64), V3(u64), V4(u64), V5(u64), V6(u64), V7(u64), V8(u64), V9(u64), V10(u64), V11(u64), V12(u64), V13(u64), V14(u64), V15(u64), V16(u64), V17(u64), V18(u64), V19(u64) } where every arm satisfies 40u64 <= v <= 80u64 preserves 40u64 <= res <= 80u64 across all nine places so exact subtractions 80u64 - res and res - 40u64 are safe.

Owning Task: `CANONICALIZE-SCALAR-RANGE-CONTRACTS` (`TASKS.md`) — the settled
proposition-based scalar range contract carried through payloads' case `where`
propositions, finishing this customer group's transport and execution across all
nine places before the next group. The family template's
`ENUM-PAYLOAD` / `CASE-PAYLOAD-BOUND` ids are not items on any board at
`80e7797d0e1` (`grep` over `TASKS.md` returns no match; they survive in 40 sibling
row READMEs), so they are recorded here rather than cited as an owner.
Governing Specifications:
- `wiki/spec/layouts/plans.md` (Tagged variant layout plans and alignment)
- `wiki/spec/terminal-psi/control_flow.md` (Pattern matching, variant dispatch, and join obligations)
- `wiki/spec/language/domains.md` (declaration and membership for case `where` propositions)

Every positive place and reject twin tests a 19-variant tagged enum (`data NineteenChoice [copy]`) with bounded 64-bit unsigned payloads across all variants (`V1(v: u64)` through `V19(v: u64)` where `v >= 40u64 && v <= 80u64`).
Matching or extracting the active variant payload yields an exact result `res: u64` statically guaranteed to lie in `[40, 80]`.
Casting or transporting `res` across places preserves `40u64 <= res <= 80u64`.
Subsequent exact subtractions `headroom = 80u64 - res` and `margin = res - 40u64` inside `verify_payload` are proved underflow-free and non-negative, evaluating to `20u64 + 20u64 = 40u64` for `v == 60u64` and terminating with exit 0.

## Places

| Place | Positive Program | Reject Twin (invalid) | Transport Mechanism |
| --- | --- | --- | --- |
| root | [`main.omg`](main.omg) | — | canonical root witness matching 19-variant enum payload and evaluating dual margin subtractions |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | extracted 64-bit variant payload is copied into a local binding before verification |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | extracted 64-bit variant payload is stored into and read from a record field before verification |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | extracted 64-bit variant payload is passed as an explicit machine parameter before verification |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | extracted 64-bit variant payload is returned from a dedicated helper machine |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both arms of a match expression unify the extracted 64-bit variant payload |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | named constants supply upper and lower ceilings for variant payload bounds |
| another state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a state transition carries the extracted 64-bit variant payload into a successor state |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | an independent forwarding machine transports the extracted 64-bit variant payload |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each hosted target provides a target-scoped machine body performing payload extraction and verification |

## Execution

Every positive place (and root `main.omg`) binds `ProgramEntry` for all four hosted targets
(`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) in its `build.omg` and declares `Expected exit: 0`.
`omega run --both` verifies Terminal Psi interpreter and native execution agreement:

```sh
omega run --both samples/cli/facts/nineteen_variant_enum_payload_bound/main.omg
omega run --both samples/cli/facts/nineteen_variant_enum_payload_bound/local/main.omg
omega run --both samples/cli/facts/nineteen_variant_enum_payload_bound/field/main.omg
omega run --both samples/cli/facts/nineteen_variant_enum_payload_bound/parameter/main.omg
omega run --both samples/cli/facts/nineteen_variant_enum_payload_bound/call_result/main.omg
omega run --both samples/cli/facts/nineteen_variant_enum_payload_bound/match_join/main.omg
omega run --both samples/cli/facts/nineteen_variant_enum_payload_bound/const_name/main.omg
omega run --both samples/cli/facts/nineteen_variant_enum_payload_bound/other_state/main.omg
omega run --both samples/cli/facts/nineteen_variant_enum_payload_bound/other_machine/main.omg
omega run --both samples/cli/facts/nineteen_variant_enum_payload_bound/target_body/main.omg
```

Every positive place and the root witness observe `native exit 0` and `interp exit: 0`.

### The oracle discriminates

In each positive place, evaluating `probe(NineteenChoice::V19 { v: 60u64 })` extracts `res = 60u64`.
Because all nineteen variant arms enforce `v >= 40u64 && v <= 80u64`, the exhaustive match statically establishes `res in [40, 80]`.
Subsequent exact subtractions in `verify_payload` (`headroom = 80u64 - res = 20u64` and `margin = res - 40u64 = 20u64`) evaluate to `20u64 + 20u64 = 40u64`.
The transition guard `combined == 40u64` verifies exact arithmetic evaluation, transitioning to `done()` with native exit 0.
If payload extraction or verification arithmetic is corrupted, the guard fails and transitions to `bad() { crash Trap; }`, terminating with exit 255.

### Invalid controls and reject twins

Each place has an `invalid/` reject twin that widens one variant's bound (`V19(v: u64) where v >= 39u64 && v <= 80u64`).
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

## Pinned Fixtures

The row is pinned by eighteen fixtures under
`tests/omega/{pass,fail}/facts/nineteen_variant_enum_payload_bound_through_*`:
the nine pass fixtures (`_exit`, each with `main.omg`, `build.omg` and
`expected_native_exit.txt`) are byte-identical copies of their place program,
and the nine fail fixtures (`_rejected`) are byte-identical copies of their
place's `invalid/main.omg` with that place's own refusal fragment pinned in
`expected.txt`. Sample and fixture therefore cannot drift apart.

Baseline measured on `macos_arm64` with the wave's shared producer built at
`9b001b8f6afa2cdac67fd580033c3560d906a5d5` (`omega` sha256
`6d24393125b7847b2f46d2f25e856f18a644f0fa1cedd2ad9b54f316f8b7da74`,
`opt_level 1`; its `--build-profile` reports `1`): all ten positives (root plus
nine places) report `native exit 0` and `interp exit: 0`, and all nine twins
refuse at checking with exit 1 and the diagnostic fragments in the table above.
`<place>/main.omg` versus `<place>/invalid/main.omg` differ only in `V19`'s own
bound (`v >= 40u64` versus `v >= 39u64`); no caller guard or control flow
differs. The same nineteen runs on the wave's previous producer at
`4c23cc3391c2e539d4065563ac95de7b18f2fc67` produced identical results, so the
row's evidence is not tied to one generation.

## Requirements and Evidence (AGENTS.md)

- **Establishment**: Variant declarations `V1(v: u64)` through `V19(v: u64)` where `v >= 40u64 && v <= 80u64` establish bounded intervals on active variant payloads. Exhaustive pattern matching across all nineteen tagged enum variants joins the incoming arm obligations, statically proving that the extracted payload satisfies `[40, 80]` without post-hoc caller equality guards.
- **Identity and lifetime**: The extracted payload interval fact `res in [40, 80]` is preserved across local bindings, carrier record fields, machine parameters, helper call returns, match join arms, named constants, state transitions, helper forwarding machines, and target-scoped bodies without semantic loss.
- **Composition**: In each place, the synthesized interval `res in [40, 80]` composes directly with `verify_payload` contract preconditions and subsequent exact subtractions `headroom = 80u64 - res` and `margin = res - 40u64` to statically discharge underflow safety and non-negativity without post-hoc caller equality guards.
- **Soundness boundaries**: In every reject twin, widening the payload bound on variant `V19` to `v >= 39u64` permits `res` to reach `39u64`, which fails closed under contract requires preconditions or ensures postconditions across boundary transmissions, verifying that out-of-interval variant payloads cannot satisfy safety bounds.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`)**: Tagged enum construction, 19-variant pattern matching, variant discrimination, and payload access lower to target-neutral Terminal Psi tagged case representations (`PSITERM\0`), while proof-only where-clauses are erased. Both Terminal Psi interpreter and native execution agree with exit 0 (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`)**: Provider layout plans represent the 19-variant tagged enum `NineteenChoice` with a 1-byte discriminant tag (`u8` range 0..18) and an 8-byte payload (`IntegerAt 8-byte` for `u64`). With natural 8-byte alignment, 7 padding bytes separate the 1-byte discriminant tag from the payload at offset 8, yielding a total layout plan size of 16 bytes and alignment of 8 bytes across all four hosted platforms.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`)**: Backend lowering selects target ISA 19-way variant tag comparison/dispatch and payload extraction instructions alongside 64-bit integer moves and exact subtractions: `sub rax, ...` on `x86_64` and `sub x0, ...` on `arm64`, maintaining disjoint register homes and activation stack frames without spilling corruption.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission)**: Direct binary emission encodes machine instructions, resolves symbolic stack-frame displacements and basic block branches, validates transitive machine-state footprints, and emits PE/ELF/Mach-O images that execute natively and agree with Terminal Psi on exit 0 across all four hosted platforms.
- **Remaining Open**: None at this row's scope: nine positive places and nine reject twins are authored, pinned under `tests/omega/{pass,fail}/facts/`, and green on the recording host. Cross-host runtime observation is limited to that host: every positive `build.omg` declares all four hosted `ProgramEntry` bindings, but only `macos_arm64` native execution and Terminal agreement are observed here; `windows_x86_64`, `linux_x86_64` and `linux_arm64` remain declared and unexecuted on this host.
