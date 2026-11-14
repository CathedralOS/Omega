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
| local | [`local/main.omg`](local/main.omg) | [`invalid_local/main.omg`](invalid_local/main.omg) |
| field | [`field/main.omg`](field/main.omg) | [`invalid_field/main.omg`](invalid_field/main.omg) |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`invalid_parameter/main.omg`](invalid_parameter/main.omg) |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`invalid_call_result/main.omg`](invalid_call_result/main.omg) |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`invalid_match_join/main.omg`](invalid_match_join/main.omg) |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`invalid_const_name/main.omg`](invalid_const_name/main.omg) |
| other state | [`other_state/main.omg`](other_state/main.omg) | [`invalid_other_state/main.omg`](invalid_other_state/main.omg) |
| other machine | [`other_machine/main.omg`](other_machine/main.omg) | [`invalid_other_machine/main.omg`](invalid_other_machine/main.omg) |
| target body | [`target_body/main.omg`](target_body/main.omg) | [`invalid_target_body/main.omg`](invalid_target_body/main.omg) |

## Build-owned target-body migration

The `target_body` place and its twin carry no source target prefix.
`BUILD-OWNED-TARGET-SELECTION` replaces the prefixed family with ordinary named
implementations (`X86...` and `Arm...`) of a boundary-trait requirement, selected
by explicit target-keyed Build rows. The operation, inputs, contracts and exit
oracle are unchanged. The entry wrapper and each implementation check a tag
(17 for x86-64, 29 for AArch64), so a wrong Build row traps. Observed evidence is
Linux x86-64 native and Terminal Psi agreement for the positive and source
rejection for the twin; the other hosted targets keep their authored rows,
unexecuted here.
