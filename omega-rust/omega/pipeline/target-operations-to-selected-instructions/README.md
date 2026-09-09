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

Whole plain-owned arrivals with no runtime structural observer retain
`SelectedStructuralTransport::Unused`. Their complete semantic bindings and
owned value ABI survive; selection emits no payload pointer, descriptor slot,
memory access, or edge copy. The bounded input gate independently rejects
observations, call actuals, projections, and escapes of those owned places, and
executable cleanup. Independently established primitive locals may be observed
and borrowed by calls while the owned inputs remain unused. Exact no-code return discards remain in the retained source
ownership, whose current frontier is checked before selection. Scalar-only
calls and simultaneous scalar transfers continue through the ordinary graph.
Edge preparation and independent replay count only active transports, including
when unused owned bindings accompany scalar copies. Object/image publication
must separately retain this custody and validate omitted physical homes.
The [owned control-cycle regressions](../../../../tests/native-differential/tests/owned_control_cycles.rs)
exercise this full continuation; local projection tests also reject invented
descriptor homes and changed binding transports.

Target-input correspondence also checks `StructuralCase` terminators in
`TargetControlGraph` against the validated abstract graph: the exact dominating
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
Returning blocks retain exact no-code discards of their own affine byte-read
results. Current-unit ownership replay reconstructs each live frontier; target
and legalized replay preserve its ordered actions, return edge, and fuel. A
function-wide result roster cannot substitute for branch-local ownership.
Other case layouts and executable or residual return cleanup remain separate
admission limits; no new ISA case opcode or physical route is introduced.
Computed Boolean comparisons remain branch predicates until value materialization
is implemented.

Natural-ranked Unit writers and integer-result loops use this ordinary graph,
including cyclic descriptor arrivals, hosted byte output, selected scalar calls,
and scalar returns. Unranked loops use the same route. Legalization borrows the
verified artifact from the validated abstract stage and replays its exact current
components. The optimizer freezes the complete Natural or unranked cyclic function,
including prefixes and exits, against the verified source; it produces no
countdown certificate or fixed-work bound. Raw cyclic units cannot replace that
custody. Block layout uses reverse postorder when backedges prevent topological
scheduling; layout itself grants no termination authority.

Physical scalar demand begins at retained instruction operands, branch conditions,
and return values, then follows incoming edge bindings. Closed cycles of unused
bindings require no registers or copies. This omits no instruction, call,
semantic binding, or fuel charge. Construction prepares demand once with a backward
worklist; independent replay follows each declared value forward to an observer.
Explicit expected-set tests cover unused cycles, live multi-edge chains, swaps,
and retained instructions whose results are unused.

Whole, unqualified, unrestricted shared or mutable byte views use the same edge bridges.
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

Indexed mutable-view writes load the original backing pointer, form the checked
byte address with `ByteViewAddress`, and use the ordinary one-byte `Store`.
The write footprint retains the exact destination, index, byte, current length,
obligation and accepted fact. Independent replay checks the complete sequence;
only the store carries the source operation's fuel. Descriptor words and bytes
outside the selected element remain unchanged. Mutable subslices and bounded-owner
field replacement remain separate realization work.

Ordinary Unit calls may present an initialized raw fixed-u8 array as a mutable
byte view, retaining the whole source or exact field-only path. Independent
receiving checks reconstruct the array extent and offset from declarations;
scalar-result calls remain excluded. Selection stages only a two-word local
descriptor containing the original backing pointer and exact static length.
Existing frame stores/addressing and call transport carry that descriptor,
including outgoing stack pointers. No backing bytes are copied, and preparing
the descriptor adds no logical fuel charge. Repeated calls retain the original
root pointer. Image and installation custody rejoin the semantic path, array
metadata and selected/frame replay; descriptor shape alone is not authority.
The `mutable_writes::fixed_arrays` native tests cover whole arrays, nested fields,
unchanged siblings/padding, repeated calls, stack transport and corrupt metadata.

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

Primitive locals use operation-and-place identified activation slots. Each executed
establishment forms its frame address and performs one exact-width initializing
store. Loop reentry reuses the activation slot and executes initialization again;
the store is not hoisted to invocation entry. Later stores and borrowed Unit/scalar calls
use that original pointer; a scalar call retains its actual result independently.
Fresh primitive reads use pointer loads and distinct SSA definitions. `Load8`,
`Load16`, `Load32`, and `Load64` read exactly 1, 2, 4, or 8 bytes for Boolean,
fixed-width signed/unsigned integers, and IEEE binary32/binary64. Narrow loads
zero-extend the raw payload into the GPR carrier without changing its scalar
type; IEEE loads preserve payload bits without floating-point arithmetic.
Materialized Boolean reads used in stores and Unit calls are values, not
comparison-only branch suffixes. Boolean-local branching remains separate
source/graph composition work.
Construction and replay retain AddressLocal, WritePlace and ReadPlace records,
exact scalar demand, fuel, and the incoming parameter roster. Local storage does
not add an ABI parameter or a synthetic aggregate.

The ordinary control graph also composes fixed-integer primitive writes with a
64-bit integer result. Independent input replay joins the complete mixed ABI,
exact incoming reference, ordered store source, and scalar return. Selection
uses the existing pointer store and result constraint; it introduces neither a
Unit wrapper nor legacy store-byte records. Object/image publication retains
the mixed ABI and incoming borrowed homes under mandatory common-pipeline replay.

Runtime IEEE Unit arguments use their target's floating-register or exact-width
stack placement. Explicit raw-bit transfers connect floating ABI views to ordinary
GPR payload storage; the values retain IEEE scalar types. Mixed call constraints
retain the exact GPR and floating operand views, including Microsoft positional
holes. Incoming scalar stack loads and outgoing stores use private scalar ABI
addresses, not invented referent places. Independent replay derives source,
format, placement, width, and call order again. Runtime preservation spills keep
the payload's IEEE type and use integer-typed slot addresses; they do not admit
floating-register residents into GPR spill instructions.

Integer and Boolean stack arguments use the same transport. Incoming loads and
outgoing stores preserve the exact 1/2/4/8-byte payload width independently of
the target's stack-slot spacing and alignment. Narrow loads retain signedness
and Boolean type on the SSA value; zero-extension into GPR storage is not a
semantic widening. Replay rejects substituted widths, ABI offsets, argument
homes, and source identities, including when a borrowed pointer follows the
stack-passed scalars.

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
