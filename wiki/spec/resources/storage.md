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
realization. Body demand contributes only to the Body epoch's execution domain;
arrival and generated/admitted adapter epochs retain their own domains and
nesting allowances. Changing a strong premise changes the bound identity even
when numeric totals happen to match.

That join is not itself a final stack plan, provisioned lease, or admission
receipt. Installation must establish backing and the applicable ceiling before
execution; compiler spill accesses do not add author-visible crash causes.
See the [language's compiler-owned stack contract](../../language_guide/chapter_16_errors_traps_failure.md#compiler-owned-stack-storage-and-spill-accesses).

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
reset recomposes the original backing.

A free-byte count does not prove that a fragmented heap contains the required
placement. Such allocation stays fallible unless an exact free-extent or
reservation theorem establishes the request. Keep capacity, placement, custody,
and recomposition evidence distinct.
