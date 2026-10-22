# Checked asm flag restore

Structured popfq restores a saved u64 flags place, and the following label reads that place. `sub` establishes ZF, `pushfq` stores bit 6, `popfq` restores it, and `observed requires saved & 64 == 64`.

Owning item: **CHECKED-ASM-LABEL-CONTRACTS**. The nine places bind `linux_x86_64`, `windows_x86_64`, and `macos_x86_64`. On Windows x86_64 both engines exit 0. Each reject twin uses a literal flags operand and rejects because that literal is outside the saved-flags u64 operand class. The pins are `tests/omega/pass/facts/checked_asm_flag_restore_through_<place>_exit` and `tests/omega/fail/facts/checked_asm_flag_restore_through_<place>_rejected`.
