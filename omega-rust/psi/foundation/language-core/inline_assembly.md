# Checked instruction catalog

[Assembly](../../../../wiki/spec/language/assembly.md) defines the language.
[inline_assembly.rs](src/inline_assembly.rs) is the closed, shared catalog of
source instructions: shape, availability, target, authority, ordered operand
constraints, ordering, flag data flow and realized clobbers. A row does not
show that every stage from source to native code supports it.

The [interpreted execution contract](../../../../wiki/spec/language/assembly.md#interpreted-execution)
requires executable instruction meaning, not just footprint metadata. Use
ordinary value, place and transition operations where exact; otherwise keep
explicitly contracted environment effects. Scratch
register and flag state is local to a block; authorized memory and declared
machine or device state persist. Unsupported execution profiles are refused;
an encoder or a Unit-returning stub does not show interpretation. Adding an
instruction needs acceptance through every consumer, with pause/resume, effect
and refusal tests for the claimed profile.

| Catalog family | Current structured operands and distinctions |
| --- | --- |
| x86 `sub`, `jz`, `jnz` | Modeled u32 GPR subtraction establishes immutable arithmetic/ZF versions. Local `jz` and `jnz` edges consume opposite polarities of the same latest ZF version, retain both successor obligations and carry no clobber or authority of their own. |
| AArch64 `subs` | Three operands name an independent writable W-register destination, a W-register left input and a matching W-register or encodable immediate right input. Modular u32 subtraction writes the destination and establishes the zero condition for `b.eq`/`b.ne`, with NZCV and the destination's X-register view clobbered. The destination need not be initialized; inputs still must be. Other flag observations and wide/stack/link/zero aliases remain outside this cohort. |
| x86 / AArch64 `cmp` | Modeled u32 scratch-register comparison preserves both operands and establishes a private subtraction result and zero-flag version. x86 uses `jz`/`jnz`; AArch64 uses `b.eq`/`b.ne`, with opposite polarities on the same checked version. AArch64 scratch operands are W0–W28; SP/FP/LR, zero and wide aliases remain refused. |
| `jmp`, `hlt` | State-target transition or halt; no local post-state. |
| `mov` / `movq` | Structured data move between a writable Omega place and a readable value; lowers to an ordinary checked assignment, so the copy's provenance, permission and exact-type obligations are the assignment's. Bracketed `[address]` operands keep refusing as unmodeled memory access; authorized memory data moves through a typed view index. |
| AArch64 `ldr` / `str` | Unordered memory transfers whose memory operand is a typed Omega place: `ldr <dest>, <place>` is `<dest> = <place>` and `str <value>, <place>` is `<place> = <value>` — the place carries the provenance, permission and exact-type contract a raw address cannot. Width-suffixed (`ldrb`/`strh`/...), offset/unscaled, ordered (acquire/release) and multi-register spellings are each a different contract and stay refused. |
| x86 `in` / `out` | Exact `u16` port at DX and `u8` value/destination at AL; only catalog-permitted fitting literals. |
| x86 `lfence` / `sfence` / `mfence` | Zero operands, load/store/full ordering; no service reach or general-register clobbers. |
| AArch64 `dmb ld` / `dmb st` / `dmb sy` | Explicit full-system option fixes the access mask: LD orders prior reads before later reads/writes, ST orders writes, SY orders reads/writes. No runtime operands, authority or clobbers. Other scopes, reserved/numeric options and `dsb` remain unsupported. |
| x86 `cli` / `sti` | MachineOwner requirement; immediate IF clear versus interrupt recognition after STI's following instruction. |
| x86 structured `pushfq` / `popfq` | Exact `u64` writable/saved place; snapshot or restore with balanced RSP. Restore requires MachineOwner; a literal is not a saved-flags place. |
| x86 `rdmsr` / `wrmsr` | Exact `u32` ECX selector and `u64` EDX:EAX value; explicit read destination; MachineOwner. |
| x86 `sgdt` / `lgdt` | Store/read exactly ten plain bytes through `&write [u8; 10]` (or `&mut`) / `&[u8; 10]` (or `&mut`). Both require MachineOwner and the catalog's R10 scratch clobber. Numeric addresses and arbitrary equal-sized records supply no operand authority. |
| x86 control-register access | Exact `u64` places/values; CR0/2/3/4 reads, CR0/3/4 writes; MachineOwner. |
| AArch64 system-register access | Exact `u64` places/values via `read_<sysreg>`/`write_<sysreg>` (architecturally `mrs`/`msr`); EL1 control, translation and thread registers (SCTLR/TCR/TTBR0/TTBR1/MAIR/VBAR/TPIDR) plus read-only ESR_EL1/FAR_EL1; MachineOwner. Deferred-effect writes synchronize through a caller-sequenced `isb`. |
| x86 `serialize` / AArch64 `isb` | Zero operands; instruction-stream serialization — every prior instruction completes and instruction fetch re-synchronizes — not memory ordering; UserChecked, no authority, no clobbers. |
| x86 `pause` / AArch64 `yield` / `wfe` / `wfi` / `sev` / `sevl` / `nop` | Zero operands; scheduler/pipeline hint the core may elide — no semantic or machine-state obligation; UserChecked, no authority, no clobbers. `nop` is target-neutral; the AArch64 `wfe`/`wfi`/`sev`/`sevl` encodings complete the architectural hint set, each legally completable as a no-op. |
| `lidt`, `iretq` / `sysret` / `sysretq`, `eret` | Deriver-only; `lidt` has the consumer-authorized descriptor contract, not an arbitrary address operand. |
| x86 `serialize` / `pause`; aarch64 `isb` / `yield` / `wfe` / `wfi` / `sev` / `sevl`; `nop` | Zero-operand pipeline directives: instruction-stream serialization and scheduling hints carry no operand, no service reach and no clobber. |
| x86 `wbinvd` / `invd` / `wbnoinvd` | Zero-operand cache maintenance: serializing, MachineOwner, no modeled operand place. `invd` drops modified lines without writeback; `wbnoinvd` writes back without invalidating. Cache operations needing a memory operand (`invlpg`, `clflush`) stay refused until a modeled memory contract exists. |

The catalog's operand and clobber constants govern the current emitted
sequence, including scratch loads and stores. Do not copy their register
lists or encoded bytes into a language rule.

These recognized spellings are refused for their real reason, not as unknown
text:
- General return, call and indirect-branch spellings are refused as hidden
  exits. This includes x86 near/far/operand-size returns
  (`retn`/`retw`/`iret*`), far calls and jumps (`lcall`/`callf`/`jmpf`/`ljmpl`),
  the AArch64 branch-consistent head, and pointer-authenticated branches and
  returns.
- Unmodeled loads and stores are refused for lacking a memory contract (the
  unordered AArch64 `ldr`/`str` above is the exception). This covers the width/signed/unscaled/unprivileged grids,
  non-temporal and signed pair forms, RCpc/limited-ordering acquire-release
  forms, the full exclusive/LSE ordering grid including the store-only (`st*`)
  aliases, the 64-byte block forms, NEON structure loads and stores, x86
  string/port-string bare and dword forms, stack and flag-store forms,
  far-pointer loads, the `xsave`/`fxsave` families, other descriptor-table
  memory operands, memory-destination non-temporal stores, and `bound`.

Mnemonics are classified whole, so spellings with a register-only form
(`movzx`, `cmpxchg`-free `bt*`, `smsw`, SSE's `movsd`/`cmpsd` shadows) are left
out and stay unrecognized rather than inheriting the memory refusal.
Register-only `mov`/`movq` is the decoded exception: with no memory operand, it
follows the ordinary assignment's contract.
Unknown mnemonics fail separately. Service-admission candidates
(`svc`/`hvc`/`smc`/`brk`, `syscall`/`sysenter`/`sysexit`), address arithmetic
(`lea`), ordering (`dsb` and unsupported `dmb` scopes) and cache/TLB maintenance
(`cl*`/`tlbi`/`ic`/`dc`) stay unrecognized until each has its own contract.
Target gates never substitute another ISA's instruction.

[Parsing](../../compilation/pipeline/01_tokens-to-syntax-trees/assembly/parse.rs)
lowers known forms and checks separators, transfer position and the exact
clobber union. Requires and ensures clauses become assertions around the
instructions, with a proof-neutral entry marker for an ensures-only block.
Checked assertions use exact block-point flow facts and refuse stale
postconditions whose places were written.

[Authority discharge](../../compilation/validation/src/machine_calls_core.rs)
checks each instruction's declared authority class against the build's
`AsmAuthorityAdmission` evidence. The evaluated `Build.freestanding` selection
admits machine ownership for every defined class (including deriver-only `lidt`). The authored `Build.privileged_services` flags grant each
mediated class on its own: `port_io` allows port I/O without machine control,
and `interrupt_table` allows publishing an interrupt table without port I/O.
Machine-owner authority has no finer grant and comes only from `freestanding`;
publication authority a consumer defines stays with the receiver, per
[privileged-service admission](../../../../wiki/spec/build/permissions.md#privileged-services).
A source gate, catalog row, parser test or instruction encoder is not evidence
for the final artifact or native execution.

The [implicit freestanding entry plan](../../../omega/compilation/backend/calling-conventions/src/plans.rs)
adds instruction-pointer, stack-pointer and control-state use to its ordinary
limit on volatile state. This compatibility path applies only to the
compiler-selected boot root; it must not widen an explicit source-selected
boundary StatePlan. The limit permits using state, not gaining missing authority.

Adding atomics, cache/TLB operations and mode transitions to the catalog must
extend the same contract and replay model; there is no separate instruction
language per family. Final realization joins
[machine-state footprint replay](../../../../wiki/spec/build/machine_state_evidence.md).

Memory fences keep their exact kind through the shared processor-event type,
the canonical Terminal wire format and the native selected forms. Receiving
roots must explicitly supply the interpreter's private sequential memory model;
it has no pending, shared or device accesses and grants no synchronization. The interpreter records each completed catalog event in order, across
helpers and work-budget pauses. Native encoders independently decode the exact
x86 fence or full-system A64 DMB form; full atomic ordering cannot stand in for
a partial catalog fence. The `checked_asm_fence_contracts` sample tests source
transport, target selection, missing supply, event traces and pause progress.
It claims nothing about hardware timing or multi-core or device correctness.

`sgdt` replaces ten existing bytes with unknown valid `u8` values; it grants no
permission over uninitialized storage and keeps no earlier facts about those
bytes. `lgdt` reads them without changing caller storage. Their
ordinary borrow operands keep projection evaluation, temporary references and
cleanup. Receiving Terminal validation repeats the size, access and
protected-content checks independently.

Loading GDTR does not reload segment registers, load a TSS, install interrupt
roots or grant pointer authority from the stored address. The MachineOwner
caller owns the processor-state contract and the lifetime of the backing bytes.
The Terminal interpreter has no GDTR model and refuses these operations.
Provider-only `lidt` is unchanged.
