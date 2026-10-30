# Chapter 22: Inline Assembly

Assembly is a checked low-level operation surface, not an escape from Omega's
ownership, authority, effects or control flow. The [assembly specification](../spec/language/assembly.md)
defines the contract; the [instruction catalog note](../../omega-rust/psi/foundation/language-core/inline_assembly.md)
records current source and target coverage.

`asm machine` gives an ordinary machine an assembly body. It can implement the
same requirements as an ordinary machine. Nested assembly-bodied declarations
also spell `asm machine`; nesting does not silently change the meaning of an
unmarked `machine`. Inline `asm { ... }` remains available inside ordinary code.

## Parsed instructions with contracts

Each accepted instruction has structured operands and a compiler-owned contract.
For x86 port output, for example:

```omega
asm {
    out port, value
}
```

The port is exactly `u16`, the byte value exactly `u8`; a wider place does not
narrow implicitly. Required authority and service reach remain obligations of
the enclosing machine. Several instructions use explicit `;` separators.

A block may state assertions and its exact realized register footprint:

```omega
asm
    requires ready
    clobbers none
    ensures ready
{
    lfence; sfence
}
```

Here the x86 fences change ordering, not general-purpose registers. `requires`
is proved on entry and `ensures` on fallthrough after invalidating stale facts.
Neither clause manufactures evidence or overrides an instruction contract.
An authored clobber set must be exact: both omitted and invented changes reject.
An outgoing jump checks the source block's `ensures` before entering its
successor. A nonreturning environment exit does not invent an ordinary return.

## Separate regions do not share hidden registers

Separate assembly regions cannot communicate through whatever a previous region left in
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

## Label contracts and loops

One assembly body contains an explicit checked control-flow graph. Nested
assembly machines name basic blocks, with optional ordinary `requires`,
`ensures` and exact `clobbers` clauses:

```omega
asm machine drain(count: u32)
terminates
registers ecx = count
{
    transition -> check;

    asm machine check {
        test ecx, ecx;
        jz done;
        transition -> step;

        asm machine step
        requires ecx > 0
        {
            sub ecx, 1;
            jmp check;
        }
    }

    asm machine done
    requires ecx == 0
    {
        ret;
    }
}
```

Every predecessor, including a loop backedge, proves the destination's
`requires`. The checker applies each instruction's catalog semantics and proves
the source label's `ensures` on every exit. Registers, flags, modeled memory,
ordinary places and values, authority, domains and live ownership obligations
are all part of that symbolic state. Separate `asm` statements still share no
hidden scratch registers. Nested blocks within one region do share the exact
modeled incoming register and flag state through their explicit edges.

A conditional jump is the final native instruction of its block. The following
`transition` names its not-taken successor and must place that successor
physically next, with no transfer instruction. The pair is one complete
terminator; another instruction after `jz` would need another block. An actual
`jmp` handles a nonadjacent target or backedge. Impossible adjacency rejects,
rather than quietly inserting a jump. Declarations do not execute by themselves.

A proposition over modeled memory performs no load and grants no access. For
example, `[eax] == 1` at a label asks the proof system about the memory state
identified by `eax`; an actual load or store separately owes its instruction
contract. Ambiguous width, address space or backing makes the proposition
unprovable. Loops use these label invariants and, when termination is required,
the ordinary ranked-decrease rules rather than an asm-only termination system.

The `registers` header initializes working registers once. It may emit code;
it is not a declaration of where the caller supplied arguments. Initializers
refer to the incoming input values, even when their physical locations overlap.
The initial transition emits no trampoline, and a backedge skips initialization.

A nested clobber list describes that block's own instructions, not every
successor it can reach. Instruction effects are inferred when the list is
omitted. A temporary write followed by restoration still contributes to the
write footprint; ABI preservation of the original value is checked separately.

## Calling conventions and results

Ordinary entries use Omega's compiler-owned internal convention. An implementation
of a boundary requirement uses that requirement's `Calling<Policy>` plan, whether
its body is assembly or ordinary Omega. The plan describes all parameter and
result locations, stack obligations and preserved state, not just a register
count. A working-register binding cannot change the caller's ABI.

There is no extra `outputs` interface. At `ret`, the checker proves the declared
result is fully initialized in the plan's result locations and proves the
required preservation, ownership and postconditions. A result may span registers
or use a hidden output destination. Copying that destination pointer retains
its established storage relationship, not new authority from its numeric bits.
Inline assembly cannot use `ret` to bypass its enclosing machine's control flow.
See [calling plans](../spec/build/calling_plans.md).

## Generics and slices

Generic assembly machines use ordinary type, const and machine parameters.
Each admitted application must justify its instruction widths, representation,
access and ownership; equal sizes alone do not authorize copying arbitrary T.
An assembly callback can satisfy the library iterator's ordinary step contract,
so `for_each<Element, Step>` needs no asm-only variant. This is static machine
composition, not textual instruction injection or guaranteed unrolling.

Ordinary slices remain valid parameters under Omega's internal convention.
Their live bounds and loans still govern assembly accesses. A foreign calling
policy instead needs an explicitly admitted descriptor shape or a pointer/length
adapter; rejecting an unspecified foreign slice ABI does not prohibit slices.

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

Internal jumps target declared assembly machines; a jump leaving the body targets
a visible Omega machine. Both are checked transfers whose target contracts, arguments,
invariants and live obligations remain visible. Control transfer is the final
instruction in its block, except for the zero-instruction transition completing
a conditional terminator. Undeclared targets, uncontracted returns and unwinds
remain rejected. A local jump neither creates a frame nor repeats register setup.

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

`align_of<T>()` reports a type layout's minimum alignment; `align_of(value)`
observes the actual location. On a nonempty borrowed slice it observes the
first element, not the descriptor. A branch on that observation can prove an
aligned instruction's precondition. Bounds and access authority remain separate.
Copies and offsets of a storage address retain only the relationships justified
by dataflow and the backing's live loan; arbitrary integers acquire no access.

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

A checked implementation of a consuming boundary-trait member may discharge a
linear argument through assembly only when the instruction semantics and label
contracts prove the required effect on every normal exit. Merely naming the
member, reaching the right service or writing an `ensures` clause proves
nothing by itself.
