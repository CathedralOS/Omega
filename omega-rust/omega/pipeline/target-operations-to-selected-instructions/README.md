# Instruction selection

[lib.rs](src/lib.rs) enters mandatory target legalization and independently
validated instruction selection. Input is the complete admitted target-operation
custody and exact validated target register environment; output is the admitted
current selected program with semantic, target, projection, fuel and constraint
identities. Raw selected data or a detached content hash is not selection authority.

## Target setup is not a program stage

[Register environment](../../backend/register-environment/src/lib.rs) joins the
exact native target, ISA physical model, instruction constraints, selected keys
and active reservation profile. Baseline and decoded/custom inputs use independent
structural and ISA-semantic validation. Selection takes that carrier explicitly;
it must not import ISA crates or reconstruct a supposedly equivalent environment.
Setup is neither instruction selection nor an optimization pass.

## Ordinary selected control flow

[Legalization](src/legalization/mod.rs) and [selection](src/selection/mod.rs)
retain ordered instructions, block parameters, explicit jumps/branches/returns,
virtual registers, exact fixed ABI constraints and machine effects. Roster order
is not control-flow order. Arithmetic and calls use the same ordinary graph;
physical register homes, liveness, frame storage and emission are downstream jobs.

Each instruction retains operation, obligation, value, definition and fuel
provenance. Successor bindings preserve semantic identities and explicit register
transport, with exact edge, polarity, target and taken-edge fuel. Fixed views are
constraints, not assigned homes. ISA-owned RFLAGS/RIP or NZCV/PC effects do not
become fictional source values. Compiler condition tests, copies and address
work do not invent Psi operations or logical charges. Returns retain their exact
Unit/scalar role, result constraint where applicable, and edge fuel.

Unit graphs use this same transport for `u8`, `u32`, `u64`, `i64` and
materialized Boolean block arguments. Exact block/value/type references are
available only in their defining block and dominated successors; each arrival
retains its own ordered bindings, even when both conditional arms have the same
target. A transferred integer does not inherit byte-length observation custody.
Selection expands materialized edge bindings into explicitly identified
implementation blocks. Each chosen edge snapshots all arguments before copying
them into destination-associated transfer registers. Distinct arrivals need not
share their original homes; parallel swaps do not overwrite unread arguments.
Ordinary liveness, allocation and encoding own these copies. Independent replay
checks the expansion and contracts it back to the source graph, with the authored
edge and its fuel retained once; the final implementation jump carries lineage,
not a second semantic transition. No source block or operation is fabricated.

Target-input correspondence also checks `StructuralCase` terminators in
`TargetUnitGraph` against the validated abstract graph: the exact dominating
result home, declared case order/tag, relevant field offset, destination
block/value/type, and edge cleanup must agree. The admitted two-case Linux byte
result lowers through ordinary frame-address, tag-load, compare and branch
instructions. Used i32 payloads load only in the chosen edge's implementation
block, before destination binding; unused payloads retain semantic metadata
without a load or bridge. Structural observations have no fabricated source
value or definition site. The semantic edge alone retains no-code affine cleanup
and fuel; its implementation continuation retains payload lineage and register
transport, with empty cleanup and fuel. Independent selection and bridge replay
restore the exact legalized case, including its destination-owned definition.
Other case layouts and multi-block return cleanup remain separate admission
limits; neither a new ISA case opcode nor a second physical route is introduced.
Computed Boolean comparisons remain branch predicates until value materialization
is implemented.

Natural-ranked Unit writers use this ordinary graph, including cyclic descriptor
arrivals, hosted byte output and caller continuation. Legalization borrows the
verified artifact from the validated abstract stage and replays its exact current
components. The optimizer freezes the complete natural-ranked function, including
rank-producing prefixes and exits, against the verified source; it produces no
countdown certificate or fixed-work bound. Raw cyclic units cannot replace that
custody. Block layout uses reverse postorder when backedges prevent topological
scheduling; layout itself grants no termination authority.

Whole, unqualified, unrestricted shared byte views use the same edge bridges.
Each block parameter owns a 16-byte activation-local descriptor slot. Its address
is formed at invocation entry without reading uninitialized contents. The chosen
edge snapshots both words of every incoming descriptor and all scalar arguments
before any destination replacement, preserving parallel swaps and earlier aliases
when a producer's slot is reused. Backing bytes are never copied. Independent
replay rejoins the exact source place, destination block/place, offsets and edge;
memory metadata distinguishes operation, block-address and edge-copy origins.
Source-graph availability establishes initialized descriptor contents; address
register dominance alone does not. Length/bounds observations belong to the newly
bound destination view rather than an arbitrary predecessor's scalar length.

Structural argument snapshots, loads/stores, frame addresses and calls use
ordinary virtual instructions with exact slot/access/call metadata. Copying an
owned referent into a distinct ABI temporary and forwarding a borrowed pointer
are different transports. Claim completion can retain evidence at an instruction
position without inventing executable work. Frame-free bodies must not acquire
storage merely because their parameters are structural; actual outgoing and
preservation storage must be replayed against the realized frame.

Straight-line Unit field stores through mutable or write-only parameters retain
the original incoming pointer. IEEE binary32/binary64, Boolean, and 8-, 16-, 32-, and 64-bit integer
stores carry the exact destination declaration, carrier path, scalar field and
typed SSA source. Input-only layout replay reconstructs the offset and width;
selection emits one pointer store with a write footprint and the source fuel
charge, without loading the destination. Independent replay checks the pointer,
value, write extent and charge before accepting the selected program.

Whole primitive stores use the same exact-width pointer instruction while
retaining `WriteOnlyPrimitiveStore` as their semantic origin. Their destination
must be an unrestricted, unqualified, claim-free mutable or write-only parameter
whose referent is exactly the SSA source's primitive scalar type. Boolean and
fixed 8/16/32/64-bit integers and IEEE binary32/binary64 are supported; no synthetic
record, field, readable borrow, or IEEE-to-integer conversion is introduced.

Runtime IEEE Unit arguments use their target's floating-register or exact-width
stack placement. Explicit raw-bit transfers connect floating ABI views to ordinary
GPR payload storage; the values retain IEEE scalar types. Mixed call constraints
retain the exact GPR and floating operand views, including Microsoft positional
holes. Incoming scalar stack loads and outgoing stores use private scalar ABI
addresses, not invented referent places. Independent replay derives source,
format, placement, width, and call order again. Runtime preservation spills keep
the payload's IEEE type and use integer-typed slot addresses; they do not admit
floating-register residents into GPR spill instructions.

IEEE field-store and Unit-call literals retain their exact defining operation,
SSA value, format and raw bits in the ordinary scalar-source vocabulary.
Independent receiving replay establishes availability only after matching the
preceding constant operation. The existing raw-bit materialization and floating
ABI transfers then realize the value; source literals introduce neither numeric
conversion nor a separate call or store instruction family.

Literal and transported subslice descriptors use activation-local homes, separately
from ABI argument-copy slots. A subslice retains the original backing plus
exact integer offset and length; a subslice used as a call or block argument acquires
an addressable 16-byte descriptor. Repeated calls forward that descriptor,
never a copy of its bytes. Producer/result identity, bounds, source-place availability
and selected address lifetime remain required, including across guarded blocks.

On the current 64-bit targets, `ByteViewAddress` forms the private descriptor's
address bits without asserting exact source integer arithmetic. Valid root
geometry `B + R <= Bound` and view geometry `O + L <= R` imply
`B + O <= Bound`; equality forces an empty view. A one-past endpoint at
`2^64` therefore becomes zero pointer bits with zero length, not an overflow
proof or permission to read address zero. Other empty views need not form a
dereference either. The original bounds and backing remain the access authority;
this address calculation introduces no Psi operation or additional fuel charge.
See [extent endpoints](../../../../wiki/spec/resources/extents.md#conservation-and-loans).

The admitted Linux `write_byte(i32)` boundary retains its exact SSA input and
builtin identity in the same graph. Its selected form owns one byte of local
scratch identified by the boundary operation, not a fabricated structural place.
The ISA realization stores the low byte, performs the existing stdout write,
and continues only on a positive result; failure traps. Registers, flags, local
memory and the external effect are explicit, and the form does not adjust the
stack pointer behind frame allocation. Independent decoding checks the complete
syscall sequence and resolved slot. Object evidence distinguishes this selected
input transport from legacy immediate materialization. This Linux realization
does not authorize a macOS/Windows provider or establish writer control flow.

Hosted `exit_process(i32)` uses the ordinary Unit scalar graph rather than an
isolated literal-only target operation. Its last boundary node becomes an explicit
process-exit terminator on Linux x86-64/AArch64 and macOS AArch64. The source's
following empty Unit return remains nominal custody: its edge is retained, but
its fuel is not charged as executed work and no native return or epilogue is
emitted. The terminal reads the original runtime i32 carrier, needs no scratch,
and traps if the kernel returns. Source, target, builtin, operand, and complete
exit span are independently replayed through publication.

Ordered Unit scalar definitions join their exact source operations before
entering the same legalized SSA graph. Explicit `u8` widening to fixed
16/32/64-bit signed or unsigned integers reuses ordinary copies after byte
inputs are zero-extended at the ABI boundary. Result type, defining operation,
source identity and value residence remain independently replayed; no new
instruction or forced stack home is needed. Other source widths and general
Unit control-flow composition remain separate realization work.

[Construction](src/selection/construction/mod.rs) and independent validation
derive separate projections. Replay checks the complete selected content against
the semantic/optimized input, target plan and register catalog, including exact
instruction order, ABI copies, call slots, memory accesses and zero-instruction
settlements. Producer success is not its own validation.

## Non-executable retained selection family

The `projected_structural_call_returns` plan retains a bounded owned-linear
projected call/return closure separately from ordinary selected functions. It
records exact fragment placements, fixed constraints, implicit effects and
required transfers without ordinary scalar registers/instructions. On differing
x86 views the copy uses the complete target copy constraint; AArch64 same-view
transport remains an explicit no-copy fact. Both
[liveness](../selected-instructions-to-selected-instructions/src/analyses/liveness/compute.rs)
and [machine-effect analysis](../selected-instructions-to-selected-instructions/src/analyses/machine_effects/facts/compute.rs)
reject a nonempty retained family. Its existence is not physical or publication
support and must not become an alternate executable pipeline.

Broader supported behavior must enter the common graph with exact ABI, memory,
ownership, proof and frame replay. Unsupported shapes reject at their owner;
there is no assigned-program fallback. [Allocation](../selected-instructions-to-register-homes/README.md)
owns homes and recovery, and [machine emission](../../backend/machine-emission/README.md)
owns the physical continuation.
