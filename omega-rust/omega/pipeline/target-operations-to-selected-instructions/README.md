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

Structural argument snapshots, loads/stores, frame addresses and calls use
ordinary virtual instructions with exact slot/access/call metadata. Copying an
owned referent into a distinct ABI temporary and forwarding a borrowed pointer
are different transports. Claim completion can retain evidence at an instruction
position without inventing executable work. Frame-free bodies must not acquire
storage merely because their parameters are structural; actual outgoing and
preservation storage must be replayed against the realized frame.

Straight-line Unit field stores through mutable or write-only parameters retain
the original incoming pointer. Boolean and 8-, 16-, 32-, and 64-bit integer
stores carry the exact destination declaration, carrier path, scalar field and
typed SSA source. Input-only layout replay reconstructs the offset and width;
selection emits one pointer store with a write footprint and the source fuel
charge, without loading the destination. Independent replay checks the pointer,
value, write extent and charge before accepting the selected program.

Literal and called subslice descriptors use activation-local homes, separately
from ABI argument-copy slots. A subslice retains the original backing plus
exact integer offset and length; only a view used as a call argument acquires
an addressable 16-byte descriptor. Repeated calls forward that descriptor,
never a copy of its bytes. Producer/result identity, bounds and selected SSA
dominance remain required, including across guarded blocks.

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
