# Checked instruction catalog

[Assembly](../../../../wiki/spec/language/assembly.md) owns the language contract.
[inline_assembly.rs](src/inline_assembly.rs) is the closed shared source catalog:
instruction shape, availability, target, authority, ordered operand constraints,
ordering, flag data flow and realized clobbers. A catalog row is not proof that
every source-to-native consumer realizes it.

| Catalog family | Current structured operands and distinctions |
| --- | --- |
| `jmp`, `hlt` | State-target transition or halt; no local post-state. |
| x86 `in` / `out` | Exact `u16` port at DX and `u8` value/destination at AL; only catalog-permitted fitting literals. |
| x86 `lfence` / `sfence` / `mfence` | Zero operands, load/store/full ordering; no service reach or general-register clobbers. |
| x86 `cli` / `sti` | MachineOwner requirement; immediate IF clear versus interrupt recognition after STI's following instruction. |
| x86 structured `pushfq` / `popfq` | Exact `u64` writable/saved place; snapshot or restore with balanced RSP. Restore requires MachineOwner; a literal is not a saved-flags place. |
| x86 `rdmsr` / `wrmsr` | Exact `u32` ECX selector and `u64` EDX:EAX value; explicit read destination; MachineOwner. |
| x86 control-register access | Exact `u64` places/values; CR0/2/3/4 reads, CR0/3/4 writes; MachineOwner. |
| `lidt`, `iretq` / `sysret` / `sysretq`, `eret` | Deriver-only; `lidt` has the consumer-authorized descriptor contract, not an arbitrary address operand. |

The catalog's operand and clobber constants are authoritative for the current
realized sequence, including scratch loaders/stores. Do not copy their register
lists or encoded bytes into a language rule. General return/call/indirect-branch
spellings currently refuse as hidden exits; recognized unmodeled loads/stores
refuse for missing memory contracts. Unknown mnemonics remain distinct failures.
Target gates do not silently substitute another ISA's instruction.

[Parsing](../../pipeline/tokens-to-syntax-trees/src/parser/statement.rs) lowers
known forms and checks separators, transfer position and exact clobber union.
Requires/ensures become assertions around those instructions, with a proof-neutral
entry marker for an ensures-only block.
[Checked assertions](../../pipeline/typed-trees-to-checked-trees/src/checks/contracts/assembly.rs)
use the exact block-point flow facts and explicitly reject stale postconditions
whose places were written.

[Authority discharge](../../semantics/validation/src/effects/asm_discharge.rs)
currently has a coarse target-class gate: hosted code rejects authority-bearing
assembly, while freestanding selection passes that gate. It is not proof of a
concrete machine-control capability. Full [privileged-service admission](../../../../wiki/spec/build/permissions.md#privileged-services)
remains required by the language contract. A source gate, catalog row, parser
test or instruction encoder is not final-artifact or native execution evidence.

The [implicit freestanding entry plan](../../../omega/representations/calling-conventions/src/plans.rs)
adds instruction-pointer, stack-pointer and control-state use to its ordinary
volatile-state ceiling. This compatibility path applies only to the compiler-
selected boot root; it must not widen an explicit source-selected boundary
StatePlan. That ceiling permits state use, not acquisition of missing authority.

Catalog expansion for atomics, cache/TLB operations, mode transitions and
AArch64 system operations must extend the same contract/replay model; there is
no per-family alternate instruction language. Final realization participates in
[machine-state footprint replay](../../../../wiki/spec/build/machine_state_evidence.md).
