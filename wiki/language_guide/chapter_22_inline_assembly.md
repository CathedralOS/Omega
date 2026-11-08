# Chapter 22: Inline assembly

Use inline assembly for operations that need a particular instruction or
machine-state transition. Each supported instruction has a contract that the
compiler checks alongside your code.

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

## Pass values between assembly blocks

Separate assembly regions cannot communicate through whatever a previous region left in
a general register or scratch flag. Inputs and outputs use ordinary typed places:

```omega
asm { mov saved, input }
asm { mov output, saved }
```

`saved` carries the value between blocks. Scratch registers and flags are local
to each block; use typed inputs and outputs for communication.

Memory writes and device effects persist across blocks. If a block masks
interrupts, restore the saved state after the protected work.

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

The zero test establishes `ecx == 0` on the edge to `done`. The other edge
establishes `ecx > 0`, permitting the subtraction in `step`. Every backedge
must prove its destination's requirements again. A termination promise also
needs a decreasing measure for the loop.

A conditional jump is the final native instruction of its block. The following
`transition` names its not-taken successor and must place that successor
physically next, with no transfer instruction. The pair is one complete
terminator; another instruction after `jz` would need another block. An actual
`jmp` handles a nonadjacent target or backedge. Impossible adjacency rejects,
rather than quietly inserting a jump. Declarations do not execute by themselves.

The `registers` header initializes working registers once; a backedge skips
that setup. It may emit instructions. A nested `clobbers` clause describes the
block's own writes, including temporary writes later restored. Preserving a
caller's register value is a separate obligation.

For contracts over memory and exact instruction footprints, see the
[assembly reference](../spec/language/assembly.md).

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

## Declare hardware authority

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

## Jumps and entry protocols

Jumps target declared machines and prove their contracts. A local jump keeps
the frame and skips entry register setup. Indirect calls require a checked
entry identity.

Interrupt returns and similar entry/exit operations belong to the generated
entry protocol. A handler supplies its checked body inside that protocol.

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
| Halt | Follow the processor model's halt and external-control contract, not report successful return or work-budget exhaustion. |

The host supplies the target environment. Missing instruction semantics or
environment support prevents execution. A work-budget pause retains state so
resuming does not repeat an already performed port write.

See [interpreted execution](../spec/language/assembly.md#interpreted-execution)
and [embedding](chapter_24_embedding.md#native-and-interpreted-cathedral) for
processor state, interrupts, and external entries.

## Further reference

The [instruction catalog](../../omega-rust/psi/foundation/language-core/inline_assembly.md)
lists supported operations. The [assembly specification](../spec/language/assembly.md)
defines their checking, control flow, and target requirements.
