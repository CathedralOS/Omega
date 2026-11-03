# Checked asm flag restore

Structured popfq restores a saved u64 flags place, and the following label reads that place. `sub` establishes ZF, `pushfq` stores bit 6, `popfq` restores it, and `observed requires saved & 64 == 64`.

Owning item: **CHECKED-ASM-LABEL-CONTRACTS**. The nine places bind `linux_x86_64`, `windows_x86_64`, and `macos_x86_64`. On Windows x86_64 both engines exit 0. Each reject twin uses a literal flags operand and rejects because that literal is outside the saved-flags u64 operand class. The pins are `tests/omega/pass/facts/checked_asm_flag_restore_through_<place>_exit` and `tests/omega/fail/facts/checked_asm_flag_restore_through_<place>_rejected`.

## Build-owned target-body migration

The `target_body` place carries no source target prefix. Its three x86-64
prefixed `restore` bodies became the requirement `SelectedRestore::restore`
with one ordinary implementation, `X86Restore`, selected for `linux_x86_64`,
`windows_x86_64` and `macos_x86_64` by explicit `Build::select_provider` rows
(`BUILD-OWNED-TARGET-SELECTION`). The assembly, its `MachineControl` reach,
the crash route and the exit oracle are unchanged; no AArch64 body exists, so
no tag check is needed. Observed with the optimized producer at `11bdd2fe41f`
on macOS x86-64: the positive exits 0 under `omega run --both` (native and
Terminal Psi agree), and the twin still rejects because integer literal `0` is
outside the saved-flags operand class. No compiler change was needed. The
other hosted targets were not executed here.
