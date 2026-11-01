# Checked asm view move

An x86 mov through a typed view index copies the indexed element by the place's ordinary assignment. The bracket twin breaks only that operand.

Owning item: **CHECKED-ASM-LABEL-CONTRACTS**. The nine places bind `linux_x86_64`, `windows_x86_64`, and `macos_x86_64`. On Windows x86_64 both engines exit 0, and each bracket twin rejects as unmodeled memory. The pins are `tests/omega/pass/facts/checked_asm_view_move_through_<place>_exit` and `tests/omega/fail/facts/checked_asm_view_move_through_<place>_rejected`.

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
