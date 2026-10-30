# Checked assembly

Named decision: `checked-assembly-machine-bodies`, settled 2026-10-29. Assembly
machines have an explicit assembly body language and ordinary checked machine
contracts. Nested assembly machines name the basic blocks of one checked
control-flow graph. This refines `checked-assembly-label-contracts`; colon labels
and implicit inter-block fall-through are retired source forms. Assembly has no
weaker, asm-only proof system.

`asm machine` declares an assembly-bodied machine. `asm { ... }` embeds a checked
assembly region in ordinary Omega code. Both are parsed for the selected target,
not treated as opaque text or bytes. Every accepted instruction has a
compiler-owned contract; unknown instructions and raw emitted bytes reject.
A prebuilt blob is a foreign provider artifact under admission, not inline
assembly. There is no compatibility `ForeignBinding::Instruction` source form.

Under [embedded interpretation](../build/embedding.md), supported instructions
execute their catalog-defined semantics against ordinary values/storage and an
explicitly supplied target environment. Neither an alternate body nor a wrapper
per block is required. This grants no execution support to an unmodeled
instruction. See [interpreted execution](#interpreted-execution).

## Machine bodies and checked entry

Every assembly-bodied declaration spells `asm machine`, including a declaration
nested inside another assembly machine. An unmarked `machine` does not silently
switch body language because of its parent. Executable Omega statements are not
mixed into the instruction stream; the structural `transition` form below and
machine contracts are checked control-flow metadata.

An assembly machine can implement an ordinary machine requirement, including a
member of a named implementation. The body must satisfy the requirement's full
signature, propositions, ownership, crash, reach, termination and applicable
calling/state plan. `asm` selects a body language, not an unchecked provider or
a distinct satisfaction relation.

Within one assembly region, nested assembly machines are local continuations,
not fresh ABI entries. Entering one through a local edge neither allocates a
frame nor repeats argument setup, register initialization or a prologue. Nesting
scopes names, not runtime storage or authority. Separately callable entries still
owe the ordinary explicit-argument and calling-plan rules.

## Explicit control flow

Each assembly machine's executable body is one basic block. Nested declarations
do not execute merely because the preceding source ends. Instructions use `;`
separators. The terminating forms are:

| Form | Meaning |
| --- | --- |
| `transition -> next;` | Checked transfer to the physically adjacent local block, with no emitted transfer instruction. |
| `jmp target;` | Actual direct branch to a visible local continuation, including sibling edges and backedges. |
| Conditional jump followed by `transition -> next;` | One conditional terminator: the instruction names the taken edge; the transition names the adjacent not-taken edge. |
| A catalog-defined return or other exit | Checked completion under the enclosing entry/exit plan, or the exact contracted environment exit. |

A conditional jump is the final native instruction of its block. Its immediately
following `transition` completes that terminator; no other executable statement
may intervene or follow. An unconditional transfer or return terminates
immediately. There is no `else` instruction suffix or implicit missing successor.

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

The initial transition selects the instruction entry after one-time setup; it
does not require a trampoline. A later jump to `check` skips that setup.
Transitions may connect a parent to its first nested block or a block to its
next laid-out sibling. Lexical nesting alone does not establish adjacency.
An intervening instruction range, incompatible adjacency constraints or an
unrealizable layout rejects; the compiler cannot silently turn `transition`
into a jump. No register moves, prologue or other hidden setup may be inserted
on that edge. An actual `jmp` may be relaxed or elided where its semantics permit;
that does not weaken the source transition's zero-instruction guarantee.

An inline `asm` region with no explicit control transfer may complete at its
checked body end and resume ordinary Omega code. That exit is not implicit entry
into a nested declaration. An assembly machine must complete through its checked
exit rather than accidentally running past its body.

## Contracts and clobbers

Root and nested assembly machines may carry `requires`, `ensures` and optional
`clobbers` clauses. A callable machine retains its ordinary contract over the
complete invocation: its `ensures` is proved at normal completion, not at an
initial transfer into its CFG. Local continuation contracts instead describe
block entry and exit. Every incoming edge proves the destination block's
`requires` against its exact register, flag, memory, place, value, authority and
ownership state. Each outgoing edge proves that local block's `ensures` after
its own instructions and before executing its successor. Those assertions do
not recursively execute a successor's body. A loop backedge still owes the
enclosing machine's ordinary ranked-decrease obligation when termination is
required.

The enclosing checked control point proves the initial entry obligations.
Lexical nesting imports neither unproved parent predicates nor stale facts.
Instruction requirements generate proof goals; an instruction's presence does
not establish its own premises. Existing flow facts and admissible private
contract inference can discharge those goals without repeated annotations, but
every actual incoming edge must justify any inferred requirement. Public
contracts cannot silently strengthen from one implementation's body.

Block propositions observe the symbolic assembly state; they emit no instruction
and grant no runtime access. A modeled memory proposition such as `[eax] == 1`
is a proof query, while a load or store separately owes access, provenance,
alignment and authority. An ambiguous width, address space or backing rejects.

These are checked assertions, not admissions or extra facts. Body-level inline
`ensures` checks the ordinary body exit; a named block's `ensures` checks each
outgoing edge, including a jump. Neither overrides the instruction catalog.

The optional inline body contract uses the same direct clauses:

```omega
asm
    requires context.ready
    clobbers none
    ensures context.ready
{
    lfence; sfence
}
```

Here `context` is an explicit ordinary value in scope, not an implicit receiver.
An authored clobber set must equal the union of the realized instruction
contracts' changed registers. Omitted changes and invented changes both reject;
order and duplicates have no meaning. An explicitly empty set is `clobbers none`.
Omitting the clause never omits the catalog's effects.

For a nested block, the set covers its own instructions up to outgoing edges,
not the union of every reachable successor. The enclosing region accounts for
the complete graph and one-time setup. Implicit instruction writes, flag effects
and overlapping register views remain authoritative even when not spelled as
destinations. Declaring a clobber neither initializes a register nor saves it.
Writing then restoring a register still contributes a write footprint; proving
ABI preservation of its entry value is a separate obligation.

## Register initialization and calling plans

An independently entered body may bind initial working registers with a
`registers` clause, as in `registers ecx = count`. The right sides are checked,
pure entry expressions over explicit inputs and authorized projections, not
arbitrary executable Omega bodies. All right sides refer to the incoming values;
initialization preserves that meaning when source and destination registers
overlap. The compiler must not sequentially destroy an input needed by a later
binding. Boolean conditions bind their target register representation, retaining
the relationship to their propositions for subsequent branch checking.

Bindings are initialization, not a declaration of where the caller supplied
arguments. They may emit moves or computations, contribute their realized
effects, and must prove indexing, address formation and other partial-expression
obligations before the body begins. Nested CFG continuations instead receive
the exact incoming register state; there is no automatic rebinding on an edge.
Proof assertions may name the region's explicit input identities while their
relationships remain live; this does not capture an enclosing Omega activation.

The [calling plan](../build/calling_plans.md) owns interface placement. Ordinary
Omega entries use the compiler-owned internal convention; boundary requirements
select `Calling<Policy>`, whose validated plan governs both callers and checked
assembly implementations. Typed symbols remain available as operands under
their ordinary value/place rules; an input name is not an immutable promise
that a particular physical register still contains that input after a write.
Explicit working-register choices must respect instruction and state constraints.

There is no separate `outputs result = register` clause. The signature defines
the logical result, and the calling plan defines all its physical locations.
At a checked `ret`, the implementation proves initialization and validity of
every result component, postconditions, ownership dispositions, stack state and
required preservation. Results may use zero or several registers or an indirect
destination; a record of two results does not necessarily return in two registers.
Authorized mutations through parameters remain separate effects.

For example, the Microsoft x64 plan for a 16-byte C-layout record returns through
a caller-supplied output destination. The plan locates the hidden pointer in
`rcx`; the entry contract establishes its writable extent and alignment. Copying
it to `r9` retains that exact storage relationship, so stores at `[r9]` and
`[r9 + 8]` can initialize two `u64` fields without an additional assertion on
`r9`. The policy does not create access authority, and an arbitrary integer in
`r9` proves nothing. Initialization and return of the destination pointer in
`rax` must satisfy the same plan.

Whole-machine assembly does not by itself promise the absence of all generated
entry/exit code. Exact-control entries must account for their complete validated
entry, stack, restoration and exit contract; no uncontracted prologue, spill or
epilogue is permitted. An inline region does not acquire control of its enclosing
machine's ABI merely by using physical registers.

## Generics, slices and tracked storage

Assembly machines use ordinary type, value, const, static-machine and named
implementation binders. Nested declarations retain exact enclosing static
applications, not runtime closures. Every admitted specialization must justify
its instructions, operand widths, layouts, multiplicity and target features.
Equal byte size alone does not authorize copying or relocating arbitrary `T`.
An assembly machine may satisfy a generic machine parameter under the same
contract as an ordinary machine; a library iterator needs no asm-only variant.
That composition neither injects source instructions nor guarantees inlining
or compile-time unrolling.

Ordinary `&[T]` and `&mut [T]` parameters retain length, backing, lifetime and
access obligations in assembly. A native boundary policy must explicitly admit
their foreign shape or use a checked pointer/length adapter; that ABI restriction
does not prohibit slices under Omega's internal convention.

Register dataflow retains justified relationships to authorized storage through
copies, pointer adjustments and joins. Each access still proves its current
extent, offset, initialization, alignment and permission. Clobbering a pointer,
losing the relation at a join, changing the backing or ending its loan invalidates
the affected evidence. Neither numeric address exposure nor tracking metadata
creates a loan or extends one. See [address exposure](counts_and_addresses.md#address-exposure)
and [location alignment](counts_and_addresses.md#location-alignment).

## Assembly state and explicit communication

A body has no implicit general-register or scratch-flag inputs from surrounding
compiled code. Such inputs must come from its declared operands or a
catalog-defined state observation. Within one body, register and flag versions
flow through explicit machine edges and are part of each block contract; separate
`asm` statements share no hidden scratch state. Register allocation and
incidental preservation are not source contracts. Clobbers account for changes;
they do not supply initial values.

Communication between separate bodies uses ordinary Omega values/places,
authorized memory effects, or explicitly contracted environment state. For
example:

```omega
asm { mov saved, input }
asm { mov output, saved }
```

Here `saved` is an ordinary typed place, not a retained register. Conversely,
interrupt-mask changes, device writes and memory updates do not reset when a
label or body exits. System registers and architectural control flags explicitly
read or changed by catalog operations belong to that persistent state. Its
lifetime and sharing follow the selected execution environment and held
authority. Labels and bodies are not transactions, isolation domains, or
rollback points.

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

The same proof closes ordinary ownership obligations. A checked machine that
implements a consuming boundary-trait requirement may terminally dispose of a
linear argument only when the assembly CFG proves the requirement's effect and
every normal exit carries the corresponding ownership disposition. A `nop`, a
write to the wrong modeled place, missing ordering or a path bypassing the
effect leaves the value live and rejects. No native binding or assembly-specific
trust assertion substitutes for this proof.

An [assumed binding contract](../build/assumed_binding_contracts.md) can supply
explicitly admitted premises at an authorized runtime crossing. It cannot exempt
an assembly block from the instruction catalog, reinterpret an assembly contract as an
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
entry/exit operations. Jumps within a body target declared assembly machines;
jumps leaving the body map to explicit checked Omega terminal transfers to
visible machines. Source nesting does not grant a raw jump into another body
or bypass its contract/acknowledgements. Both forms retain arguments, invariants
and live obligations. Undeclared targets,
uncontracted returns, unwinds and unmodeled exits reject. An assembly machine's
return requires its validated exit plan; an inline region cannot return past
its enclosing machine. Return-from-interrupt cannot bypass an entry contract.
Direct targets remain final-artifact-validated;
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

## Implementation status

The machine-body, adjacency-transition, entry-register and raw-memory forms above
are the accepted language contract, not claims of complete implementation.
Both producers, retained representations, interpretation, native realization
and final-artifact checks must migrate together. Existing colon-label parsing,
implicit conditional fall-through and rejection of general returns describe
the current subset, not an alternate supported language. The owning task in
[TASKS.md](../../../TASKS.md#checked-assembly-control-flow) retains that migration
and its positive execution and invalid-control acceptance.
