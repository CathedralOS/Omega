# Checked assembly

`asm { ... }` is parsed for the selected target, not an opaque text or byte
escape. Every accepted instruction has a compiler-owned contract; unknown
instructions and raw emitted bytes reject. A prebuilt blob is a foreign provider
artifact under admission, not inline assembly. There is no compatibility
`Binding::Instruction` source form.

## Blocks and assertions

Blocks contain at least one known instruction. Multiple instructions use `;`
separators; newlines are not grammar. A control transfer must be the last
instruction. The optional `where` surface permits `requires`, `ensures` and an
exact register-clobber declaration:

```omega
asm where
    requires self.ready
    clobbers none
    ensures self.ready
{
    lfence; sfence
}
```

`requires` is proved at block entry. `ensures` is proved at a falling-through
exit after instruction writes invalidate stale facts. These are checked
assertions, not admissions or extra facts. Non-fallthrough `hlt` and `jmp`
blocks cannot have local postconditions. Neither clause overrides the catalog.

An authored clobber set must equal the union of the realized instruction
contracts' changed registers. Omitted changes and invented changes both reject;
order and duplicates have no meaning. An explicitly empty set is `clobbers none`.
Omitting the clause never omits the catalog's effects.

## Instruction obligations

The complete contract describes operand types/widths and target features;
address provenance, bounds, alignment, initialization and permission; service
reach and authority; register, flag, memory and machine-state changes; ordering,
atomicity, cache/TLB effects; required/established regimes; and all control exits.
Exact operand constraints do not silently narrow wider integer places.

Each requirement must be discharged from checked facts and held authority or
through the permitted admission route. Moving an instruction into a helper
cannot remove its requirements. Direct assembly and a boundary operation for
the same mechanism contribute the same normalized reach and authority demand.
Assembly does not automatically make its enclosing machine a provider.

A checked block may separately establish a conformance for callers when its
instruction model proves it. Permitted admitted evidence remains identified in
receipts and reports. Admission never leaves an emitted obligation open.

## Control flow and machine state

Catalog availability distinguishes user-checked instructions from deriver-only
entry/exit operations. Known jumps map to declared Omega states and checked
transitions, retaining arguments, invariants and live obligations. Hidden labels,
loops, returns, unwinds and unmodeled exits reject. Return-from-interrupt is not
a way to bypass an entry contract. Direct targets remain final-artifact-validated;
indirect calls/tail calls require sealed requirement-compatible entry references,
not numeric addresses.

Return and continuation state is compiler-owned, not ordinary addressable data.
Memory safety and [stack provisioning](../resources/storage.md#compiler-owned-stack-accesses)
protect that state under their contracts. Every stack/control mutation must be
modeled; an instruction unable to preserve a modeled exit is deriver-only or
rejected. Forward-edge entry identity is an independent obligation.

Regime-changing instructions require their prior regime and establish the next.
A mode transition is not an exotic calling convention. The complete boundary
`StatePlan` constrains transitive use after optimization, allocation and callees;
source appearance cannot prove SIMD absence. [Final machine-state evidence](../build/machine_state_evidence.md)
checks the actual artifact and exact return-control/restored-state set. Opaque
execution requires the admitted plan evidence or adequate isolation under a
root-reported receipt. Missing or plan-drifted evidence rejects.

Floating instructions require canonical semantic controls, excluding sticky
status flags. Assembly changing them must establish the matching state transition
and restore the [canonical configuration](numeric_values.md#target-realization-and-control-state)
before ordinary checked code resumes. Independent final-byte transfer certificates
and CET/PAC/shadow-stack hardening remain separate proof/trust-reduction work,
not additional source properties.

## Hardware memory

An integer address grants no memory authority. Loads/stores require an authorized
extent/view or a specific provider grant. Plan-derived MMIO access follows the
same rules. Cache maintenance, TLB invalidation/shootdown, DMA completion and
cross-core instruction-fetch visibility require their target-specific contracted
sequences. Executable mapping additionally requires admitted-artifact provenance,
scoped installation authority and reach; ordinary bytes cannot supply them.

[Hardware materialization](../build/hardware_materialization.md) separates checked
writers, consumer validation and publication. The instruction catalog owns `lidt`
requirements, not an OS's table carrier or lifecycle. Current catalog coverage
and source realization limits live [beside the catalog](../../../omega-rust/psi/foundation/language-core/inline_assembly.md).
