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
Unit/scalar/aggregate role, complete result constraints, and edge fuel.

Fresh scalar sums use activation-local carriers in this graph. Construction
initializes the complete carrier, including padding and inactive payload bytes,
then writes the declared tag and exact-width fields. The carrier-address
instruction retains the single source-operation charge; its other initialization
instructions add none. Ordinary calls capture every direct result fragment into
the caller's result slot, and returns load every fragment under the same retained
ABI plan. Result definitions are not also unknown call clobbers. Current direct
carriers cover one or two register fragments; hidden-pointer returns remain an
explicit realization limit, including 16-byte sums on Microsoft x64.

Primitive arrays share that aggregate storage and call/return path without a sum
tag. [Array input](src/selection/scalar_array_input.rs) reconstructs the declared
dimensions and leaf carrier; constructor selection writes each row-major leaf at
its exact width. [Aggregate results](src/selection/aggregate_result_input.rs)
joins both array and sum homes to their retained calling plans. Independent replay
checks every store, result fragment, and the single constructor fuel charge.
Owned arguments load those same homes or copy captured incoming fragments;
returning an incoming array retains its parameter identity. Each argument joins
its exact producer, place, type, and destination ABI, including mixed borrowed
arguments. Direct values retain complete graph replay at publication, not
pointer-only legacy records.
Direct fragments cover exact widths 1 through 8 bytes, up to two registers;
Microsoft x64 supports its single-register direct results. Odd widths use
`LoadPacked`/`StorePacked` with an explicit instruction-local scratch register.
The ISA expansion accesses each meaningful byte exactly once without rounding
the storage extent up to a machine word. Early-write constraints keep inputs,
load results, and scratch distinct; ordinary definition interference also keeps
dead scratch separate from the result. Selection and physical replay retain the
exact width and scratch identity instead of borrowing a hidden fixed register.
Empty physical values, stack/indirect owned arguments, and
hidden-pointer results remain explicit transport limits. Zero physical
size never erases the semantic array type. The native differential
`scalar_array_results` target covers full publication and matching-host execution;
its packed cases cover 3/5/6/7-byte tails in one- and two-fragment calls and returns
on the three direct-register targets. Its floating cases preserve binary32/binary64
payloads, including signed zeros and NaN payloads. Array results keep their
integer-fragment aggregate ABI; they are
not foreign C homogeneous-floating aggregates. Scalar floating inputs use the
ordinary float-bank transfers before bit-preserving graph and array storage.
Calls to array-producing machines reuse the mixed-bank argument constraints,
with integer result definitions appended and independently checked by catalog
and encoding readers. The floating regression exercises the complete caller.
Narrow scalar calls normalize their exact signed or unsigned 8/16/32-bit carrier
before whole-register consumers; raw array fragments need no such scalar
interpretation. Computed Boolean comparisons establish canonical full-register
zero/one values before array stores, calls, or returns consume them. Their exact
Boolean carrier remains distinct from the integer carrier sharing its ABI shape.

Multi-case dispatch uses ordinary comparisons and explicitly identified
`CaseDispatch` continuation blocks. An unsuccessful comparison has taken no
semantic edge and charges none; the chosen edge carries its exact case payload,
destination, and once-only fuel through the ordinary edge bridge. Constructors,
calls and returns preserve declaration identity rather than inferring a field
from its name. These paths share allocation, frame, encoding and publication;
independent selection replay checks their complete instruction and storage roster.

Unit graphs use this same transport for `u8`, `u32`, `u64` and `i64` block
arguments. Materialized Boolean block arguments use it in both Unit and
scalar-result graphs. Exact block/value/type references are
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
block/value/type, and edge cleanup must agree. Hosted byte results and ordinary
scalar sums lower through frame-address, tag-load, compare and branch
instructions. Used integer payloads load only in the chosen edge's implementation
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
Non-scalar case layouts and executable or residual return cleanup remain separate
admission limits; no new ISA case opcode or physical route is introduced.
Comparison value materialization uses the existing instruction sequence: a
comparison establishes flags and the immediately following condition materializer
defines the Boolean value. Independent replay checks condition, operands, result,
flags, and the single source-operation fuel charge. Branch-only folding requires
exclusive consumption of the complete predicate suffix, including successor
arguments; a shared value retains its ordinary register definition.
Boolean equality retains Boolean operand types through comparison and independent
replay; it uses normalized register values without admitting Boolean ordering.

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
is formed on entry to that block without reading uninitialized contents. Future
descriptor addresses therefore need not survive earlier calls; edge bridges
address destination slots directly. The chosen
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
Materialized Boolean reads retain their exact scalar home across stores, calls,
and branches. A branch tests the fresh read's Boolean carrier using the ordinary
zero comparison; it does not substitute the initializer or a previous observation.
Definition availability, readable access, and source/home identity remain checked
independently of the comparison-only predicate suffix rules.
Construction and replay retain AddressLocal, WritePlace and ReadPlace records,
exact scalar demand, fuel, and the incoming parameter roster. Local storage does
not add an ABI parameter or a synthetic aggregate.

The ordinary control graph also composes Boolean and fixed-integer primitive
writes with a 64-bit integer result. Independent input replay joins the complete mixed ABI,
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

Register aggregate results compose with these same mixed input banks. Their
constraint rows reuse Unit-call inputs and append integer result definitions,
removing only those result write units from the unknown clobbers. Physical
operands are ordered GPR inputs, IEEE inputs, then result fragments; the authored
argument roster and each ABI's fixed positions remain unchanged. This is the
integer-fragment Omega array ABI, not a foreign homogeneous-float aggregate ABI.
The `scalar_array_results::floating` native regressions exercise construction,
owned forwarding, interleaved input banks, and two-fragment transport.

Integer and Boolean stack arguments use the same transport. Incoming loads and
outgoing stores preserve the exact 1/2/4/8-byte payload width independently of
the target's stack-slot spacing and alignment. Signed narrow stack loads are
sign-extended after the raw load; unsigned and Boolean loads already zero-extend.
Register parameters and ordinary scalar call results likewise normalize the
complete GPR from their exact fixed-width type before comparisons or forwarding.
`selection/scalar_call_abi.rs` selects the width and signedness; independent entry
and call replay requires that exact operation. This normalizes unspecified ABI
bits, not the source type or mathematical value, and charges no additional Psi
fuel. Already-admitted exact casts normalize their destination carrier without
changing proof or conversion admission. Replay rejects substituted widths, ABI
offsets, argument homes, and source identities, including when a borrowed pointer follows the
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
instruction or forced stack home is needed. Proof-bearing exact casts retain
their accepted obligation in ordinary control graphs, independently replay the
source/result types, and normalize narrow results by exact width and signedness.
Supported casts use fixed 8/16/32/64-bit carriers. Signed-to-signed casts involving
sub-64-bit carriers and widening from a 16-bit source still reject under the
existing conversion admission rules. Extending those rules requires checking
the complete producer/consumer path; adding ABI normalization alone does not
admit another conversion. Sign-changing casts retain the proof that their
mathematical value is nonnegative.

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
