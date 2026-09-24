# Checked instruction catalog

[Assembly](../../../../wiki/spec/language/assembly.md) owns the language contract.
[inline_assembly.rs](src/inline_assembly/mod.rs) is the closed shared source catalog:
instruction shape, availability, target, authority, ordered operand constraints,
ordering, flag data flow and realized clobbers. A catalog row is not proof that
every source-to-native consumer realizes it.

| Catalog family | Current structured operands and distinctions |
| --- | --- |
| `jmp`, `hlt` | State-target transition or halt; no local post-state. |
| `mov` / `movq` | Structured data move between a writable Omega place and a readable value; lowers to an ordinary checked assignment, so the copy's provenance, permission and exact-type obligations are the assignment's. Bracketed `[address]` operands keep refusing as unmodeled memory access; authorized memory data moves through a typed view index. |
| AArch64 `ldr` / `str` | Unordered memory transfers whose memory operand is a typed Omega place: `ldr <dest>, <place>` is `<dest> = <place>` and `str <value>, <place>` is `<place> = <value>` — the place carries the provenance, permission and exact-type contract a raw address cannot. Width-suffixed (`ldrb`/`strh`/...), offset/unscaled, ordered (acquire/release) and multi-register spellings are each a different contract and stay refused. |
| x86 `in` / `out` | Exact `u16` port at DX and `u8` value/destination at AL; only catalog-permitted fitting literals. |
| x86 `lfence` / `sfence` / `mfence` | Zero operands, load/store/full ordering; no service reach or general-register clobbers. |
| x86 `cli` / `sti` | MachineOwner requirement; immediate IF clear versus interrupt recognition after STI's following instruction. |
| x86 structured `pushfq` / `popfq` | Exact `u64` writable/saved place; snapshot or restore with balanced RSP. Restore requires MachineOwner; a literal is not a saved-flags place. |
| x86 `rdmsr` / `wrmsr` | Exact `u32` ECX selector and `u64` EDX:EAX value; explicit read destination; MachineOwner. |
| x86 control-register access | Exact `u64` places/values; CR0/2/3/4 reads, CR0/3/4 writes; MachineOwner. |
| AArch64 system-register access | Exact `u64` places/values via `read_<sysreg>`/`write_<sysreg>` (architecturally `mrs`/`msr`); EL1 control, translation and thread registers (SCTLR/TCR/TTBR0/TTBR1/MAIR/VBAR/TPIDR) plus read-only ESR_EL1/FAR_EL1; MachineOwner. Deferred-effect writes synchronize through a caller-sequenced `isb`. |
| x86 `serialize` / AArch64 `isb` | Zero operands; instruction-stream serialization — every prior instruction completes and instruction fetch re-synchronizes — not memory ordering; UserChecked, no authority, no clobbers. |
| x86 `pause` / AArch64 `yield` / `wfe` / `wfi` / `sev` / `sevl` / `nop` | Zero operands; scheduler/pipeline hint the core may elide — no semantic or machine-state obligation; UserChecked, no authority, no clobbers. `nop` is target-neutral; the AArch64 `wfe`/`wfi`/`sev`/`sevl` encodings complete the architectural hint set, each legally completable as a no-op. |
| `lidt`, `iretq` / `sysret` / `sysretq`, `eret` | Deriver-only; `lidt` has the consumer-authorized descriptor contract, not an arbitrary address operand. |
| x86 `serialize` / `pause`; aarch64 `isb` / `yield` / `wfe` / `wfi` / `sev` / `sevl`; `nop` | Zero-operand pipeline directives: instruction-stream serialization and scheduling hints carry no operand, no service reach and no clobber. |
| x86 `wbinvd` / `invd` / `wbnoinvd` | Zero-operand cache maintenance: serializing, MachineOwner, no modeled operand place. `invd` drops modified lines without writeback; `wbnoinvd` writes back without invalidating. Cache operations needing a memory operand (`invlpg`, `clflush`) stay refused until a modeled memory contract exists. |

The catalog's operand and clobber constants are authoritative for the current
realized sequence, including scratch loaders/stores. Do not copy their register
lists or encoded bytes into a language rule. General return/call/indirect-branch
spellings refuse as hidden exits — including the x86 near/far/operand-size
return spellings (`retn`/`retw`/`iret*`), far call/jump forms (`lcall`/`callf`/
`jmpf`/`ljmpl`), the AArch64 branch-consistent head, and the pointer-authenticated
branch/return spellings — so each fails for the semantic reason rather than as
unknown text. Recognized unmodeled loads/stores refuse for missing memory
contracts — the canonical unordered AArch64 pair `ldr`/`str` is the contracted
exception above, and the refused coverage spans the width/signed/unscaled/unprivileged grids,
non-temporal and signed pair forms, RCpc/limited-ordering acquire-release forms, the complete
exclusive/LSE ordering grid including the store-only (`st*`) aliases, the 64-byte block forms, NEON structure loads and
stores, x86 string/port-string bare and dword forms, stack and flag-store forms,
far-pointer loads, `xsave`/`fxsave` state families, descriptor-table memory
operands, memory-destination non-temporal stores, and `bound`. Spellings with a
register-only form (`movzx`, `cmpxchg`-free `bt*`, `smsw`, SSE's `movsd`/`cmpsd`
shadows) are deliberately absent — mnemonics are classified whole, so a partly
register-only spelling stays unrecognized rather than inheriting the memory
refusal. Register-only `mov`/`movq` is the decoded exception: it carries no
memory-addressing operand, so its contract is the ordinary assignment's.
Unknown mnemonics remain distinct failures, and service-admission candidates
(`svc`/`hvc`/`smc`/`brk`, `syscall`/`sysenter`/`sysexit`) plus address-arithmetic
(`lea`), ordering (`dmb`/`dsb`), and cache/TLB maintenance (`cl*`/`tlbi`/`ic`/`dc`)
families stay unrecognized pending their own contracts.
Target gates do not silently substitute another ISA's instruction.

[Parsing](../../pipeline/01_tokens-to-syntax-trees/src/bodies/statements/inline_assembly.rs) lowers
known forms and checks separators, transfer position and exact clobber union.
Requires/ensures become assertions around those instructions, with a proof-neutral
entry marker for an ensures-only block.
[Checked assertions](../../pipeline/typed-trees-to-checked-trees/src/checks/contracts/assembly.rs)
use the exact block-point flow facts and explicitly reject stale postconditions
whose places were written.

[Authority discharge](../../semantics/validation/src/machine_calls/effects/asm_discharge.rs)
checks each instruction's declared authority class against the build's supplied
`AsmAuthorityAdmission` evidence. The evaluated `Build.freestanding` selection
is received as machine-owner admission covering every defined class
(including the deriver-only `lidt`). The authored
`Build.privileged_services` flags grant each mediated class independently —
`port_io` admits port I/O without machine control and `interrupt_table`
admits interrupt-table publication without port I/O — while machine-owner
authority itself has no granular grant and stays `freestanding`-only;
consumer-defined publication authority stays receiver-side per
[privileged-service admission](../../../../wiki/spec/build/permissions.md#privileged-services).
A source gate, catalog row, parser
test or instruction encoder is not final-artifact or native execution evidence.

The [implicit freestanding entry plan](../../../omega/representations/calling-conventions/src/plans/mod.rs)
adds instruction-pointer, stack-pointer and control-state use to its ordinary
volatile-state ceiling. This compatibility path applies only to the compiler-
selected boot root; it must not widen an explicit source-selected boundary
StatePlan. That ceiling permits state use, not acquisition of missing authority.

Catalog expansion for atomics, cache/TLB operations and mode transitions must
extend the same contract/replay model; there is
no per-family alternate instruction language. Final realization participates in
[machine-state footprint replay](../../../../wiki/spec/build/machine_state_evidence.md).
