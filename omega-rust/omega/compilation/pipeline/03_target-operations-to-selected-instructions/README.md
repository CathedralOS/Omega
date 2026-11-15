# Instruction selection

This stage turns target operations into machine instructions.
[lib.rs](src/lib.rs) runs required legalization and instruction selection. It
takes the complete admitted target-operation program and the
validated target register environment, and returns the admitted selected
program with its semantic, target, projection, work and constraint
identities. Raw selected data or a detached content hash grants no authority.

## Target setup is not a program stage

The [register environment](../../backend/register-environment/src/lib.rs)
combines the native target, the ISA's physical model, instruction constraints,
selected keys and the active register-reservation profile. Baseline and
decoded/custom inputs each get their own structural and ISA checks. Selection
receives the environment explicitly; it must not import ISA crates or build a
supposedly equivalent one.

## Ordinary selected control flow

[Legalization](src/legalization.rs) and [selection](src/selection.rs) keep
ordered instructions, block parameters, explicit jumps, branches and returns,
virtual registers, fixed ABI constraints and machine effects. List order is
not control-flow order. Arithmetic and calls use the same graph; physical
registers, liveness, frame storage and emission come later.

[Place declarations](src/legalization_scalar_graph.rs) link each runtime place
to the parameter, result or local that produces it. Primitive locals, byte
literals and aggregate results combine without a whole-function
storage-family classifier. Declaration ordinals count declarations, not
instruction positions. The whole unit keeps dominance and ownership;
selection's function entry captures the incoming ABI, and call and storage
instructions use the addresses, homes and placements the target stage kept.
Instruction building does not rebuild reference custody. The explicit
`verify_target_operation_plan` entry point keeps the independent source/target
audit; focused native tests run it on the object's retained pipeline input,
outside the default compile path.

Arguments to direct, normalized foreign and no-normal-return calls use the
retained target selector rows, including the bound and obligation key. Proof
lookup uses the admitted immutable fact catalog, not an executable body. The
independent audit still derives source geometry and available scalar sources,
and rejects changed bounds, proof keys, strides or scalar correspondences.
Recovery encoding stores these fields with the argument. A direct call is
built from only its machine ID, target row and admitted fact slice, not earlier
function or unit bodies.

Function headers likewise use the retained target storage contract, scalar
ABI parameters and entry block, and scalar definitions follow their parameter
ordinals. The audit compares the whole structural contract, claims and
service ceilings included, to the source. Other ordinary instruction building
still reads earlier bodies; that handoff is unfinished.

[Control-exit construction](src/legalization_control.rs) uses only target
terminators, retained storage declarations and admitted proof facts. Only the audit uses the earlier body's terminator, as a reference for whole
exits including work, ownership, cleanup and successor bindings. Return,
branch, sum dispatch, crash and no-normal exits all work this way, with no
executable bodies recovered during ordinary building.

Common instruction annotations also come from the target row, not the old
optimization node. A missing annotation is refused even if that node is still available. The audit compares the exact
result definition, logical work, effect order and custody events and reports
which annotation differs. A block's parameter definitions use its retained
parameter values and ordinals. Building operation kinds still reads earlier
bodies; the annotations do not hide that.

Every target function holds its `TargetControlGraph` directly; this stage
converts no flat bodies or whole-function expression trees. Unit, scalar and
aggregate results share construction and independent replay, and projected
structural returns have no second function list. Natural-ranked loops keep
their ordinary proof obligations on the same route. Older unsigned-countdown native custody is rejected, not let skip verification.

Each instruction records its operation, obligation, value, definition and work
provenance. Successor bindings keep semantic identities and explicit register transport,
with exact edge, polarity, target and taken-edge work. Fixed
views are constraints, not assigned registers. ISA flags and program counters
(RFLAGS/RIP, NZCV/PC) do not become fake source values. Compiler-added condition tests, copies and address
arithmetic invent no Psi operations or charges. Returns keep their Unit/scalar/aggregate role, full result
constraints and edge work.

Unsigned 64-bit saturation keeps its arithmetic policy on this graph. Exact
division and remainder accept fixed 8/16/32/64-bit signed and unsigned
carriers and keep the full definedness obligation: a nonzero divisor and a
representable quotient, so signed `MIN % -1` is rejected. Normalized operands
use the existing Exact signed division instructions, except full-width
unsigned operands, which keep the unsigned form. On x86-64, preparing the
zero or sign high half keeps explicit scratch constraints and adds no source
operation or work charge.

Integer arithmetic copies each fixed source `Use` into a value local to the
instruction, as the selected target constraint row requires. A reused source
value therefore keeps its flexible range across later calls, and equal source
operands can reach different required registers without a special case. The
copies keep source identity and definition custody; the arithmetic
instruction keeps its operation, definedness obligations and work.
Instruction-local scratch, including unsigned division's explicit zero high
half, keeps its lineage and fixed constraints. Scratch definitions, early
clobbers and physical-register checks stay explicit constraints for later
stages. An early-clobber output also needs distinct physical inputs, so a
repeated source operand gets a separate copy from the same constraint row,
even on targets whose operands have no fixed register.

Fresh sum values use carriers local to the activation. Construction initializes the whole carrier, padding and inactive
payload bytes included, then writes the declared tag and exact-width scalar
fields, or copies whole owned structural children through the same field
transport as records. Case and field identities, recursive payload geometry,
scalar range obligations and child sources survive legalization and
independent replay. The carrier-address instruction carries the single
source-operation charge; the other initializing instructions add none.
Calls capture every direct result fragment into the caller's result slot;
returns load every fragment under the same ABI plan. Result definitions are not also unknown call clobbers. Direct carriers
cover one or two register fragments. Indirect results use the calling plan's
hidden pointer: RCX/RDI on x86-64 and X8 on AArch64. Function entry captures
that pointer before any call; the caller reserves result storage before the
call and publishes its initialized home afterward. x86-64 returns the pointer
in RAX. Independent replay checks the hidden input, shifted ordinary
parameters, result storage, exact writes and return convention.

Plain record construction uses the same aggregate homes. Each initializer is
matched to its declared field, recursive layout, scalar value or completed
owned child, and range obligation. Selection initializes padding and copies
nested children at exact widths before publishing the result address. Child
storage may come from an operation result or from an owned entry parameter
whose ABI storage is already captured; a borrowed operand cannot stand in for
an owned child. Records arriving at a block use the existing aggregate edge
transport with recursive payload geometry. Shared projected calls borrow that
block's destination home, not an incoming producer or a byte-view descriptor.
Independent replay rebuilds field placement, child sources, stores and the
single operation charge.

Scalar field reads keep the readable parameter, nominal field, result type and
original operation. Each read produces a scalar SSA value before any later call
or store. Target and selected replay rebuild the field offset and exact-width
load from the declaration; a field of the same type or a later reload cannot
replace the recorded read.

Owned record inputs passed in registers that need field reads, and record or
sum inputs lent to calls, use one `StructuralParameter` local slot keyed by the
incoming place. Function entry stores every captured register fragment before
authored operations. Field reads and borrowed calls use that slot; later whole
returns or nested copies read its current bytes, not stale entry fragments. A
borrowed input still refers to its original value and is never copied into this
slot. Register inputs used only to move the whole value keep fragment-only
storage. Replay rebuilds the slot, its extent, writes and every read. Shared
calls on constructed or call-returned sums likewise use their original
operation-result home; being copyable does not permit lending a payload
snapshot.

Inline stack inputs already own storage for the whole activation. Entry keeps their incoming frame address, and later reads, calls,
nested copies and whole returns read the current bytes. Capturing every fragment up front would add needless register pressure across calls.
This storage is the ABI's copy, not the caller's original; no extra copy is
needed.

Indirect owned record inputs instead keep the pointer to the ABI-prepared
copy, with no second copy or borrowed access. Entry captures the
pointer bits from the register or incoming stack slot before authored
operations. The caller's payload-copy offset is not the incoming pointer-slot
offset. Field reads and nested constructor operands use this backing, even with no
earlier read. Published pointer homes keep owned and borrowed
identity apart; the full call plan and graph replay are still required.
Returning the whole parameter uses the same backing under its own selected
result ABI, hidden result storage included. Ordinary calls forward the backing
through the callee's own selected argument ABI. Indirect arguments copy the
payload into an outgoing `ValueCopy` slot and pass its address in the retained
register or `Argument` stack slot; the slot roles tell payload and pointer
apart without inventing another argument ordinal. Microsoft x64 payload copies
keep 16-byte alignment; odd tails use the same exact-width loads and stores as
direct aggregate fragments. Construction and receiving replay rebuild these transports.

Whole arrays of records use the same recursive size, alignment and aggregate
homes as records. Calls and returns respect padded element strides and field
offsets; they need no primitive leaves and do not imply element construction
works. Replay derives the full layout and argument/result ABI again.

Primitive arrays share that aggregate storage and call/return path, without a
sum tag. [Array input](src/selection_inputs.rs) rebuilds the declared
dimensions and leaf carrier, and constructor selection writes each row-major
leaf at its exact width. [Aggregate results](src/selection_inputs.rs) joins
array and sum homes to their retained calling plans. Replay checks every
store, result fragment and the single constructor work charge.

- Owned arguments load those homes or copy captured incoming
  fragments; returning an incoming array keeps its parameter identity. Each
  argument is matched to its producer, place, type and destination ABI,
  mixed borrowed arguments included. Inline stack arguments use one
  contiguous outgoing slot per call and argument ordinal. Entry keeps the
  incoming copy's address for the whole activation; only the frame encoder
  adds the incoming frame and return-address offset. Register operands
  include only fragments held in registers. Where an input lives does not
  change the result ABI checks. Inline values keep full graph replay at
  publication, not pointer-only legacy records.
- Unit and scalar-result calls use the same full list of structural
  arguments: each declaration ordinal maps to its absolute ABI parameter
  ordinal after the scalar parameters. Matching argument count alone admits
  nothing. Projection and replay rebuild the callee's full plan and check
  each argument's type, access, producer and placement; an owned value beside
  a borrowed output is ordinary, not a separate kind of call.
- Direct fragments are 1 to 8 bytes wide, in up to two
  registers; Microsoft x64 supports its single-register direct results. Odd
  widths use `LoadPacked`/`StorePacked` with an explicit scratch register
  local to the instruction. The ISA expansion touches each meaningful byte
  exactly once and never rounds the extent up to a machine word. Early-write
  constraints keep inputs, load results and scratch apart; ordinary
  interference keeps dead scratch apart from the result. Selection and
  physical replay keep the exact width and scratch identity rather than
  borrowing a hidden fixed register.
- Empty arrays keep their full type and `Some(empty)` ABI placement but
  have no payload slot, address, register or memory access. The constructor's
  operation and work provenance is attached to the next instruction in the
  same block, even a terminator, and replay rebuilds that prefix. Calls and
  returns with empty results reuse physical Unit instructions without
  changing their structural contracts. Indirect owned entry arguments keep
  their incoming copy's pointer, as records do. Zero physical size never
  erases the semantic array type. Admitting inline stack values does not
  claim allocation can handle every graph.
- Array results keep their integer-fragment
  aggregate ABI; they are not foreign C homogeneous-floating aggregates.
  Scalar floating inputs use the ordinary float-register transfers before
  bit-preserving graph and array storage. Calls to array-producing machines
  reuse the mixed-bank argument constraints, with integer result definitions
  appended and checked separately by catalog and encoding readers; the
  floating regression test covers the whole caller. Narrow scalar call
  results normalize their signed or unsigned 8/16/32-bit carrier before
  anything reads the whole register; raw array fragments need no such step.
  Computed Boolean comparisons produce a canonical full-register zero or one
  before array stores, calls or returns use them, and the Boolean carrier
  stays distinct from the integer carrier with the same ABI shape.

Multi-case dispatch uses ordinary comparisons and explicitly marked
`CaseDispatch` continuation blocks. A failed comparison takes no semantic edge
and charges nothing; the chosen edge carries its case payload, destination and
one-time work through the ordinary edge bridge. Constructors, calls and returns
keep declaration identity instead of guessing a field from its name. These
paths share allocation, frame, encoding and publication, and replay checks
their full list of instructions and storage.

Unit graphs use the same transport for `u8`, `u32`, `u64` and `i64` block
arguments, and materialized Boolean block arguments use it in Unit and
scalar-result graphs. A block, value or type reference is available only in
its defining block and the successors it dominates; each arrival keeps its own
ordered bindings, even when both arms of a branch go to the same block. A
transferred integer does not inherit byte-length facts. Selection expands
edge bindings into explicitly marked implementation blocks. Each chosen edge
snapshots all arguments before copying them into the destination's transfer
registers, so separate arrivals need not share homes and parallel swaps never
overwrite an unread argument. Liveness, allocation and encoding handle these copies. Replay checks the expansion and folds it back into the
source graph, keeping the authored edge and its work once; the final
implementation jump records lineage, not a second semantic transition. No
source block or operation is invented.

Owned arrivals use ordinary storage, ABI and cleanup contracts even with an
unread payload. Each successor declaration has its own home; each
binding picks its transport from its layout and where its source lives, not
from a list of allowed operations in the function. Zero-byte owned bindings
keep `SelectedStructuralTransport::Unused`; nonzero values use the ordinary
whole-value bridge. Optional optimizers may remove unused physical work only on that general
route. Inline call values may stay in ABI
fragments when no operation needs an address; indirect inputs keep their
pointer-location records at object/image publication. Bounded integer fields
keep their integer carrier layout and full range declaration. Layout grants no
ownership or permission to write restricted fields, and the ordered cleanup
action list keeps executable hooks and exact no-code discards.

Plain-owned scalar sums that are read use
`SelectedStructuralTransport::WholeValue` with the source pointer, destination
block-parameter slot, byte extent and alignment. The edge bridge snapshots
scalar arguments, borrowed descriptors and whole owned values before any
destination is overwritten. Whole-value copies use existing loads and stores, narrow final chunks
included, with no per-case instruction. Replay checks
every snapshot, address, store and memory record against the semantic binding
and destination layout. The chosen edge keeps its single work and ownership
transition; its implementation continuation adds neither.

Target-input matching also compares `StructuralCase` terminators in `TargetControlGraph` with the validated abstract graph: the
dominating operation-result or block-parameter home, declared case order and
tag, field offset, destination block/value/type and edge cleanup must agree.
Hosted byte results and ordinary scalar sums lower to frame-address, tag-load,
compare and branch instructions. A used integer payload is loaded only in the
chosen edge's implementation block, before the destination binding; an unused
payload keeps its semantic metadata with no load or bridge. Structural reads
get no invented source value or definition site. Only the semantic edge
carries no-code affine cleanup and work; its implementation continuation keeps
payload lineage and register transport, with empty cleanup and work. Selection
and bridge replay restore the exact legalized case, including the definition
the destination owns. Returning blocks keep exact no-code discards of their
own affine byte-read results. Ownership replay for the current unit rebuilds
each live frontier; target and legalized replay keep its ordered actions,
return edge and work. A function-wide result list cannot replace per-branch
ownership. Non-scalar case layouts and executable or residual return cleanup are not yet
admitted; no new ISA case opcode or physical route is added.

A comparison value uses the existing sequence: the comparison sets flags and
the condition materializer right after it defines the Boolean value. Replay
checks condition, operands, result, flags and the single work charge. Folding
a comparison into a branch requires that nothing else uses the predicate
suffix, successor arguments included; a shared value keeps its ordinary
register definition. Boolean equality keeps Boolean operand types through
comparison and replay; it uses normalized register values without admitting
Boolean ordering.

Natural-ranked Unit writers and integer-result loops use this graph, including
loop-carried descriptor arrivals, hosted byte output, selected scalar calls
and scalar returns; unranked loops use the same route. Legalization borrows
the verified artifact from the validated abstract stage and replays its
current components. The optimizer fixes the whole Natural or unranked looping
function, prefixes and exits included, against the verified source; it
produces no countdown certificate or fixed-work bound, and raw looping units
cannot replace that custody. Block layout uses reverse postorder when
backedges prevent a topological order; layout grants no termination
authority.

Demand for physical scalar registers starts at retained instruction operands,
branch conditions and return values, then follows incoming edge bindings.
Closed cycles of unused bindings need no registers or copies, and no
instruction, call, semantic binding or work charge is dropped. Construction
computes demand once with a backward worklist; replay follows each declared
value forward to a reader. Expected-set tests cover unused cycles, live
multi-edge chains, swaps and kept instructions with unused results.

Whole, unqualified, unrestricted shared or mutable byte views use the same
edge bridges. Each block parameter owns a 16-byte descriptor slot in the
activation, whose address is formed on entry to that block without reading
uninitialized contents. Descriptor addresses therefore need not survive
earlier calls; edge bridges address destination slots directly. The chosen
edge snapshots both words of every incoming descriptor and all scalar
arguments before overwriting any destination, which keeps parallel swaps and
earlier aliases correct when a producer's slot is reused. Backing bytes are
never copied. Replay rejoins the source place, destination block and place,
offsets and edge; memory metadata tells operation, block-address and edge-copy
origins apart. Source-graph availability, not address-register
dominance, shows a descriptor is initialized. Length
and bounds facts belong to the newly bound destination view, not to some
predecessor's scalar length.

Indexed writes through a mutable view load the original backing pointer, form
the checked byte address with `ByteViewAddress` and use the ordinary one-byte
`Store`. The write footprint keeps the destination, index, byte, current
length, obligation and accepted fact. Replay checks the whole sequence; only
the store carries the source operation's work. Descriptor words and bytes
outside the element stay unchanged. Mutable subslices are separate work.

Bounded inline byte fields reuse that one-byte store, adding their statically
rebuilt payload offset to the runtime index. The original writable record
pointer stays the backing; no view descriptor, length publication or read of
displaced bytes is needed. Current-IR validation requires a same-field live-length read that stays fresh across every arrival and
backedge; capacity alone cannot justify an index. Terminal replay keeps the
authored field and operands, and selection replay rebuilds the offset,
single-byte footprint and committing work charge. Replacing a whole field
still copies only live source bytes before publishing the length through the
existing `CopyBytes` route.

Ordinary Unit calls may pass an initialized raw fixed-u8 array as a mutable
byte view, keeping the whole source or exact field-only path. Receiving checks
rebuild the array extent and offset from declarations; scalar-result calls
stay excluded. Selection stages only a two-word local descriptor holding the
original backing pointer and the static length. Existing frame stores,
addressing and call transport carry it, outgoing stack pointers included. No
backing bytes are copied, preparing the descriptor adds no work charge, and
repeated calls keep the original root pointer. Image and installation custody
rejoin the semantic path, array metadata and selected/frame replay; the
descriptor's shape alone grants nothing.

Structural argument snapshots, loads, stores, frame addresses and calls use
ordinary virtual instructions with exact slot, access and call metadata.
Copying an owned value into a separate ABI temporary and forwarding a
borrowed pointer are different transports. Claim completion can record
evidence at an instruction position without inventing executable work.
Functions without a frame must not gain storage just because their parameters
are structural; actual outgoing and preservation storage must be replayed
against the realized frame.

Straight-line Unit field stores through mutable or write-only parameters keep
the original incoming pointer. Stores of IEEE binary32/binary64, Boolean and
8-, 16-, 32- and 64-bit integers carry the destination declaration, carrier
path, scalar field and typed SSA source. Layout replay from the inputs alone
rebuilds offset and width; selection emits one pointer store with a write
footprint and the source work charge, without loading the destination. Replay checks pointer, value, write extent and charge.

Whole primitive stores use the same exact-width pointer instruction and keep
`WriteOnlyPrimitiveStore` as their semantic origin. The destination must be an
unrestricted, unqualified, claim-free mutable or write-only parameter whose
referent has exactly the SSA source's primitive scalar type. Boolean, fixed
8/16/32/64-bit integers and IEEE binary32/binary64 are supported; no synthetic
record, field, readable borrow or IEEE-to-integer conversion is added.

Projected primitive stores can also target established owned aggregate homes.
They keep the producing operation and structural result, never an invented
ABI parameter. Receiving checks rejoin that identity and the full projection;
selection reuses the aggregate's local pointer and checks the exact-width
store on its own. Linear or qualified homes are excluded.

A scalar field store may also write an owned structural block parameter: the
arrival into which every incoming edge moved its whole value. Its destination
keeps the owning block and declaration, and its storage is that block's own
`StructuralBlockParameter` slot, whose address the block entry publishes for
both reads and stores. A lender's producer home, another block's arrival, a
loan or a shared declaration is refused. Primitive leaf stores into such an
arrival are not admitted yet.

Primitive locals use activation slots identified by operation and place.
Each establishment forms the frame address and does one exact-width
initializing store. Re-entering a loop reuses the slot and
runs initialization again; the store is not hoisted to function entry. Later
stores and borrowed Unit/scalar calls use that original pointer, and a scalar
call keeps its own result.

- Fresh primitive reads use pointer loads and distinct SSA
  definitions. `Load8`, `Load16`, `Load32` and `Load64` read exactly 1, 2, 4
  or 8 bytes for Boolean, fixed-width signed/unsigned integers and IEEE
  binary32/binary64. Narrow loads zero-extend into the general-purpose
  register (GPR); signed narrow reads then restore the sign with the same
  normalization as incoming ABI values, before anything uses the result. Only
  the load charges the read's work; normalization neither widens the memory
  access nor changes the type. IEEE loads keep the bits without
  floating-point arithmetic.
- A materialized Boolean read keeps its scalar home across
  stores, calls and branches. A branch tests the fresh read with the ordinary
  zero comparison, never the initializer or an earlier read. Definition
  availability, readable access and source/home identity are checked
  separately from the comparison-only predicate suffix rules.
- Construction and replay keep AddressLocal, WritePlace and
  ReadPlace records, exact scalar demand, work and the incoming parameter
  list. Local storage adds no ABI parameter or synthetic aggregate.

The control graph also combines Boolean and fixed-integer primitive writes
with Boolean and fixed 8/16/32/64-bit integer results. Input replay joins the
full mixed ABI, the incoming reference, the ordered store source and the
scalar return. Selection uses the existing pointer store and result
constraint, adding no Unit wrapper or legacy store-byte records.
Object/image publication keeps the mixed ABI and incoming borrowed homes
under the mandatory common-pipeline replay.

Runtime IEEE Unit arguments use the target's floating registers or
exact-width stack slots. Explicit raw-bit transfers connect floating ABI
views to ordinary GPR storage, and the values keep IEEE types. Mixed call
constraints keep the exact GPR and floating operand views, including
Microsoft x64's positional gaps. Incoming scalar stack loads and outgoing
stores use private scalar ABI addresses, not invented places. Replay derives
source, format, placement, width and call order again. Runtime preservation
spills keep the IEEE type and use integer-typed slot addresses; floating
registers never go through GPR spill instructions.

Aggregate results in registers combine with the same mixed input registers.
Their constraint rows reuse Unit-call inputs and append integer result
definitions, removing only those result registers from the unknown clobbers.
Physical operands are ordered GPR inputs, IEEE inputs, then result fragments;
the authored argument list and each ABI's fixed positions are unchanged. This
is Omega's integer-fragment array ABI, not a foreign homogeneous-float
aggregate ABI.

Integer and Boolean stack arguments use the same transport. Incoming loads and
outgoing stores keep the exact 1/2/4/8-byte width whatever the target's
stack-slot spacing and alignment. Signed narrow stack loads are sign-extended
after the raw load; unsigned and Boolean loads already zero-extend. Register
parameters and ordinary scalar call results likewise normalize the whole GPR
from their fixed-width type before comparisons or forwarding.
[`selection_call_abi.rs`](src/selection_call_abi.rs) picks the width and
signedness, and entry and call replay require that exact operation. This
cleans up bits the ABI leaves unspecified; it changes neither the source type
nor the value and charges no extra Psi work. Already-admitted exact casts normalize their destination carrier without changing proof or
conversion admission. Replay rejects substituted widths, ABI offsets,
argument homes and source identities, also when a borrowed pointer follows
the stack-passed scalars.

Wrapping addition uses the ordinary scalar graph for signed and unsigned
8/16/32/64-bit integers. `WrappingAddI64` reuses the target's add encoding and
register constraints but keeps its own semantic identity: it is not an Exact
add with invented no-overflow evidence. Sub-64-bit results go through the same carrier normalization before their SSA
definition is available. The raw sum stays private; later comparisons, stores and calls see
only the normalized value. Replay checks that sequence and charges the
operation's work once, on the add. Target replay also rejects recomputing an
earlier expression instead of using its result home. Borrowed record
parameters keep their pointer ABI when a function returns a scalar; field
read/write permission is checked at the operation, not inferred from the
return shape.

IEEE literals in field stores and Unit calls keep their defining operation,
SSA value, format and raw bits as ordinary scalar sources. Receiving replay
makes the value available only after matching the preceding constant
operation. The existing raw-bit materialization and floating ABI transfers
then produce the value, with no numeric conversion or separate call or store
instructions.

Literal and transported subslice descriptors use homes in the activation,
separate from ABI argument-copy slots. A literal's home holds only its 16-byte
descriptor: `ImageAddress` names the bytes' read-only image storage by the
operation and place that created it, and image emission binds that address to
the literal pool, so a copied literal descriptor outlives the activation. A
subslice keeps the original backing plus its exact offset and length; a
subslice passed as a call or block argument gets an addressable 16-byte
descriptor. Repeated calls forward the descriptor, never its bytes. Producer and result identity, bounds, availability of the source place
and the selected address's lifetime are still required, also across guarded
blocks.

Building a collection keeps the carrier's address separate from its backing.
An empty structural path selects the carrier; collection geometry loads the
backing pointer once when that storage is borrowed. A child projection starts
in the root's element storage before applying its offsets. Always resolving a
view projection first would load the pointer twice and treat payload bytes as
an address. Construction and replay keep this distinction.

On the current 64-bit targets, `ByteViewAddress` forms the private
descriptor's address bits without claiming exact source integer arithmetic.
Valid root geometry `B + R <= Bound` and view geometry `O + L <= R` imply
`B + O <= Bound`, and equality forces an empty view. A one-past-the-end
address at `2^64` therefore becomes zero pointer bits with zero length; it is
neither an overflow proof nor permission to read address zero. Other empty
views need not form a dereference either. The original bounds and backing
still decide access, and this address arithmetic adds no Psi operation or
work charge. See [extent endpoints](../../../../../wiki/spec/resources/extents.md#conservation-and-loans).

The admitted Linux `write_byte(i32)` boundary call keeps its SSA input and
builtin identity in the same graph. Its selected form owns one byte of local
scratch named by the boundary operation, not an invented structural place.
The ISA code stores the low byte, makes the existing stdout write and
continues only on a positive result, else traps. Registers, flags, local
memory and the external effect are explicit, and the form does not move the
stack pointer behind frame allocation's back. Independent decoding checks the
whole syscall sequence and the resolved slot. Object evidence tells this
input transport apart from legacy immediate materialization. This Linux
version does not authorize a macOS or Windows provider or establish writer
control flow.

Hosted `exit_process(i32)` uses the ordinary Unit scalar graph, not a
separate literal-only target operation. Its last boundary node becomes an
explicit process-exit terminator on Linux x86-64/AArch64 and macOS AArch64.
The empty Unit return after it in the source stays as a formality: its edge is
kept, but its work is not charged as executed, and no native return or
epilogue is emitted. The terminator reads the original runtime i32 carrier,
needs no scratch and traps if the kernel returns. Source, target, builtin,
operand and whole exit span are replayed through publication.

Ordered Unit scalar definitions are matched to their source operations before
entering the same legalized SSA graph. Total widening between fixed
8/16/32/64-bit carriers reuses the existing sign- and zero-extension
instructions, chosen by source width and signedness. Signed-to-unsigned
conversion is not total and does not take this route. Result type, defining
operation, source identity and value location are replayed independently; no
new instruction or forced stack home is needed. Proof-bearing exact casts keep their accepted obligation in ordinary control graphs, replay the source
and result types, and normalize narrow results by width and signedness.
Supported casts use fixed 8/16/32/64-bit carriers. Signed-to-signed casts involving sub-64-bit carriers, and widening from a
16-bit source,
are still rejected by the existing conversion admission rules. Extending those
rules means checking the whole producer-to-consumer path; adding ABI
normalization alone admits no new conversion. Sign-changing casts keep the
proof that their value is nonnegative.

[Construction](src/selection.rs) and independent validation compute separate
projections. Replay checks the whole selected program against the
semantic/optimized input, target plan and register catalog, including exact
instruction order, ABI copies, call slots, memory accesses and settlements
that emit no instructions. A producer's success does not validate it.

Newly supported behavior must enter the common graph with
exact ABI, memory, ownership, proof and frame replay. Unsupported shapes are
rejected by the stage that owns them; there is no fallback to an
already-assigned program. [Allocation](../05_selected-instructions-to-register-homes/README.md)
owns register homes and recovery, and [machine emission](../../backend/machine-emission/README.md)
owns the physical steps after that.
