# Provisioned storage and stack demand

[Logical work](logical_work.md) | [Terminal product](../terminal-psi/product.md)

## Concrete storage authority

There is no flat global memory-budget meter. Allocation and storage consume
concrete authority supplied to the execution:

- Individually sized extents or allocator capabilities.
- Stacks and activation-stack pools sized from verified demand.
- Static image/code storage admitted at installation.
- Qualified extents for pinned, shared, physical, DMA-visible, persistent, or
  other provider-defined memory.

Multiple heaps are multiple allocator/extent values. A component receives bounded
child authority rather than ambient global allocation. External retained storage
remains part of ordinary claims and custody. A spatial provision may belong to
an installation profile; it is not API/ABI identity unless the API promises it.

## Stack demand and backing

Worst-case stack usage (WCSU) is a spatial bound for a closed activation.
Use the final validated physical frame extent, including spills, rather than an
allocation-independent source estimate. Bind the composed demand to the exact
physical realization. A separate provisioning judgment establishes usable backing.
Runtime tail recursion lowers to iteration with no accumulated frames.

Retain and replay exact code-positioned frame, temporary, argument/shadow,
return-address, and call evidence. Derive local and caller-live peaks from the
instructions. Sequential callees compose by maximum; a nested call accounts for
its live caller frontier plus the callee's demand. An admitted opaque foreign
leaf contributes its exact caller-live frontier plus admitted bytes, retaining
the selected-plan identity, semantic requirement, admission coordinates, and
strong contribution commitment. Numeric alignment without demonstrated call
placement/padding is not an alignment proof.

Mutually exclusive branches compose by maximum. Reconstruction must validate
every reachable prefix, branch, return/crash leaf, and stack mutation. Shared
cleanup suffixes require the exact complete incoming frontier and edge roster;
replay them for each admitted path, not once as if all paths execute together.
Source attribution alone does not independently prove a control-flow relation
that the object replayer cannot see. The canonical semantic subject and its
verified lowering correspondence remain necessary.

A native body-demand theorem excludes external entry adapters and architectural
arrival state. Root admission joins it to the selected context-indexed entry
realization specified by [entry-stack composition](entry_stacks.md).
Body demand contributes only to the Body epoch's execution domain;
arrival and generated/admitted adapter epochs retain their own domains and
nesting allowances. Changing a strong premise changes the bound identity even
when numeric totals happen to match.

That join is not itself a final stack plan, provisioned lease, or admission
receipt. Installation must establish backing and the applicable ceiling before
execution; compiler spill accesses do not add author-visible crash causes.

### Compiler-owned stack accesses

Spill slots belong to the activation frame. Derive final live physical extent
including reuse, alignment, saved registers and calling-plan storage, not the
sum of spill instructions or overlapping slot demands. Compose it through the
closed call chain and external-root context/nesting rules, including checked or
admitted same-stack providers, then compare it with admitted supply.
Runtime non-tail recursion rejects; tail backedges accumulate no frames. A
termination measure alone supplies neither frame sizing nor recursive stack
capacity. Spill realization introduces no recursion-depth or per-call exhaustion
protocol.

Provisioning must establish access, alignment, valid backing and lifetime through
the selected stack plan/lease or external-entry contract, including suspension.
A byte count alone is insufficient. Failed supply or establishment follows
admission, installation or activation failure, not a new source `Trap`. Ordinary
nested calls consume the composed bound rather than adding a new exhaustion
mechanism.

Within that established contract, validated spill loads/stores are non-faulting
in the language model. Target-required setup or probing may establish the
contract and must charge its live overhead. It does not remove physical
provenance, bounds, offsets, alignment, lifetime or frame replay. A wrong generated
address or expired backing is a compiler defect, not permitted resource failure.

Demand binds the exact optimization/allocation result, final frames, target and
calling rules, and installed artifact. Matching Terminal semantics alone cannot
reuse another realization's demand. Existing independently checked transitive
identity relations may establish the join; no parallel hash field is required
solely to repeat it. Abstract spill requirements and stack composition alone do
not establish physical realization or provisioned backing.

## Program-local content roots

A content-bearing domain authorizes one exact entry requirement. Fresh local
lineage may appear only at its statically enumerable installed parameter
positions. The requirement states finite capacity per occurrence or constrains
a selected constant family. Ordinary invocation must supply an existing root;
calling the requirement again cannot reset the account. No additional provision
declaration or data annotation establishes capacity.

[Content custody](content_custody.md#installed-introduction-schemas) defines
reconstructed introduction schemas, finite installed occurrence sets, and
cross-era aggregate accounting. A shared assembly cap is one parent root divided
among children. A machine-lifetime cap persists across epochs; it cannot be
recreated as fresh local authority.

## Capacity is not placement

A counted byte quantity accounts for normalized requested size, alignment padding,
and allocator metadata against a proof-level natural residual keyed by the
`Bytes` unit. A residual extent supplies placement; the number does not.
For bump allocation, releasing a live extent does not restore capacity until
reset recomposes the original backing. [Allocation strategies](allocation.md)
own that package-level contract; [bounded growth](bounded_growth.md) states when
fixed storage can avoid allocation entirely.

A free-byte count does not prove that a fragmented heap contains the required
placement. Such allocation stays fallible unless an exact free-extent or
reservation theorem establishes the request. Keep capacity, placement, custody,
and recomposition evidence distinct.
