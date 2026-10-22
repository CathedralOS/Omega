# Checked assembly rank decrease

sub records the decreased register, and the backedge reads that record.

The seed-2 mov/sub/jnz countdown is unchanged. This machine adds the ranked
decrease `terminates by eax -> Nat::Descending`. The countdown fact's label
contracts `loop requires eax > 0` and `done requires eax == 0` are ordinary
edge obligations. They are not this termination promise.

Owning item: **CHECKED-ASM-LABEL-CONTRACTS**. The nine places bind
`linux_x86_64`, `windows_x86_64`, and `macos_x86_64`. On Windows x86_64 both
engines exit 0. Each reject twin changes only `done requires eax == 0` to
`done requires eax == 1`. The pins are
`tests/omega/pass/facts/checked_asm_rank_decrease_through_<place>_exit` and
`tests/omega/fail/facts/checked_asm_rank_decrease_through_<place>_rejected`.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) |
| other state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) |
| other machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) |
| target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) |
