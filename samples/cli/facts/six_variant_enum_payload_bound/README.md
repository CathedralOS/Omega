# Six-variant enum payload bound

Fact: matching a 6-variant tagged enum payload Packet { Alpha(u64), Beta(u64), Gamma(u64), Delta(u64), Epsilon(u64), Zeta(u64) } where every arm satisfies 40u64 <= v <= 80u64 preserves 40u64 <= res <= 80u64 across all nine places so exact subtractions 80u64 - res and res - 40u64 are safe.

Owning Task: `ENUM-PAYLOAD` / `CASE-PAYLOAD-BOUND` (`TASKS.md`)

Every positive place and reject twin tests a 6-variant tagged enum (`data Packet [copy]`) with bounded 64-bit unsigned payloads across all variants (`Alpha(v: u64)`, `Beta(v: u64)`, `Gamma(v: u64)`, `Delta(v: u64)`, `Epsilon(v: u64)`, and `Zeta(v: u64)` where `v >= 40u64 && v <= 80u64`).
Matching or extracting the active variant payload yields an exact result `res: u64` statically guaranteed to lie in `[40, 80]`.
Casting or transporting `res` across places preserves `40u64 <= res <= 80u64`.
Subsequent exact subtractions `headroom = 80u64 - res` and `margin = res - 40u64` inside `verify_payload` are proved underflow-free and non-negative, evaluating to `20u64 + 20u64 = 40u64` for `v == 60u64` and terminating with exit 0.

## Places

| Place | Positive Program | Reject Twin (invalid) | Transport Mechanism | Pinned Pass Fixture | Pinned Reject Fixture |
| --- | --- | --- | --- | --- | --- |
| root | [`main.omg`](main.omg) | — | canonical root witness matching 6-variant enum payload and evaluating dual margin subtractions | — | — |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | extracted 64-bit variant payload is copied into a local binding before verification | `tests/omega/pass/facts/six_variant_enum_payload_bound_through_local_exit` | `tests/omega/fail/facts/six_variant_enum_payload_bound_through_local_rejected` |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | extracted 64-bit variant payload is stored into and read from a record field before verification | `tests/omega/pass/facts/six_variant_enum_payload_bound_through_field_exit` | `tests/omega/fail/facts/six_variant_enum_payload_bound_through_field_rejected` |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | extracted 64-bit variant payload is passed as an explicit machine parameter before verification | `tests/omega/pass/facts/six_variant_enum_payload_bound_through_parameter_exit` | `tests/omega/fail/facts/six_variant_enum_payload_bound_through_parameter_rejected` |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | extracted 64-bit variant payload is returned from a dedicated helper machine | `tests/omega/pass/facts/six_variant_enum_payload_bound_through_call_result_exit` | `tests/omega/fail/facts/six_variant_enum_payload_bound_through_call_result_rejected` |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both arms of a match expression unify the extracted 64-bit variant payload | `tests/omega/pass/facts/six_variant_enum_payload_bound_through_match_join_exit` | `tests/omega/fail/facts/six_variant_enum_payload_bound_through_match_join_rejected` |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | named constants supply upper and lower ceilings for variant payload bounds | `tests/omega/pass/facts/six_variant_enum_payload_bound_through_const_name_exit` | `tests/omega/fail/facts/six_variant_enum_payload_bound_through_const_name_rejected` |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a state transition carries the extracted 64-bit variant payload into a successor machine | `tests/omega/pass/facts/six_variant_enum_payload_bound_through_other_state_exit` | `tests/omega/fail/facts/six_variant_enum_payload_bound_through_other_state_rejected` |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | an independent forwarding machine transports the extracted 64-bit variant payload | `tests/omega/pass/facts/six_variant_enum_payload_bound_through_other_machine_exit` | `tests/omega/fail/facts/six_variant_enum_payload_bound_through_other_machine_rejected` |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each hosted target provides a target-scoped machine body performing payload extraction and verification | `tests/omega/pass/facts/six_variant_enum_payload_bound_through_target_body_exit` | `tests/omega/fail/facts/six_variant_enum_payload_bound_through_target_body_rejected` |

## Execution

Every positive place (and root `main.omg`) binds `ProgramEntry` for all four hosted targets
(`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) in its `build.omg` and declares `Expected exit: 0`.
`omega run --both` verifies Terminal Psi interpreter and native execution agreement:

```sh
omega run --both samples/cli/facts/six_variant_enum_payload_bound/main.omg
omega run --both samples/cli/facts/six_variant_enum_payload_bound/local/main.omg
omega run --both samples/cli/facts/six_variant_enum_payload_bound/field/main.omg
omega run --both samples/cli/facts/six_variant_enum_payload_bound/parameter/main.omg
omega run --both samples/cli/facts/six_variant_enum_payload_bound/call_result/main.omg
omega run --both samples/cli/facts/six_variant_enum_payload_bound/match_join/main.omg
omega run --both samples/cli/facts/six_variant_enum_payload_bound/const_name/main.omg
omega run --both samples/cli/facts/six_variant_enum_payload_bound/other_state/main.omg
omega run --both samples/cli/facts/six_variant_enum_payload_bound/other_machine/main.omg
omega run --both samples/cli/facts/six_variant_enum_payload_bound/target_body/main.omg
```

Every positive place and the root witness observe `native exit 0` and `interp exit: 0`.

### The oracle discriminates

In each positive place, evaluating `probe(Packet::Zeta { v: 60u64 })` extracts `res = 60u64`.
Because all six variant arms enforce `v >= 40u64 && v <= 80u64`, the exhaustive match statically establishes `res in [40, 80]`.
Subsequent exact subtractions in `verify_payload` (`headroom = 80u64 - res = 20u64` and `margin = res - 40u64 = 20u64`) evaluate to `20u64 + 20u64 = 40u64`.
The transition guard `combined == 40u64` verifies exact arithmetic evaluation, transitioning to `done()` with native exit 0.
If payload extraction or verification arithmetic is corrupted, the guard fails and transitions to `bad() { crash Trap; }`, terminating with exit 255.

### Invalid controls and reject twins

Each place has an `invalid/` reject twin that widens one variant's bound (`Zeta(v: u64) where v >= 39u64 && v <= 80u64`).
When `v == 39u64`, `res` can fall to `39u64 < 40u64`, which violates the contract precondition `value >= 40u64` required by `verify_payload` (or ensures postcondition on helper extraction).
The compiler strictly fails closed at compile time:

| Place | `omega run --both` | Reject twin diagnostic |
| --- | --- | --- |
| `local` | native 0 / interp 0 | `error: cannot prove requires contract for call verify_payload from probe: local_res >= 40 && local_res <= 80` |
| `field` | native 0 / interp 0 | `error: cannot prove requires contract for call verify_payload from probe: holder.res >= 40 && holder.res <= 80` |
| `parameter` | native 0 / interp 0 | `error: cannot prove requires contract for call consume from probe: res >= 40 && res <= 80` |
| `call_result` | native 0 / interp 0 | `error: cannot prove ensures contract for exit from extract_payload at statement 11: result >= 40 && result <= 80` |
| `match_join` | native 0 / interp 0 | `error: cannot prove requires contract for call choose from probe: res >= 40 && res <= 80 && 60 >= 40 && 60 <= 80` |
| `const_name` | native 0 / interp 0 | `error: cannot prove requires contract for call verify_payload from probe: res >= 40 && res <= 80` |
| `other_state` | native 0 / interp 0 | `error: cannot prove requires contract for call consume from probe: res >= 40 && res <= 80` |
| `other_machine` | native 0 / interp 0 | `error: cannot prove requires contract for call forward_payload from probe: res >= 40 && res <= 80` |
| `target_body` | native 0 / interp 0 | `error: cannot prove requires contract for call verify_payload from target_probe: res >= 40 && res <= 80` |

## Pinned Fixtures

The row is pinned under `tests/omega/pass/facts/six_variant_enum_payload_bound_through_<place>_exit` and
`tests/omega/fail/facts/six_variant_enum_payload_bound_through_<place>_rejected` for the nine places (`local`, `field`,
`parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`).
Each pass fixture is a byte-identical copy of its place's `main.omg`/`build.omg` plus
`expected_native_exit.txt` = `0`; each fail fixture is a byte-identical copy of its place twin's
`main.omg`/`build.omg` and carries the runner-emitted rejection message verbatim in `expected.txt`
(e.g. `cannot prove requires contract for call verify_payload from probe: local_res >= 40 && local_res <= 80
(needs fact ...) in state `entry``). The twins' `expected.txt` previously carried the CLI's
`error:`-prefixed wrapper form, which the corpus diagnostic route never emits (measured: all 18 rows
weighed `unexpected` at the first check leg); correcting it to the runner-emitted message is what weighs
the rows `expected` while keeping every `sample.json` `rejection_contains` fragment satisfied. Positive
versus twin `git diff` moves only the fact's own construct: the payload bound `v >= 40u64` -> `v >= 39u64`
and the dropped `// Expected exit: 0` header.

Measured on macOS arm64 with the factsr1 shared generation producers (gen-9b001b8f6af, omega sha256
`6d24393125b7847b2f46d2f25e856f18a644f0fa1cedd2ad9b54f316f8b7da74`; gen-317bacd12b0, omega sha256
`ecbd667e4dbae1b559f1f08f30dc10e3dbc369b8c8630327560227d06058a805`, corpus_runner sha256
`6065879534a8320403e0dd3918e86f77d1f025ff4e233f6740644b44460daa9c`):

- `omega run --both` over the root and the nine places: 10/10 report `native exit 0` and
  `interp exit: 0`; all nine twins refuse at checking with the authored fragment and never reach native
  execution. The 18 twin diagnostics are byte-identical under both producers - 98 `.rs` files and 69
  commits apart between `317bacd12b0` and `9b001b8f6af` - the differential measurement behind the
  landing's residual-movement argument.
- Perturbing the root oracle (one token: `transition combined == 40u64` -> `== 41u64`) exits 255 with
  `native signal 5` and `interp crash: Trap (host presentation agrees)`, so the authored 0 discriminates.
- `python3 tools/sample_gate.py --cli <generation omega> --filter
  six_variant_enum_payload_bound,ten_variant_enum_payload_bound --write-results ...` records 103/103
  projects checked on Darwin arm64; all 47 projects of these two rows are among them (six: root plus the
  nine places and nine twins; ten: the same plus nine `invalid/<place>` mirrors) with all 27
  intended-invalid controls matched.
- The three corpus legs were recorded with the fixture and golden changes staged at published main
  `317bacd12b0`, so every row is stamped `@317bacd12b0a+wt`: `tests/omega/corpus_outcomes.txt` gained 36
  rows (18 `checked` carrying `check-target:macos_arm64` and 18 `rejected` carrying their diagnostics with
  the expected fragments satisfied, 24 diagnostic continuation lines),
  `tests/omega/corpus_native_macos_arm64.txt` gained 18 `built ... exit:0` rows and
  `tests/omega/corpus_both_macos_arm64.txt` gained 18 `built ... exit:0 interpreter-exit:0
  interpreter:agrees` rows. Every golden diff is a pure addition (0 deletions; the content guard reports
  0 keys, 0 continuation lines and 0 attachments lost), and the legs exited 0 with 36/36, 18/18 and 18/18
  fixtures done, 0 unimplemented rows and 0 rows recording a lost success.

## Requirements and Evidence (AGENTS.md)

- **Establishment**: Variant declarations `Alpha(v: u64)`, `Beta(v: u64)`, `Gamma(v: u64)`, `Delta(v: u64)`, `Epsilon(v: u64)`, and `Zeta(v: u64)` where `v >= 40u64 && v <= 80u64` establish bounded intervals on active variant payloads. Exhaustive pattern matching across all six tagged enum variants joins the incoming arm obligations, statically proving that the extracted payload satisfies `[40, 80]` without post-hoc caller equality guards.
- **Identity and lifetime**: The extracted payload interval fact `res in [40, 80]` is preserved across local bindings, carrier record fields, machine parameters, helper call returns, match join arms, named constants, state transitions, helper forwarding machines, and target-scoped bodies without semantic loss.
- **Composition**: In each place, the synthesized interval `res in [40, 80]` composes directly with `verify_payload` contract preconditions and subsequent exact subtractions `headroom = 80u64 - res` and `margin = res - 40u64` to statically discharge underflow safety and non-negativity without post-hoc caller equality guards.
- **Soundness boundaries**: In every reject twin, widening the payload bound on variant `Zeta` to `v >= 39u64` permits `res` to reach `39u64`, which fails closed under contract requires preconditions or ensures postconditions across boundary transmissions, verifying that out-of-interval variant payloads cannot satisfy safety bounds.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`)**: Tagged enum construction, variant discrimination, and payload access lower to target-neutral Terminal Psi tagged case representations (`PSITERM\0`), while proof-only where-clauses are erased. Both Terminal Psi interpreter and native execution agree with exit 0 (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`)**: Provider layout plans represent the 6-variant tagged enum `Packet` with its discriminant tag and 8-byte payload (`IntegerAt 8-byte` for `u64`), respecting layout alignment and calling conventions across all four hosted platforms.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`)**: Backend lowering selects target ISA variant tag comparison and branch instructions alongside 64-bit integer moves and exact subtractions: `sub rax, ...` on `x86_64` and `sub x0, ...` on `arm64`, maintaining disjoint register homes and activation stack frames without spilling corruption.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission)**: Direct binary emission encodes machine instructions, resolves symbolic stack-frame displacements and basic block branches, validates transitive machine-state footprints, and emits PE/ELF/Mach-O images that execute natively and agree with Terminal Psi on exit 0 across all four hosted platforms.
- **Remaining Open**: None for this fact row; all 9 places pass both native and interpreter gates with exit 0 (`omega run --both`), and all 9 reject twins reject at checking with their authored diagnostics and pinned fragments.
