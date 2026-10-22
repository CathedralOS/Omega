# Crash-entry IEEE representation views

Owning requirement: **CRASH-CONTRACT**, entry-operand normalization.

Fact: a checked view of an entry `i32` equal to `1065353216`, read as `f32`, discharges `maybe_zero`'s `value == 0.0` `Trap` crash guard without a caller crash ceiling.

The bit pattern is the one pinned by `tests/omega/pass/recast/i32_float_bits_view_compile`: `1065353216` is `0x3F800000`, which reads as `1.0`. The view records that IEEE value. The crash guard compares it with `0.0`. `1.0 == 0.0` is false, so the Trap route drops. An entry integer proved equal to `0` reads as `+0.0`, so the route stays live. A literal actual is delivered as the callee's entry scalar, so the omit-proof twin passes the same bits through a call that publishes no value, and the field twin omits only the field proof. Both leave the Trap route uncovered.

`read_bits` publishes no crash ceiling. Each positive binds `ProgramEntry` for `windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64`.

## Nine-Place Coverage

| Place | Positive witness | Omit-proof twin | Zero-bit twin |
| --- | --- | --- | --- |
| Local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) | [local/zero_bits/main.omg](local/zero_bits/main.omg) |
| Field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) | [field/zero_bits/main.omg](field/zero_bits/main.omg) |
| Parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) | [parameter/zero_bits/main.omg](parameter/zero_bits/main.omg) |
| Call result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) | [call_result/zero_bits/main.omg](call_result/zero_bits/main.omg) |
| Match join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) | [match_join/zero_bits/main.omg](match_join/zero_bits/main.omg) |
| Const name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) | [const_name/zero_bits/main.omg](const_name/zero_bits/main.omg) |
| Other state | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) | [other_state/zero_bits/main.omg](other_state/zero_bits/main.omg) |
| Other machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) | [other_machine/zero_bits/main.omg](other_machine/zero_bits/main.omg) |
| Target body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) | [target_body/zero_bits/main.omg](target_body/zero_bits/main.omg) |

The call-result place returns the viewed `f32` from a machine whose body is that parameter. It does not use `ensures result == value`, because IEEE equality is not reflexive at NaN.

Pinned corpus fixtures are `pass/facts/crash_entry_ieee_view_<place>_exit`, `fail/facts/crash_entry_ieee_view_<place>_rejected`, and `fail/facts/crash_entry_ieee_view_<place>_zero_rejected`.

## Requirements and Evidence

- **Establishment:** `requires value == 1065353216` (or `packet.bits` / `ONE_BITS` / the successor state's same equality) establishes the entry integer before `&value as &f32` observes it.
- **Identity and lifetime:** the view records the IEEE value of the entry integer's bits. The crash guard reads that record.
- **Composition:** the same sentence is the local copy, the `Packet.bits` field, the direct parameter, the returned parameter, the match join, the const name, the successor state, the forwarding machine, and the four target-qualified bodies.
- **Soundness boundaries:** the recast reinterprets bits. It does not convert the integer numerically. A missing proof or a zero bit pattern keeps the Trap route.
- **Remaining open under `CRASH-CONTRACT`:** nonempty-path lengths and policy casts remain on `TASKS.md`.
