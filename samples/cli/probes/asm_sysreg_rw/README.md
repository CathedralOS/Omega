# asm_sysreg_rw

EL1 system-register access uses the structured `read_<sysreg>` /
`write_<sysreg>` spellings — `read_sctlr_el1`, `write_tcr_el1`,
`read_ttbr0_el1`, `read_esr_el1`, `read_far_el1` — with exact-u64 value
flow and a declared scratch-clobber set.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
