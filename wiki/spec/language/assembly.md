# Checked assembly

`asm { ... }` is parsed for the selected target, not an opaque text or byte
escape. Every accepted instruction has a compiler-owned contract; unknown
instructions and raw emitted bytes reject. A prebuilt blob is a foreign provider
artifact under admission, not inline assembly. There is no compatibility
`ForeignBinding::Instruction` source form.

Under [embedded interpretation](../build/embedding.md), supported instructions
execute their catalog-defined semantics against ordinary values/storage and an
explicitly supplied target environment. Assembly remains valid source; neither
an authored alternate body nor a boundary wrapper per block is required. This
does not require a whole-host emulator or grant execution support to every
catalogued instruction. See [interpreted execution](#interpreted-execution).

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

## Block-local state and explicit communication

A block has no implicit general-register or scratch-flag inputs from surrounding
compiled code or another block. Such inputs must come from its declared operands
or a catalog-defined state observation; reads within the block must follow the
catalog's defined dataflow. Register allocation and incidental preservation are
not source contracts. Clobbers account for changes; they neither supply initial
values nor make scratch state a persistent communication channel.

Communication between blocks uses ordinary Omega values/places, authorized
memory effects, or explicitly contracted environment state. For example:

```omega
asm { mov saved, input }
asm { mov output, saved }
```

Here `saved` is an ordinary typed place, not a retained register. Conversely,
interrupt-mask changes, device writes and memory updates do not reset when a
block exits. System registers and architectural control flags explicitly read
or changed by catalog operations belong to that persistent state, not scratch.
Its lifetime and sharing follow the selected execution environment and held
authority. Blocks are not transactions, isolation domains, or rollback points.

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

An [assumed binding contract](../build/assumed_binding_contracts.md) can supply
explicitly admitted premises at an authorized runtime crossing. It cannot exempt
an assembly block from the instruction catalog, reinterpret `asm where` as an
assumption, admit raw emitted bytes, or suppress instruction obligations. The
checked body consumes authority under its own contract, just like non-assembly
code; the potentially false relationship belongs to the root-selected boundary.

## Interpreted execution

Interpret the checked operations, not native instruction bytes or source text
reparsed at runtime. Reuse ordinary Terminal value, place and transition
operations where their full semantics suffice. Otherwise retain the exact
catalog operation, target/features, inputs/outputs, authority, state effects and
exits through the ordinary pipeline. No second asm-only interpreter pipeline or
opcode per spelling is required. A clobber/effect summary alone is not executable
semantics: each supported operation needs its actual result and state transition,
including the catalog's permitted nondeterminism and faults.

The block's observable transition is:

```text
explicit inputs + authorized storage + declared environment state
    -> outputs + storage updates + environment updates + checked exit
```

| Operation kind | Interpreted realization |
| --- | --- |
| Value moves and computation | Ordinary checked value/place operations with exact target semantics. |
| Scratch registers and flags | Block-local state only where needed to realize the operation. |
| Memory access | Existing authorized storage/views, with the same bounds, layout and custody. |
| Port I/O, system state, interrupts and ordering | Operations on the explicitly supplied target environment under the catalog contract. |
| Control transfer | Existing checked transitions or the exact modeled external-control exit, never arbitrary native jumps. |

The build proposes the target environment and its providers; the receiving host
admits and supplies their scoped state and authority. Use ordinary checked
interfaces/bindings for external mechanisms, not an author-supplied callback
that may redefine a mnemonic. The compiler owns instruction meaning and its
realization obligations; environment packages own device/state implementations;
Cathedral owns OS structures, drivers, scheduling and boot/update policy.
No single universal CPU/device object or complete ISA emulation is required.
Provide the state needed by the supported operations, with explicit sharing
between execution contexts where required.

Modeled state is not the host machine's state. Machine-owner authority for a
modeled processor cannot disable host interrupts or acquire host physical
memory. Live hardware access needs its separately admitted provider and actual
host authority. Simulation establishes behavior under its stated model, not
native timing or hardware correctness. Ordinary provider evidence/trust policy
applies; a successful callback or matching signature proves no correspondence.

Preserve persistent effects across blocks, helpers, normal code and fuel pauses.
Interrupt recognition delays, synchronization, ordering and external-entry
conditions follow the exact target contract; a source statement or interpreter
step is not automatically one native instruction. Halt is the modeled processor
state/control effect, not interpreter fuel exhaustion or a successful Unit return.
Resume/external entry follows its contracted route and retains installation,
loan and acknowledgement custody. If a fuel pause occurs inside a block, retain
its live scratch and progress too; completed instructions/effects are not replayed.
External events may occur only at the model's admitted points, not merely because
control returned to the host. A block is not implicitly atomic.

Before starting a selected executable closure, validate its demanded operation
coverage and required environment supply, including callees/providers. Repeat
that admission for dynamically installed components. Missing semantics or an
unsupported environment rejects explicitly, never as a fabricated zero, Unit
success, skipped effect or implicit native/JIT fallback. A no-op is valid only
where the catalog permits it and all remaining obligations are preserved.
Native-only availability is not interpreted support. Ordinary authored provider
alternatives remain possible; they are not mandatory replacements for asm.

Interpreted address identities are not a byte-addressed physical memory model.
Operations requiring address translation or physical geometry need an explicit
admitted mapping; never reinterpret an entry/storage ID as a host pointer.
Scratch isolation does not remove that requirement.

Interpretability alone grants no compile-time execution permission. Pure
operations remain subject to [semantic-evaluation admission](evaluation.md);
build/test environments retain their existing authority limits. Runtime
[assumed binding contracts](../build/assumed_binding_contracts.md#runtime-only)
neither supply missing instruction semantics nor become build-admissible.

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
