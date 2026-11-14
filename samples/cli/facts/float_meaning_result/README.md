# Captured float meaning result

The minimum call's checked IEEE meaning survives transport and discharges `require_min` for its exact captured result.

This source migration supports the explicit identity requirement in **PROOF-KERNEL-CORE** in `TASKS.md`. `forward` returns `F32::minimum(left, right)` and states that the result's `FloatMeaning` is identical to the specified minimum's meaning. The caller transports the result and uses that identity in `require_min`. Its runtime check still uses ordinary IEEE comparison.

Every invalid twin changes only `F32::minimum` to `F32::maximum`, retaining the minimum guarantee and consumer. It must reject because `forward` cannot prove that guarantee. Identity of a `FloatMeaning` does not assert identity of the underlying float representation or change IEEE comparison behavior.

| Place | Positive | Invalid twin |
| --- | --- | --- |
| local | [local/main.omg](local/main.omg) | [invalid_local/main.omg](invalid_local/main.omg) |
| field | [field/main.omg](field/main.omg) | [invalid_field/main.omg](invalid_field/main.omg) |
| parameter | [parameter/main.omg](parameter/main.omg) | [invalid_parameter/main.omg](invalid_parameter/main.omg) |
| call_result | [call_result/main.omg](call_result/main.omg) | [invalid_call_result/main.omg](invalid_call_result/main.omg) |
| match_join | [match_join/main.omg](match_join/main.omg) | [invalid_match_join/main.omg](invalid_match_join/main.omg) |
| const_name | [const_name/main.omg](const_name/main.omg) | [invalid_const_name/main.omg](invalid_const_name/main.omg) |
| other_state | [other_state/main.omg](other_state/main.omg) | [invalid_other_state/main.omg](invalid_other_state/main.omg) |
| other_machine | [other_machine/main.omg](other_machine/main.omg) | [invalid_other_machine/main.omg](invalid_other_machine/main.omg) |
| target_body | [target_body/main.omg](target_body/main.omg) | [invalid_target_body/main.omg](invalid_target_body/main.omg) |

All positive projects bind `ProgramEntry` for all four hosted targets. Qualification requires native and Terminal Psi exit `0` on each host; the runtime check expects `1.0`. The nine positive fixtures under `tests/omega/pass/facts/float_meaning_result_through_*` retain byte-identical source and Build declarations. The nine invalid samples above exist, but matching `tests/omega/fail/facts/` mirrors are absent.

The checks must establish:

- The selected operation's checked return contract supplies the identity, and `forward` proves it for its original operands.
- The identity names the exact captured result and argument storage versions. Writes retain their ordinary invalidation rules.
- All nine places preserve the same fact; `require_min` consumes it without a new guard establishing it.
- Interpreter and native execution compute and transport the f32 result through the selected target's calling and storage rules, and satisfy the authored result check.

The migration preserves the existing programs and does not establish new execution results. The `match_join` program currently invokes only `joined(true)` and has identical arm bodies, so its oracle does not distinguish branch selection. That coverage gap and the missing invalid corpus mirrors remain open; this change closes no row or parent acceptance.

## Build-owned target-body migration

The `target_body` place and its twin carry no source target prefix.
Ordinary named implementations (`X86...` and `Arm...`) of a boundary-trait
requirement are selected by explicit target-keyed Build rows. The entry wrapper
and each implementation check a tag (17 for x86-64, 29 for AArch64), so a wrong
selection traps. These Build declarations, inputs and exit oracles are unchanged.
