# asm_sysregs

First sample with structured AArch64 system-register access:
`read_sctlr_el1`/`write_sctlr_el1`/`read_tcr_el1`/ttbr0/1/
mair/vbar/tpidr/esr/far — exact-u64 value flow, declared
`clobbers x9, x10, x11, x15` scratch set, `reaches
MachineControl`.

Compile-checked only: freestanding target — run a check
target, not native.
