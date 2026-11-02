# asm_msr_multi

First sample with a multi-service reach (`reaches
MachineControl + PortIo`) and MSR access: `rdmsr
main.result, main.index` / `wrmsr main.index, main.value`
beside `out`/`in` in one `asm clobbers` block.

Compile-checked only: freestanding target — run a check
target, not native.
