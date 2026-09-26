# Chapter 22: Inline Assembly

Assembly is a checked low-level operation surface, not an escape from Omega's
ownership, authority, effects or control flow. The [assembly specification](../spec/language/assembly.md)
defines the contract; the [instruction catalog note](../../omega-rust/psi/foundation/language-core/inline_assembly.md)
records current source and target coverage.

## Parsed instructions with contracts

Each accepted instruction has structured operands and a compiler-owned contract.
For x86 port output, for example:

```omega
asm {
    out self.port, self.value
}
```

The port is exactly `u16`, the byte value exactly `u8`; a wider place does not
narrow implicitly. Required authority and service reach remain obligations of
the enclosing machine. Several instructions use explicit `;` separators.

A block may state assertions and its exact realized register footprint:

```omega
asm where
    requires self.ready
    clobbers none
    ensures self.ready
{
    lfence; sfence
}
```

Here the x86 fences change ordering, not general-purpose registers. `requires`
is proved on entry and `ensures` on fallthrough after invalidating stale facts.
Neither clause manufactures evidence or overrides an instruction contract.
An authored clobber set must be exact: both omitted and invented changes reject.
`hlt` and `jmp` blocks have no local post-state for `ensures`.

## Blocks do not share hidden registers

Separate blocks cannot communicate through whatever a previous block left in
a general register or scratch flag. Inputs and outputs use ordinary typed places:

```omega
asm { mov saved, input }
asm { mov output, saved }
```

Here `saved` carries the value, not incidental register preservation. Each
block accounts for its own scratch dataflow and clobbers. An explicit catalog
operation may observe architectural state, but it cannot invent a guarantee
about surrounding compiler scratch.

This does not isolate side effects: memory writes, device commands and changes
to interrupt state persist. For example, interrupt masking can cover ordinary
Omega code between assembly blocks; restoring the saved state remains an
explicit obligation. A block is not an atomic transaction or rollback scope.

## No quiet spelling

Direct assembly cannot hide a mechanism's service reach or authority by avoiding
a wrapper. A helper containing `wrmsr` still owes machine-control authority.
Conversely, a machine remains checked when its complete instruction obligations
are proved; assembly does not automatically make it an opaque provider.

High-level interrupt APIs normally use an opaque linear guard: save and mask,
perform checked work, then consume the guard to restore the prior state. The
instruction supplies register/state behavior, not special-purpose linearity or
permission for callers to reconstruct saved-state tokens. The
[interrupt contract](../spec/build/interrupt_obligations.md#mask-and-acknowledgement-custody)
owns the guard's opaque representation, routed qualification and exact restore.

Unknown instructions and raw bytes reject. Admitted prebuilt code uses the
ordinary foreign-provider path.

## Control flow remains Omega control flow

```omega
asm {
    jmp next_state()
}
```

This is a checked transition: its target, arguments, invariants and live
obligations remain visible. Control transfer must be the final instruction.
Hidden labels, loops, returns and unwinds are not another control-flow system.

Some catalog operations are user-checked; entry/exit operations such as
return-from-interrupt are deriver-only. A handler cannot use them to bypass its
entry protocol. Indirect calls require sealed entry identity, not an integer
that happens to resemble a code address.

## Machine-state regimes

A control-register write may change the machine regime. Its contract must name
the required prior state and established state; a mode switch is not merely an
unusual ABI.

The boundary `StatePlan` constrains the complete realized handler and its callees.
Source code that appears SIMD-free may acquire SIMD through optimization or
allocation, so [machine-state evidence](../spec/build/machine_state_evidence.md)
checks the final artifact. Assembly changing floating controls must restore the
canonical Omega controls before ordinary checked code resumes.

## Memory and hardware instructions

An integer address grants no memory authority. Assembly memory operations need
the same provenance, bounds, alignment and permissions as other operations.
MMIO normally uses plan-derived accessors; cache/TLB operations, DMA completion
and executable publication need their exact target contracts.

For example, a checked table writer does not acquire publication authority merely
by producing valid bytes. The consumer establishes the table's semantic validity
and the installer separately publishes it. See [hardware materialization](../spec/build/hardware_materialization.md).

## Required catalogs

Target support grows by adding complete instruction contracts and their checked
realization, not by granting arbitrary mnemonic access. A catalog entry, parser
test or emitted byte sequence alone is not evidence of complete native execution.
Current families and rejected forms are listed beside the catalog.

## Running assembly under interpretation

The interpreter executes the supported instruction's meaning, not a blob of
native bytes. For the structured `mov` above, that means an ordinary checked
assignment. Other operations can use block-local scratch or the supplied target
environment:

| Instruction effect | Interpreter responsibility |
| --- | --- |
| Move a value | Read/write the same authorized typed places. |
| Write a device port | Perform the exact port operation through the supplied environment. |
| Change interrupt state | Update persistent modeled state that governs subsequent event delivery. |
| Halt | Follow the processor model's halt and external-control contract, not report successful return or fuel exhaustion. |

The root and receiving host select/admit that environment. A simulated processor
does not confer control of the real host processor. Device providers supply
mechanisms; Cathedral still executes its own drivers, scheduler and boot logic.
No alternate body or `Binding` wrapper is required around every assembly block,
and no full-host emulator is required simply to interpret value operations.

Fuel pauses preserve the activation's scratch, persistent state and progress.
Resumption does not repeat an already performed port write. Interrupt timing,
ordering and legal external-entry points follow the target model; an interpreter
step is not automatically a native instruction. Address IDs alone do not supply
a physical memory map.

Missing instruction semantics, authority or environment support rejects before
execution. There is no implicit native/JIT fallback. A no-op is valid only for
effects the catalog actually permits to be elided. Interpretation does not by
itself admit an operation into constants or build-time code. See the
[complete execution contract](../spec/language/assembly.md#interpreted-execution)
and [embedding](chapter_24_embedding.md#native-and-interpreted-cathedral).

## Working rules

When offering a boundary API, describe the semantic operation in its contract
and keep instruction details in the selected implementation. Direct checked
assembly is also legitimate; a wrapper is not required for interpretation.
Preserve exact effects, authority, control exits and state restoration. If those
obligations cannot be discharged, the assembly rejects; a helper or admission
receipt cannot leave them open.
