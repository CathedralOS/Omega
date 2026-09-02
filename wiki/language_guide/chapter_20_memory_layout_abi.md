# Chapter 20: Memory Layout And ABI

Memory layout is part of the contract between Omega, native code, wire formats,
drivers, inline assembly, and generated machine bytes.

## Zeroed Storage And Establishment

Omega guarantees that the ALL-ZERO BIT PATTERN is a safe storage state for
every checked-shape `data` type. It never creates an invalid machine
representation, undefined behavior, or memory-safety hole. Whether those bytes
already establish an accessible value of the type is a separate, derived
judgment.

```omega
data Inventory {
    gold: u32;          // zero: 0 gold
    items: [Item; 8];   // zero: 8 zeroed items
    label: [u8; 64];    // zero: 64 zero bytes
}
```

What makes this hold layer by layer:

- Integers, floats, and booleans: zero is an ordinary value.
- Fat descriptors (slices and borrowed text windows): `{ ptr: 0, len: 0 }` is
  the canonical empty carrier. Reads see emptiness; nothing dereferences a
  zero pointer with a zero length.
- Raw fixed arrays `[T; N]` contain exactly `N` inline elements and no hidden
  live-length word; all-zero storage recursively zeroes every element. A
  bounded live-length collection is a distinct ordinary record such as
  `FixedVec { items: [T; N], length }`, whose zeroed length is zero.
- Case-bearing data (sum and mixed shapes): tag `0` is the first declared
  case, so a zeroed value IS the first case (with zeroed payload if it has
  one). A payload-bearing first case is ordinary: zero may be `Integer(0)` or
  another fully established payload value (see
  [Case Members](chapter_1_data_values_literals.md)).
- Aggregates: zero recursively zeroes every field, so the guarantee composes.
- The compiler never performs niche-style layout optimization that gives the
  zero pattern a different meaning or makes it unrepresentable.

Invariants and domains describe ESTABLISHED values, not raw storage. The
compiler derives zero establishment by evaluating the default domain at zero
and recurring through common fields and the first case's payload. A field
contracted to `1..=100`, an obligation-free default fact, or a zero-reachable nested
type with the same gate prevents establishment. Later sum cases do not matter
because their payload is inactive at tag zero.

A type whose default domain excludes zero is therefore GATED: all-zero remains
safe storage, but code cannot observe it as a `T` until construction or
qualification establishes the missing facts. Proof soundness comes from that
gate, never from pretending the bits cannot exist. See
[Chapter 7](chapter_7_types_constraints_invariants.md) and
[Chapter 12](chapter_12_dependent_types.md).

Data fields have no declaration-site default initializers. Non-zero or
computed construction belongs in ordinary constructor machines; zero-filling
continues to mean exactly the all-zero representation. It does not establish
authority, validation, history, or semantic emptiness.

Emptiness and reset behavior are ordinary authored semantics. A type that
needs them publishes a domain and a constructor or reset machine:

```omega
data Command {
    case None;
    case Say(text: [u8; 256]);
}

pub domain Command::Inert;

pub machine Command::empty() -> command: Command in Command::Inert {
    Command::None
}
```

Consumers that genuinely depend on inertness require `Command in
Command::Inert`. The compiler does not infer that semantic claim from a
payload-free first case.

### What a zeroed value is, concretely

A never-written place has exact representation semantics, identical in the
interpreter and native emission. Observation as `T` additionally requires the
derived zero-establishment judgment:

| Zeroed value            | Reads as                                            | Pinned by |
|-------------------------|-----------------------------------------------------|-----------|
| scalar field / element  | `0` (any width, any nesting depth)                  | `core/zii_default_composite_exit` |
| sum (`data ... case`)   | the FIRST case, with zeroed payload                 | `core/zii_default_composite_exit` |
| bounded text carrier    | live length `0`: `== ""` holds and `== "x"` is false without reading inline bytes | `text/zii_default_string_equality_exit` |
| bounded carrier at a host call | projects as an empty borrowed byte view       | `text/zii_string_host_write_exit` |

Two consequences worth designing around:

- The first `case` of a sum is its zero-representation state. Its zeroed
  payload must establish if code observes a newly zeroed value, but it need not
  be payload-free or semantically empty.
- Reassigning a sum from a longer case to a shorter one leaves the longer
  payload's tail bytes stale in storage. No language surface can observe
  them: destructuring reads only the active case, and synthesized equality
  compares the tag and then only the active case's payload
  (`traits/equatable_sum_stale_payload_exit`).

## Default Layout

Default data layout is compiler-controlled.

```omega
data Player {
    health: i32;
    gold: u32;
}
```

The compiler may choose field order, padding, and alignment unless a declaration
requests a stable representation.

## Stable Representation

Stable layouts are required at host, wire, and ABI boundaries.

```omega
repr native
data WinHandle {
    value: u64;
}
```

The exact spelling is provisional. The concept is not: if outside code observes
layout, the layout must be declared and checked.

## Alignment And Padding

Layout reports should include:

- field offsets,
- field sizes,
- alignment,
- padding,
- total size,
- target-specific assumptions.

These rows cover the erased-stripped runtime form. An `[erased]` binding
remains in semantic type identity but has no field offset, size, padding, or
transfer operation. A placement plan establishes any proof fact it carries
through its checked or admitted contract rather than pretending the fact
occupies hardware bytes.

The implemented native slice currently applies that rule to non-generic
transparent record, sum, and mixed common-field/case layout, plus closed
synthesized generic-record instances selected by explicit local, assignment,
unique free-call, or return destinations and exact closed pure or mixed generic-sum instances. It also applies to the
machine storage and runtime
contained-machine topology of closed, non-generic plain records when every
attached machine is an ordinary checked body with no unresolved machine
parameters. Erased common and payload fields are omitted without changing the
tag prefix, variant order, or case numbering. Ambiguous unresolved generic
uses, explicit placement plans, and attached
machines outside that exact checked-record cohort remain rejected until their
representation classifiers consume the same erased-stripped form.

For public ABI faces, fixed non-generic record parameters and results retain
their erased bindings in semantic and terminal-Psi identity but recursively
omit those bindings from the calling-policy shape graph, native aggregate
class, register/stack transfers, and result reconstruction. An all-erased record
still has no representable by-value ABI shape. Case-bearing and unresolved
generic aggregates remain outside the current public calling-policy shape
vocabulary independently of relevance.

Padding is not semantic data. Proofs and wire protocols must not rely on
uninitialized padding bytes.

## Fat Descriptors

Omega uses one representation model for fat descriptors and the pointer-based
carriers built on them: slices and text windows. The shape is:

```text
FatDescriptor {
    ptr;   // byte offset 0
    len;   // byte offset pointer_size
}
```

Total size is `2 * pointer_size` and the descriptor is pointer-aligned. For a
slice, `len` is an element count; for a text window, `len` is a byte count. A
kind tag distinguishes the two interpretations.

Owned and borrowed carriers share the identical in-memory layout. They differ
only by an ownership tag carried in the semantic spine, which records drop
responsibility, not by layout. A borrowed slice and an owned slice are byte-for-byte
the same `FatDescriptor` in memory.

Subslicing is expressed uniformly:

```text
new.ptr = base.ptr + start * element_byte_size
new.len = end - start
```

This descriptor shape is owned by exactly one crate, `omega-runtime-abi`, which
exposes field-offset accessors and a subslice accessor. `omega-layout` and
instruction selection are consumers: they must not re-derive the `+ pointer_size`
and `2 * pointer_size` layout independently. Owning the shape in one place keeps
descriptor layout from drifting between the layout pass and code generation.

A future growable `Vec` carrier `{ ptr, len, cap }` is the same model
parameterized by word count, so it stays under the same owner.

## Calling Conventions

Machine calls inside Omega use Omega calling rules — the internal convention
is compiler-sovereign, never stated, never observable, free to change any
release. Conventions exist only at **boundaries**.

As detailed in `design_briefs/calling_plans.md`, a calling
convention is a **layout over the register file + stack frame** and gets the
layout treatment — a per-ABI policy (stated or computed, audited against the
psABI document) produces a validated **CallPlan** from a signature: per-param
placements (`InReg`/`OnStack`/`ByPointer`), return placement, clobber set,
shadow space, stack alignment. Inbound entries additionally carry an independent
**StatePlan**: initial machine regime, interrupted state, save/restore policy,
and permitted transitive machine-state use. One evaluated boundary-entry plan
feeds both derivers — the outbound call encoder and the inbound entry stub —
so caller and callee agree by construction without confusing ABI placement with
interrupted-state preservation.

Opaque boundary data used by value must first resolve one exact target-closed
representation application. A named provider representation is selected during
build composition; a compiler-owned family such as `Ptr<T>` derives its
representation from pinned target semantics. The compiler derives the closed
shape and physical move/finalization plan from that carrier, then the calling
policy places it like any other sealed shape. The policy cannot inspect private
carrier fields or invent the ABI.

This demand is lazy: reference-only opaque pointees and proof-erased values need
no by-value representation. The exact application is part of the boundary
signature and `CallPlan` identity, and every producer and consumer must agree on
it. Agreement is scoped to the active compilation and to each actual future
independently compiled by-value composition edge; it does not unify historical
selections from unrelated package-as-root reviews. Representation does not mint
a valid value, establish a domain fact, or change the opaque declaration's
multiplicity or discharge rules.

The plan's `EntryStack` member selects the execution-stack disposition. It is
not a scalar claim that hardware arrival, adapter execution, and the machine
body all occupy that stack. External-root installation separately validates a
context-indexed sequence of entry epochs, with active-domain, per-domain
occupancy/alignment, and nesting evidence. This realization is provider and
installed-artifact evidence checked against the published `StatePlan`; it is
not another source signature or architecture-specific language construct.
The first UEFI consumer of this evidence derives the receiver-free x86-64
ProgramStorage wrapper's live-frame contribution from its exact installed
bytes, resolved private call, and canonical three-epoch occupancy. Equal raw
byte counts, missing generated origin, or epoch drift do not satisfy that
term; physical-entry and semantic-wrapper calling-plan commitments remain
distinct.

An outbound registrar plan may additionally contain private callback-
materialization rows. Each maps one nominal static-machine binder slot to an
explicit native-only callback parameter or to a field path through one
validated native layout. A static-machine parameter has no ABI ordinal of its
own. A direct destination is declared interleaved at its real position in the
registrar requirement's ordered native telescope. It has a nominal parameter
identity and target-closed function-pointer shape but no Omega runtime type,
value, or source-call argument. The requirement owns that parameter and one
declaration creates its typed demand; calling policy may place but not create,
reorder, or retarget it.

Nested callback fields are typed private layout demands absent from the
semantic data schema. A target package declares one as a named conformance such
as
`WndClassWindowProcedureSlot: WndClassLayout satisfies
PrivateCallbackSlot<WindowProcedure::call>;`; the layout plan explicitly cites
that evidence when it places the slot. The declaration is inert until cited,
so no ambient conformance lookup or special owner rule exists. The composed
call plan must supply each demand exactly once. The authoritative layout owns
the physical offset; the materialization row names only the validated slot.
The rows carry no source-visible address and do not describe whether the
foreign side copies or retains argument storage—that remains the ordinary
parameter lifetime/custody disposition.

Both `NativePlace::Parameter` and `NativePlace::Field.parameter` index one
nominal native-parameter space. Existing field roots originate in semantic
formals; direct callback entries originate in exact binder/requirement pairs.
Authored telescope order separately fixes ABI position. Exact replay uses a
boundary-plan application fingerprint covering that ordered identity-to-
placement mapping in addition to the reusable physical plan, so equally shaped
parameter reorder cannot hide behind an unchanged register sequence. The
ordinal-to-nominal identity migration is a versioned reissue of affected
artifacts, not a reinterpretation.

A boundary requirement names its convention through the ordinary generic policy
relationship `Calling<C>`, where `C` satisfies `CallingPolicy`. The policy's
compile-time `plan` machine receives the normalized boundary signature and
returns either `Accepted(BoundaryEntryPlan)` or a structured rejection. The
compiler validates and canonicalizes accepted results. Only the canonical
evaluated plan enters contract identity: neither the policy type's name nor the
source body does. A source refactor that computes the same canonical plan is
therefore ABI-invisible; a changed observable placement or machine-state promise
is an ABI change.

Policy evaluation is also the signature-admission point. A flexible convention
such as a platform C ABI commonly computes placements for many legal signatures;
a hardware-dictated convention may instead reject most signatures. An interrupt
policy, for example, rejects an incompatible frame parameter, ordinary return
value, or return-control form directly at the `Calling<C>` relationship. It does
not manufacture an invalid plan and wait for a later lowering diagnostic.

This relationship belongs to the requirement, not to a `Binding`. A syscall,
DLL import, vtable slot, or provider realization must refine the convention the
requirement already pinned; its mechanism does not silently select an ABI. A
semantic boundary trait that is reusable across conventions can expose the
policy as an ordinary type parameter, for example
`boundary trait Console<C>: Calling<C> where C satisfies CallingPolicy`. Concrete
instantiations remain distinct boundary contracts, and one instantiation cannot
mix conventions entry by entry.

The requirement's canonical `CallPlan + StatePlan` is the published promise.
Register allocation, emitted clobbers, and the final machine-state footprint are
realization evidence checked against that promise; changing legal evidence
revalidates the provider artifact without changing caller identity. A calling
plan is auditable policy data, never an unchecked ABI string.

Policy classification follows one boundary-shape rule. When a type's public
normalized structure determines all ABI facts, the selected policy may
structurally classify or reject it. Fixed arrays and fixed records fall in this
category; their element/member structure, not byte size alone, drives aggregate
classification. Omega does not reproduce C source-level array decay.

When ABI facts remain choices, the native leaf must declare them. Safe slices,
text views, vectors, and bounded text carriers do not choose a foreign length
type, nullability, retention contract, terminator, or descriptor-versus-
separate-parameters shape, so default native policies reject them directly. A
checked adapter lowers the safe value to the foreign API's actual declared
pointer and length parameters, null-terminated pointer, or real descriptor
record. The compiler's private slice carrier is not public ABI. Retaining
foreign calls need an explicit pinned loan, ownership transfer, or registration
protocol rather than the synchronous borrowed-out contract.

The closed plan vocabulary and validator remain compiler-owned. Target/platform
packages author deterministic compile-time policy machines over that vocabulary;
the compiler evaluates, canonicalizes, fingerprints, and emits from the accepted
plan. Current built-in Rust evaluators are migration bootstraps, not the
steady-state authorship boundary.

The firewall is observational: a counterparty must agree on register/stack
placement and preserved machine state, so those normalized promises are contract
identity. The particular allocation, stub shape, and footprint certificate used
to prove one provider meets them are implementation evidence and remain outside
caller identity.

The plan must cover argument placement, return placement, clobbers, stack
alignment, and failure behavior — validated before any deriver trusts it.

## Placed And Externally Mutable Memory

Ordinary values remain ordinary. A local `Point`, an owned array, and a normal
`&mut T` use direct field projection and lvalues; they do not acquire placement
plans or accessors. The machinery in this section applies when code imposes a
typed interpretation on backing whose authority or observation behavior must
be checked: MMIO, concurrently shared pages, DMA-visible storage, restricted
RAM views, and similar placed storage.

An ordinary `&mut [u8]` is already a broad grant. It licenses ordinary byte
loads and stores and lets the optimizer reorder, combine, or remove accesses.
Describing those bytes afterward cannot retract that authority. Consequently,
MMIO or concurrently mutable shared storage is never exposed first as ordinary
mutable bytes and then repaired with a cast.

`&write T` is the corresponding ordinary-place attenuation when a checked
callee may overwrite an existing valid `T` but must not observe its prior
contents. It keeps the exclusive source loan and ordinary reference ABI while
removing loads, readable reborrows, read-modify-write, and every other
content-dependent operation. It is not a placement accessor, does not govern
device observation, and never denotes vacant or uninitialized storage.

The current bounded whole-root primitive-store carrier intentionally ends
before this ABI realization. Abstract and optimization forms retain the exact
write-only parameter and typed incoming value, but target lowering reports a
dedicated unsupported-store fence until address, width, store operation, and
provider non-observation authority can be derived independently.

### Geometry, demand, and supply

Placed storage keeps three questions independent:

- `LayoutPlan` describes where the fields and bits are.
- `AccessPlan` describes which primitive operations a consumer requests for
  each field.
- `ResourceProfile` is admitted provider evidence describing what the backing
  can actually support over each subrange.

The plans meet in `Placed<P, T>`. `T` is the semantic schema and `P` is a
nominal placement policy that selects layout, access, and required boundary
reach:

```omega
trait Placement {
    machine plan(schema: Schema) -> plan: PlacementPlan;
}

data PlacementPlan {
    layout: LayoutPlan;
    access: AccessPlan;
    reach: BoundaryReach;
}

data UartMmio;

machine UartMmio::plan(schema: Schema) -> plan: PlacementPlan
    satisfies Placement::plan
{
    let layout = UartLayout::plan(schema);
    let access = UartAccess::plan(schema, layout);
    PlacementPlan {
        layout: layout,
        access: access,
        reach: DeviceIo::reach()
    }
}

data UartRegisters {
    status: u32;
    transmit: u32;
}
```

`Placement::plan` is build-time admissible. The compiler reflects `T` into a
`Schema`, evaluates the selected policy, validates and normalizes both plans,
and includes their normalized identity in compatibility artifacts.
`Placed<UartMmio, UartRegisters>` is the resulting view type. Two
nominal policies remain distinct even when their normalized plans happen to
match: the policy name owns the binding and its binding-private projection
surface.

`Placed<P, T>` is a compiler-derived placed view, not ordinary record storage.
It retains either a source borrow or an owned split extent. Projection is pure
and yields an accessor rather than an lvalue:

```omega
let status = uart.status.read();
uart.transmit.write(byte);
```

The projection performs no access. `read` or `write` performs the authorized
event. The accessor carries the exact field identity, address, width,
observation model, plan identity, source borrow, and reach needed by lowering;
it cannot outlive or exceed the placed view.

### Access plans

An access policy mirrors a layout policy:

```omega
trait Access {
    machine plan(
        schema: Schema,
        layout: LayoutPlan
    ) -> plan: AccessPlan;
}
```

`Schema` contains compiler-issued field keys. They are opaque identities, not
integer indexes and not runtime authority. A policy starts with
`AccessPlan::inaccessible(schema)` and replaces decisions by key. Reordering a
source declaration therefore cannot silently move a permission to a different
field, and omission denies access. The effective transfer container and width
come from the validated layout; the access author chooses operations, not a
second copy of the geometry.

Each field has exactly one access case:

```omega
data Exposure {
    case Exported;
    case BindingPrivate;
}

data ExternalRead {
    case None;
    case Read;
    case Take;
}

data FieldAccess {
    case Inaccessible;
    case Stable(
        read: bool,
        take: bool,
        write: bool,
        swap: bool,
        exposure: Exposure
    );
    case External(
        read: ExternalRead,
        write: bool,
        exposure: Exposure
    );
    case Atomic(
        operations: AtomicOperations,
        exposure: Exposure
    );
}
```

`Stable` permits ordinary observation of storage that does not change behind
the active loan. With the corresponding plan permission and both an exclusive
current borrow and an exclusive source borrow, it may derive destructive take,
write, swap, ordinary mutation, and compound updates. Stability describes
observation behavior; it does not itself prove Omega-side exclusivity.

`External` means each primitive transfer occurs exactly once at an admitted
width and is neither elided nor combined with another transfer. A readable
logical field must be covered by one admitted transfer; the compiler never
assembles one field from several device reads. A narrow non-consuming read may
read its whole transfer container once and then project bits from the owned
snapshot. A write must cover a complete admitted container; the compiler never
synthesizes a read-modify-write for external storage. External write permission
comes from the access plan and provider rights, not from pretending that the
device is excluded by an Omega `&mut` borrow.

`Atomic` exposes only the individually admitted atomic operations and the
ordinary atomic ordering vocabulary. Load, store, swap, compare-exchange, and
the fetch operation families remain distinct permissions. Stable access does
not imply atomic access. An active atomic loan pins one transfer granularity for
each overlapping location; simultaneously live atomic views of the same bytes
cannot select different widths.

Every generated operation has both a logical extent and an effect footprint.
The logical extent names the field value; the effect footprint names every bit
or transfer container the operation observes or changes. Borrow conflicts use
the effect footprint:

| Operation | Conflict rule |
|---|---|
| non-consuming read | shared over its transfer footprint |
| destructive read | exclusive over its complete effect footprint |
| stable read-modify-write | exclusive over its complete transfer container |
| atomic operation | the exact admitted atomic operation and width |

Therefore two disjoint bitfields in one word are not independently mutable by
read-modify-write. A destructive container likewise exposes one `take` of the
whole snapshot, followed by pure projection from the returned value; it never
derives separately callable destructive accessors for fields consumed by the
same transfer. Generated accessors are limited to effects confined to their
declared footprint. Broader device side effects belong behind an authored
package machine.

The compiler derives small operation requirements from accepted fields:
`Readable<T>`, `DestructiveRead<T>`, `Writable<T>`, `Swappable<T>`, and the
atomic operation families. Helpers may accept one such accessor instead of the
whole view:

```omega
machine send_byte<T, Write: T satisfies Writable<u8>>(
    transmit: T,
    byte: u8
)
{
    Write::write(transmit, byte);
}
```

`Writable<T>` may take an exclusive borrow of the short-lived accessor value
without implying an exclusive borrow of the underlying External range. Stable
write derivation additionally requires the retained exclusive source borrow;
External write derivation instead requires one permitted complete transfer.

Atomic access uses one sealed `omega::core` requirement per primitive
operation: `AtomicLoad<T>`, `AtomicStore<T>`, `AtomicSwap<T>`,
`AtomicCompareExchange<T>`, `AtomicCompareExchangeOnce<T>`,
`AtomicTryExchange<T, Key>`, `AtomicTryExchangeOnce<T, Key>`, and each
`AtomicFetch*<T>` remain distinct. The first compare-exchange axis is decisive
versus single-attempt execution; the second is whether failure exposes the
resident. Ordinary core atomics and placed accessors conform to the same
requirements. A normalized placement derives only its admitted subset; missing
conformance makes the operation unavailable, and an arithmetic carrier bound
cannot manufacture it. All receivers are shared and ordering is explicit
proof-static operation data.

Every operation requires a fixed representation fitting one admitted atomic
width and alignment. Load requires a duplicable resident; store requires the
displaced resident to be discardable; swap conserves ownership and may move an
affine or linear resident only when Stable initialization or an exact
resident-content transfer established that the placement owns it. A
provider-opened External view never
owns device contents and cannot derive that operation. Both observing
compare-exchange forms return the resident observation on failure and therefore
require a copyable resident. The non-observing forms return the proposed value
on mismatch and, for the single-attempt form, on uncommitted failure; success
always returns the displaced resident, and ordinary multiplicity decides
whether the caller may discard it. Their copyable `Key` and selected atomic
rule prove the exact comparison encoding without constructing a second owned
`T`; neither is a runtime parameter of `AtomicTryExchangeOutcome<T>` or
`AtomicTryExchangeOnceOutcome<T>`. The observing forms analogously return
`AtomicCompareExchangeOutcome<T>` or
`AtomicCompareExchangeOnceOutcome<T>`. These fixed nominal sums and their
canonical case order are part of the operation ABI. Fetch conformance
additionally proves the exact provider raw transition over every read-reachable
representation; no External read/write pair synthesizes it.

The compiler internally retains the observing single-attempt result as a
distinct three-arm custody identity through checked validation. That carrier is
not source admission or execution authority: the public call remains fenced
until Terminal, provider, interpreter, and target replay preserve the same
operation and result axes end to end.

A destructive read derives `DestructiveRead<T>::take(&mut self)`, never
`Readable<T>`. Stable take moves the exact resident field occurrence out and
leaves that field semantically vacant in a partial placed value; it does not
claim that the old bits were cleared. External take performs one admitted
destructive transfer, advances the external content version, and returns one
owned whole-container snapshot. Whether either operation is exported or
binding-private remains a separate policy choice: a FIFO pop may be public,
while a read-to-clear status container may be wrapped by one package machine
that reads once and returns an owned snapshot. Only the nominal placement
package may directly name or issue a binding-private accessor. Possession is
deliberate delegation: externally authored generic code may invoke the
accessor's public operation requirements without naming its opaque type.
Copyability controls durable duplication, cross-activation shareability
controls concurrent delegation, and a counted permit is required when the
number of delegated uses must be bounded.

Device protocol meaning does not become an access-plan case. W1C,
read-back-to-flush, FIFO, doorbell, lock, and coherent-snapshot behavior belong
to authored package machines over the permitted primitives.

DMA publication, device acquisition, cache maintenance, MMIO notification,
and posted-write completion are device-protocol roles rather than fields added
to every boundary signature or one universal fence. A hosted, firmware, or
native provider may initially satisfy one complete DMA service boundary while
keeping those roles private. Checked source cannot compose their intermediate
proofs until a concrete driver fixes role-specific typed operations. Each
eventual operation binds its exact range, mapping, device instance, and sealed
runtime queue/session scope. Build selection admits the provider and capability
schema; the installed provider issues each scope occurrence.

Publication evidence is bound to the published place and invalidated by any
intersecting write frame. Its erased value proves source composition but cannot
order emitted code; the publication operation itself contributes the scoped IR
ordering event. Device acquisition consumes completion evidence tied to the
same request, external-loan occurrence, instance, and runtime scope. Device
status and custody release remain separate. It restores Stable CPU observation
only when exact completion evidence proves release; otherwise it returns the
pending loan and completion candidate and keeps the placement External.

### Admission and placement

An `Extent` is transparent geometry:

```omega
pub data Extent [linear] {
    base: addr;
    length: u64;
}
```

Anyone can spell the fields. That does not establish the provider-originated
facts that make the range usable. Reconstructing the same numbers similarly
does not copy authority. Useful operations require a borrow of an
`Extent in Granted`. The qualification supplies the provider-originated
authority; weakening it to `&Extent` leaves only geometry and cannot authorize
placement. Placement operations borrow the exact source place directly.

There is no source-visible `ExtentLoan`. The borrow and projected place already
carry the range, lifetime, and shared or exclusive polarity. Compiler and
foundation implementations may retain an internal loan record, but it is
borrow-checker bookkeeping rather than a nominal value or authority root.
Static subranges use place projection and its ordinary disjointness proof;
runtime-owned subranges use conserved `Extent` split and merge.

`ResourceProfile` is ordinary provider-authored data, not a capability. The
same selected-provider grant that establishes `Granted` binds one normalized
profile and sealed receipt to the exact range. The placement checker finds that
receipt through the qualification; callers never pass a profile,
receipt, or admission value that a record literal could forge. Compatibility is
a compiler and installation judgment, not a source-visible admitted value.

`Placed<P, T>` is an opaque `omega::core` view. It does not duplicate the
borrow checker's range, ownership, lease, or revision ledger. Its compiler-known
jobs are to derive each field's geometry from normalized `P`, produce an
accessor tied to that exact source place, and lower each accessor operation
through `P` rather than as an ordinary RAM lvalue. Its multiplicity follows the
source and semantic inputs: a shared source can produce a shared read-only view,
while an embedded linear Type value makes the view linear.

Anyone may request an explicit interpretation. Success depends on the facts and
custody held, not on the caller's identity. Existing domains establish external
qualifications such as `Extent in UartRegisters`; only their sealed routes may
originate those facts, but any holder may use them as placement inputs. There is
no `Contains<P, T>` domain and no separate registry of code allowed to cast.

Stable dormant content uses a different kind of qualification:

```omega
Extent in Granted & Vacant
Extent in Granted & Resident<P, T>
```

`Resident<P, T>` is an erased, owned, type-indexed core domain. It says that
this exact placement range, including required padding and transfer footprint,
owns one complete live `T` represented through `P`. Its normalized `P` and `T`
arguments are invariant semantic identity; the exact content occurrence,
mapping, and revision remain occurrence provenance rather than type arguments.
The qualification cannot weaken or cast away and is mutually exclusive with
`Vacant` over the same range. It retains represented custody and every
zero-layout or explicitly `[erased]` Type field of the semantic value without
turning those fields into hidden runtime bytes.

Ordinary Extent split or merge rejects while `Resident<P, T>` is present. The
qualification covers the whole exact range, so a split cuts the object and a
merge would describe several objects as one. Structural extraction instead
uses a placed view and its partial-move rules. A future extent containing
several independently resident objects requires an explicit closed resident-map
algebra; it is not inferred from adjacency or field geometry.

Three core operations share the placement checker but remain distinct because
they do different things to content:

| Operation | Meaning |
|---|---|
| `Placement::view_borrowed` / `view_owned` | interpret existing content without running a content validator |
| `Placement::initialize_borrowed` / `initialize_owned` | encode a newly constructed `T` into exclusive `Vacant` Stable storage |
| `Placement::validate_borrowed` / `validate_owned` | inspect Stable existing content with one checked static validator and establish its guarded facts |

There is no generic `adopt` operation. A provider-specific open/adopt machine
establishes its external domain and custody, then calls the appropriate view operation. Generic
initialization is never synthesized for `External`: programming a device is an
authored protocol whose ordering and side effects belong in its machine
contract.

`view` and `validate` never create represented non-copy custody. Over a range
without `Resident<P, T>`, they reject when `T` contains a represented non-copy
field. Such content requires `initialize`, an existing exact resident claim, or
an admitted resident-content transfer from the provider or prior owner.
Zero-layout and explicitly `[erased]` non-copy Type fields remain ordinary
custody inputs because there are no resident bytes whose identity could disagree
with them. Validation may establish representation and predicates; it cannot
establish ownership or uniqueness.

The generic `Resident<P, T>` declaration authorizes initialization from
`Vacant + T` and one core `ResidentContentTransfer<P, T>` provider requirement.
Initialization is a derived establishment over accounted inputs. A selected
provider call is an introduction only at an exact installed/provider-issuance
occurrence with no parent resident lineage and the matching receipt. `view`,
loan ending, and resident-preserving retirement are not establishment routes.

The resident-content claim and an active view have separate identities. A
borrowed view creates an ordinary loan naming the exact parent claim, range,
polarity, and lifetime; the lender retains ownership but is inaccessible as the
loan requires, and several shared loans may coexist. Moving an owned resident
extent transfers the resident claim into the view. Resident-preserving
retirement returns that exact same claim identity. Repeated retirement and
re-viewing create new view occurrences but never new resident custody. Ordinary
in-artifact calls substitute those occurrences into callee parameters rather
than creating roots.

The current concrete foundation carrier implements the provider-issued owned
cycle directly: provider transfer seals one nonzero resident claim into dormant
Stable content, each explicit owned view supplies a fresh nonzero placed-
occurrence identity, primitive access retains both, and resident-preserving
retirement returns the unchanged claim and provider receipts. Borrowed resident
views retain that same lender-owned claim and receipts, a fresh occurrence, and
one exact whole-range shared or exclusive loan; ending them releases only the
loan. The source-visible `Extent::Vacant` and invariant indexed
`Extent::Resident<P, T>` identities now live in core together with opaque
`Placed<P, T>` and the ordinary outcome, return, and custody-trait vocabulary.
This does not yet implement placement operations, custody agreement checking,
`Vacant` transitions, partial moves, or Terminal and installation propagation.

The instantiated operation derives its requirements from `P`, `T`, and the
exact source. Geometry and access demand come from normalized `P`; total decode,
encodability, and default-domain predicates come from `T`; facts about external
reality require existing admitted provider qualifications. Proof may establish
a proposition but may not manufacture a Type value. Every unconditional
non-runtime Type field therefore appears in one ordinary by-value custody
record, recursively keyed by canonical declaration path, regardless of whether
it is structurally zero-layout or explicitly `[erased]`. Proposition witnesses
use the proof lane after `;`. Case-dependent Type custody is not flattened into
that record: an authored establishment machine first classifies the content and
then transfers the authority for the selected case.

Validation selects an ordinary static machine parameter with a complete
structural `where machine` contract. That contract includes its input/result,
guarded `ensures`, effects, crashes, and applicable termination guarantee. The
selected validator is directly specialized and invoked; it is not a retained
foreign callback and needs no registered-callback requirement. Its exact
identity and derivation still enter occurrence provenance.

Static plan, provider-profile, rights, transfer-width, or known-geometry
incompatibility rejects compilation or installation. A runtime result exists
only for genuinely dynamic checks such as range geometry, content validation,
or an establishment-time revision comparison. Runtime rejection is ordinary
cased data, never an exception or hidden trap. Validator-specific errors retain
their own declared sum rather than collapsing into an opaque error code.

Placement outcome and recovery identities are ordinary authored core data:

```omega
data PlacementOutcome<View, Returned, Reason> {
    case Ready(view: View);
    case Rejected(returned: Returned, reason: Reason);
}

data PlacementReturn<Source, Custody> {
    source: Source;
    custody: Custody;
}
```

Each operation uses an authored operation-specific reason sum over only the
dynamic failures it can observe, such as range or alignment mismatch and an
establishment-time stale revision. Validate additionally carries its selected
validator's declared content-error sum as an ordinary type parameter; it never
erases that information into a number, string, or undifferentiated
`InvalidContent` code.

The custody payload is authored rather than synthesized. For a semantic type
with one non-runtime authority field:

```omega
pub trait PlacementCustody<P, T> { }

data PacketCustody {
    authority: DeviceAuthority;
}

pub PacketNativeCustody:
    PacketCustody satisfies PlacementCustody<Native, Packet>;
```

`PlacementCustody<P, T>` is a compiler-checked ordinary trait relationship.
The named conformance proves that the custody record agrees exactly with the
non-runtime Type projection chosen by the evaluated `Placement::plan`: every
required canonical field path appears once with the exact type and
multiplicity, and no represented field appears. The compiler retains this as
ordinary conformance evidence through closure, package review, canonical
encoding, and replay. It does not manufacture a source-visible returned-row
type or a placement-specific evidence category.

The plan is part of the agreement subject. If a policy revision moves a field
between representation and custody, the conformance rejects. Its diagnostic
names the exact plan machine and normalized field decision—for example that
`Packet.bits` is represented at offset 0 with width 4—rather than reporting an
unexplained field-set difference.

The current first compiler rung checks this agreement for one concrete named
conformance after the exact concrete `Placed<P, T>` policy/schema plan has been
retained. For direct erased record fields, canonical path, normalized type, and
multiplicity must match exactly, and any field with a physical plan entry is
rejected from custody with the retained offset/width decision. One
represented acyclic, non-generic, case-free checked-record field may also hold
direct erased leaves. Its custody field is an authored projection record whose
leaves match the complete root-to-leaf canonical paths, exact types, and
multiplicities; represented siblings are rejected using the enclosing field's
plan decision. The checker recognizes the toolchain core trait rather than a
same-spelled user trait. One further represented acyclic, non-generic,
case-free record with a nonzero canonical fixed representation may occur on
that spine; the authored projection retains both enclosing field identities
before each direct erased leaf, and represented siblings still cite the root
plan entry. Third through thirteenth represented record levels are now available
under the same restrictions. All thirteen enclosing identities remain in each
erased-leaf path, and one bounded recursive traversal rejects hidden unsupported
custody without a depth-specific implementation. A fourteenth represented record
level, structurally zero-layout wrappers, arrays, generic and case-dependent
fields, planless agreements, and the generic establishment calls below remain
unavailable.

A generic placement operation carries both the custody type and the exact
selected conformance:

```omega
machine inspect<P, T, C, Evidence: C satisfies PlacementCustody<P, T>>(
    extent: Extent in Granted,
    custody: C
);
```

Concrete calls explicitly name that evidence, including its owned arguments;
the custody type alone never triggers ambient conformance search. The complete
owned call has this result shape:

```omega
Placement::view_owned<
    Native,
    Packet,
    PacketCustody,
    PacketNativeCustody
>(move extent, move custody)
    -> PlacementOutcome<
        Placed<Native, Packet>,
        PlacementReturn<Extent in Granted, PacketCustody>,
        ViewRejection
    >;
```

The six source operations are named separately because Omega does not infer
ownership polymorphism. Borrowed rejection ends the source loan and returns
the moved custody value `C`. Owned rejection returns
`PlacementReturn<Extent in Granted, C>`, so neither the extent nor custody can
disappear. Type and Prop are not packaged together: ordinary outcome payloads
stay before `;`, and selected evidence outputs use the separate proof-output
lane from chapter 10.

Every formal input has an explicit disposition on every outcome:

| Input | Allowed outcome disposition |
|---|---|
| moved Type value | embedded in the view, returned now, or consumed by one named authorized operation |
| borrowed Type value | retained by the view or released |
| proposition term | cited or copied; no custody disposition |
| static validator | selected and provenance-recorded; no runtime value |

Absence from an outcome does not prove consumption. A consumed disposition
names the exact authorized consumer or cleanup operation. An embedded input
becomes retirement debt; a returned input appears exactly in the authored
rejection carrier. At a placement call site, `move` marks moved arguments; it
never decorates the corresponding formal parameter declaration:

```omega
transition Placement::validate_owned<
    P,
    T,
    PacketCustody,
    PacketNativeCustody,
    Validator
>(
    move source,
    move custody,
    move revision_ticket
) {
    PlacementOutcome::Ready { view } ->
        use(view)

    PlacementOutcome::Rejected {
        returned: { source, custody },
        reason
    } ->
        recover(move source, move custody, reason)
}
```

Here the owned `source` and `custody` are embedded on `Ready` and returned
unchanged on `Rejected`. If revision checking consumes `revision_ticket` on
both paths, both disposition rows name that checking operation. The borrowed
counterpart instead retains the source loan in the ready view, releases it on
rejection, and returns only `custody`.

A statically discharged dynamic check narrows the result to `Ready`; it does not
coerce the sum to its payload. General irrefutable case destructuring extracts
the view while keeping the proof obligation visible:

```omega
let PlacementOutcome::Ready { view: pair } =
    Placement::view_borrowed<P, T, C, Evidence>(&mut source, move custody);
```

The pattern is accepted only when the fact catalog proves that `Rejected` is
impossible. Otherwise the caller handles every live case at a transition.

Asynchronously revocable mappings use a distinct fallible provider protocol,
not a `Placed` policy whose every field access may suddenly fail. An ordinary
live `Placed` mapping retains a lease or claim guaranteeing that revocation
cannot occur until the view retires.

Borrowed initialization is a scoped establishment. The lender is vacant before
the borrow and vacant again after it; while the `Placed` value exists the lender
is inaccessible, and mandatory edge cleanup destroys `T` before the borrow can
end. This depends on Omega having no `forget`-style escape from conservation.
An owned initialization consumes a split vacant `Extent`. A complete owned
Stable view may later destroy or move out `T` and return
`Extent in Granted & Vacant`, or leave the value intact and return
`Extent in Granted & Resident<P, T>`. The latter is identity forwarding of the
resident claim held by the view, not another establishment route. Ending a
borrowed resident view merely ends its loan and makes the lender's same claim
usable again. Ordinary drop does not invent an allocator, release route, or
resident claim. An owned provider-backed view returns provider custody; owning
its storage alone never establishes ownership of the existing semantic content.

Retirement is generated from the establishment disposition table. Every input
embedded on the successful outcome must be returned, forwarded, or consumed by
an exact authorized operation; inputs already consumed during establishment do
not reappear. The original source capability returns with only those
qualification changes proved by explicit initialization, mutation,
destruction, move-out, or provider transitions. Linear non-runtime Type fields
therefore remain ordinary conservation obligations even when they occupy no
bytes.

A partially moved placed value has no retirement outcome. The missing paths
must be restored, or every remaining field must be moved out or destroyed so
the range becomes `Vacant`. There is no implicit partial-resident
qualification. In-place migration consequently has one general shape: take the
whole old value, leaving the range vacant, then initialize the new value. The
new placement footprint must fit that exact range; otherwise migration uses a
second range and retires the old one to `Vacant`. Component replacement may
retain resident content only when normalized `P` and `T` identities agree, or
through an explicit migration consuming the old resident claim.

A crash while a partial view is live records the exact range, resident lineage,
live/moved/vacant field paths, non-runtime custody, operation in progress, and
provider dependencies in the abandonment frontier. That record establishes
neither `Vacant` nor `Resident<P, T>`. Continued execution can reclaim the range
only through structural isolation plus an admitted resource-specific reset,
recovery, quarantine, or custody-exit route; otherwise it remains abandoned.

Projection preserves the semantic kind of each field. Represented fields yield
plan-derived accessors. Proposition fields yield copyable proof terms.
Structurally zero-layout and explicitly `[erased]` Type fields use ordinary
field borrowing and movement with their declared multiplicity. Stable take of a
represented non-copy field and movement of a non-runtime Type field may leave a
structurally tracked partial placed value: whole-`T`
operations become unavailable, unaffected sibling paths remain usable where the
ordinary partial-move rules permit, and restoration or retirement must account
for every residual field. Nominal whole-value cleanup or a plan that cannot
expose one independent path forbids the move.

Every retained fact and non-runtime binding has a statically closed dependency
shape whose occurrence-specific members name the exact mapping, lease, revision,
and semantic places established for this view. An intersecting write is derived
only when those dependencies are preserved or replacement inputs are supplied.
Otherwise no accessor is generated. The compiler never guesses a weaker
semantic type or typestate transition; an author writes an explicit operation
whose contract returns the intended differently qualified view. Default-domain
invariants reuse the ordinary invariant window and must be restored before the
next consumption boundary. Owned or routed custody may be moved out, returned,
or swapped, but never silently forgotten or absorbed by a generic write.

A relationship between several placed occurrences is an ordinary Type carrier
borrowing all of them, not a global invalidation graph. For External storage it
also retains the provider lease, revision, or quiescence custody that prevents
outside mutation for the relationship's lifetime.

`Vacant` is erased place state meaning that the exact range contains no live
established value. It does not mean zeroed, readable, never used, or newly
allocated. Allocation/raw storage and authorized destruction or move-out may
establish it. Merely dropping a borrowed view does not. The name here denotes
the compiler's established-place judgment; ordinary callers normally inherit
it from the storage producer or retirement route rather than spelling or
minting it themselves.

The compatibility check requires every requested field interval to fit an
admitted region; every requested operation, transfer width, alignment, and
reach to be supplied; and the requested observation to be a safe use of that
backing. Stable backing may be observed conservatively through `External`
operations. External backing cannot be treated as stable. Atomic access is
available only when explicitly supplied.

Representation and transfer legality are checked independently, per field and
per operation. On the read side, the compiler establishes whether the selected
encoding maps every possible stored pattern to `T`; a one-time validator is
permitted only for `Stable` storage. On the write side, it establishes whether
every value admitted by the field type has an encoding, or requires proof that
the concrete value fits. The compiler checks such a proof but never invents a
qualification such as `Fits12`, silently truncates, or chooses an encoder.

A foreign integer whose stored width differs across targets uses the closed
integer-placement form. The placement names its byte offset, stored width, and
signed or unsigned interpretation; field projection performs the corresponding
sign or zero extension into the semantic carrier. This keeps one portable
record definition across ABIs such as `struct stat`. Writing through such a
view requires total encodability or a concrete fit proof in addition to the
ordinary legal-transfer and observation checks. An adapter machine is optional
policy for more involved decoding, not the canonical answer to integer-width
variation alone. The stored width is measured in bits while the offset is
measured in bytes. The normalized vocabulary currently accepts whole-byte
widths through 64 bits and rejects any encoding whose complete stored range
does not fit the declared semantic carrier. The validated offset, stored width,
and interpretation survive through typed plan-laid and concrete backend layout
records. Direct owned and reference-backed projection loads the physical width
and sign- or zero-extends into the semantic carrier. Runtime-indexed projection
uses the same decode after ordinary descriptor, inline-frame, machine-owned, or
reference-backed address calculation. The compiler also retains whether the
field's admitted semantic range is wholly encodable at the stored width. A
direct write into stable owned storage may narrow on that total-write evidence,
and direct guards compare the widened semantic projection rather than the raw
neighboring carrier bytes. Read-only interpreter record views apply the same
decode. The standard filesystem's portable stat carrier uses this mechanism so
Darwin, Linux x86-64, Linux AArch64, and Windows policies can retain their real
integer widths without changing semantic field types. Concrete proved-fit
writes from exact compile-time integers or runtime values with a Psi-checked
inclusive range now use the same exact-width store. Every assignment whose
target type resolves participates in range analysis without becoming a new
language proof obligation. The checked value retains its use-site type reference
and BigInt discharge interval, including stable incoming guards and boundary
witnesses, so lowering consumes a Psi fact rather than inferring proof from
storage shape. Unproved values remain fail closed. Ordinary scalar consumers do
not make `IntegerAt` interchangeable with ordinary `At`.

A mutable raw-byte record recast preserves each field's `IntegerAt` encoding.
Reads decode the exact physical bytes, and every write must either be total for
that encoding or carry a Psi proof that its value fits; the write then touches
only those physical bytes. This does not make differently represented typed
records aliases of one another: mutable typed aggregate recasts still require
identical representations.

When such a plan-laid record crosses a boundary by value, ABI classification
uses the stored integer's physical width and alignment rather than its wider
semantic carrier. The field remains a physical aggregate leaf during transfer;
ordinary field projection performs the retained sign or zero extension after
the value lands.

The target-neutral scalar materializer applies the same rule to concrete named
values. It checks fit before changing the destination, emits exactly the stored
bytes in the selected byte order, and its inverse decoder extends back into the
declared semantic width. A compiler/provider-resolved symbolic value passes the
same fit check. A loader cannot defer an unresolved narrowed value and therefore
rejects it. A post-handoff generated writer may resolve the sealed identity
privately, but it retains the exact signed/unsigned fit constraint in its
invocation evidence and checks the resolved value before any write or context
publication.

The same target-neutral foundation accepts compiler-sized owned aggregate
fields through one whole-field `At` placement. An outer fixed array may instead
use exactly one `At` per compiler-sized element at one nonoverlapping constant
destination stride; sorted destination offsets determine semantic element
order independently of authored entry order. It stages zeroed output, checks
the exact supplied extent and every destination interval, and rejects missing,
extra, incomplete, wrong-count, irregular-stride, overlapping, scalar-fragment,
stored-integer, or out-of-bounds fields before changing the destination. This is
the normalized writer primitive. The current typed source-owned bridge derives
complete bytes for recursively fixed records and arrays in the supported
checked-shape subset, including fully specialized generic-record instances and
erased fields that remain semantically required but contribute no storage. A
specialized record is selected by its synthesized concrete symbol and
substituted member types, not by the spelling of its name; unresolved generic
shapes still reject. This omission is recursive: a relevant nested record with only erased
runtime content receives no physical field entry, but its exact semantic value
is still required. It rejects malformed nested values atomically. Runtime establishment
beyond that fixed subset remains source-materialization work; sum placement
waits for its settled vocabulary.

Encodability alone does not authorize a transfer: sub-unit mutation must also
have a legal implementation. Stable exclusive storage may use one bounded
read-patch-write sequence. External storage requires a whole-container or
provider-supplied masked write. Shared atomic set/clear may use one admitted
fetch operation; arbitrary sub-unit assignment cannot hide a compare-exchange
retry loop and is unavailable without one bounded masked-replace operation.
Diagnostics distinguish missing exclusivity, unavailable masked transfer, and
unprovable value fit.

Alignment has a build-time and an admission-time part. From every field offset
and transfer alignment, plan validation derives a constraint on the base:

```text
(base + field_offset) mod alignment = 0
```

Power-of-two constraints combine into one normalized base congruence.
Mutually inconsistent field requirements reject while evaluating the plan.
Admission then checks the actual base, or consumes a provider guarantee strong
enough to discharge the constraint. Diagnostics name the conflicting fields,
offsets, and transfer requirements before showing the congruence detail.

After admission there is no per-access profile lookup. Lowering consumes the
sealed accessor authorization. It may strengthen an atomic ordering for a
target but may not weaken it or recover missing authority from a numeric
address. A single compare-exchange attempt is a bounded primitive; a machine
that retries until success carries its ordinary unknown or no-finite-guarantee
work attribution. Generated `.write` accessors never conceal such a loop.

Admission proves that `P` is supportable by the supplied range. It cannot prove
that `P` assigns the correct meaning to the physical device. The binding
therefore publishes a separate admitted schema-correspondence fact tied to the
provider's device identity, with its datasheet or platform source retained in
artifact provenance. Revision-sensitive bindings may condition that fact on a
runtime identification read: the bootstrap ID placement is admitted, the
observed value derives a revision predicate, and that predicate selects the
full placement. The observation and full placement must remain bound to the
same stable device instance and grant. This improves provenance without
changing the admitted trust floor.

### Loans, aliases, and phases

Stable ordinary mutation is the conjunction of plan permission, an exclusive
current borrow, and an exclusive source borrow. Taking `&mut` to a view placed
from a shared source does not upgrade it. External and atomic operations follow
their declared operation permissions instead: an Omega borrow cannot exclude a
device, and a shared external view may issue a permitted whole-container write.
Borrow polarity governs aliasing among Omega views; the access plan and provider
rights govern which transfers exist.

Multiple non-consuming reads may coexist. Destructive reads and stable
read-modify-write operations reserve their entire effect footprint. Therefore
logical bitfield disjointness is insufficient when two fields share one
transfer container. Disjoint exclusive subrange views coexist only when both
their place extents and operation footprints are nonconflicting. A child borrow
receives only the parent's profile restricted to the child interval and
attenuated rights, reach, and operations.

Observation can change with an ownership phase. While a device owns a DMA
buffer, CPU access may be absent or explicitly external. Consuming the
completion token can restore a stable CPU loan, after which ordinary
optimizable reads are legal again. The profile is not mutated; different
phase transitions establish different restricted loans.

Shared-memory IPC uses the same discipline. A page transferred into exclusive
CPU ownership may become ordinary stable typed storage. A concurrently shared
page remains placed and exposes only the atomic, external, and protocol
operations admitted for the current loan. Agreement on a layout does not by
itself establish a lock protocol or make a hostile peer cooperative.

### Applicability

`Placed<P, T>` applies when the current execution domain can name a stable
place whose lifetime and aliasing the loan can govern, and each primitive
access is a finite compiler-owned, non-suspending operation with no recoverable
failure under the admitted provider contract. This is a bound on program work,
not on cache or fabric latency.

MMIO, resident shared pages, persistent memory while mapped, and CPU-mapped GPU
storage can satisfy that contract. Durability, device completion, and GPU
execution remain explicit protocols. Ordinary demand-paged or
externally-truncatable mappings, disks, streams, RPC endpoints, and
device-only GPU storage use fallible services or handles instead.

Three parts of this model are mechanically enforced: projection is pure,
operations are explicit, and the primitive vocabulary is compiler-owned. Two
physical claims remain admitted and separately attributed: that the backing
behaves as its provider profile says, and that the selected nominal placement
describes the identified device. Compatibility between those two declarations
is derived; neither declaration proves the physical world.

Hardware-shaped structures use the same programmable layout mechanism as
Omega-native and protocol formats. Name-keyed fragmented bit placements are
needed for structures such as x86 IDT gates, but placed-view access remains a
separate concern.

### Recast views

A recast borrow keeps one storage address while exposing a second checked
shape. The spelling states both the borrow polarity and target shape:

```omega
let read: &u32 = &self.word as &u32;
let write: &mut u32 = &mut self.float as &mut u32;
let interior: &mut u32 = &mut self.bytes[offset] as &mut u32;
let header: &mut Header = &mut self.bytes[offset] as &mut Header;
let words: &mut [u16] = &mut self.bytes as &mut [u16];
```

Recast applies to ordinary values and storage, not to `Placed<P, T>` views or
their accessors. A view-to-view reshape could expose fields that the source
access plan made inaccessible even when both views used `External`
observation. Such attenuation is not yet a source feature. Code holding only a
narrowed placed view cannot recast around it; code holding the underlying
extent loan may request another placement and undergo admission again. Reading
an external container into an owned integer and recasting those detached bits
into a compatible snapshot record is ordinary value recast and remains legal.

Borrow checking retains the source loan for validated whole-name and
whole-member recasts, with the authored shared or mutable polarity. An indexed
recast may cover a wider byte footprint than its syntactic source element. The
first precise indexed rungs therefore admit only an exact literal offset into a
fixed byte array with a fact-free primitive target, a recursively nonzero
literal-array target, a nonzero closed acyclic tree of quotient-free,
all-relevant fact-free records, or the one direct phantom-lifetime application
described below. The validated complete half-open target footprint, including
normalized record padding, rather than the selected byte alone, enters overlap
facts with the authored shared or mutable polarity. Runtime or merely bounded
offsets, slices, nested lifetime applications, invariant-bearing/erased/cased
records, and other indexed forms remain conservative.

The source and target must cover the same bytes under their normalized layout
plans. A shared recast may only weaken facts (source implies target). A mutable
recast requires implication in both directions so writes through the view
cannot invalidate the source type. This is a static representation judgment,
not an unchecked transmute or a value conversion. The implemented mutable
subset accepts equal-width fact-free primitive places, proven-in-bounds scalar
offsets into byte regions, and recursively fact-free fixed record shapes over
such regions. Those record shapes may contain literal-length fixed arrays,
including arrays of nested records. A plan-reflected fixed array remains one
`Repeated` field and a recursively fixed record remains one `Nested` field;
both require one whole-field `At` placement rather than scalar bit/integer
placement or active field access. Record fields and statically or dynamically
indexed array elements follow ordinary or validated plan-laid offsets
recursively; reads and writes preserve the complete scalar footprints in both
native backends and the interpreter. Top-level literal-length arrays use the
same recursive judgment. An unsized slice target consumes the complete source
representation; its runtime element count is
`source_byte_count / target_element_byte_size`, and a remainder rejects rather
than truncating. Typed shared aliases may weaken facts, while typed mutable
aliases require bidirectional representation equivalence. Raw bytes can target
only recursively fact-free elements, so neither an array nor a slice recast can
establish element facts. These structural views preserve indexed read/write identity
through state forwarding in both native backends and the interpreter. The
recursive representation judgment also repeats through aggregate slice
elements: typed fixed arrays and differently named record-element slices may
alias when padded element stride, leaf offsets, and leaf representation sets
agree. Shared aliases may weaken each repeated leaf; mutable aliases require
exact equivalence. Float ranges compose by interval inclusion only when both
views use the same float carrier; equal intervals may alias mutably. The same
leaf rule composes through typed record views. A shared view may forget the
interval by exposing the same bytes through an unconstrained equal-width
carrier. Cross-carrier mutable equivalence remains fenced, because a numeric
interval is not an enumeration of IEEE bit patterns.

For borrow tracking, a recast at one exact literal byte-array index retains the
complete normalized half-open footprint when the target is a fact-free
primitive, an eligible closed fact-free record, or a recursively nonzero
literal fixed array ending in either an exact non-Boolean fixed-width primitive
or such a record. Primitive-terminal arrays must be exactly tiled.
Record-terminal arrays instead repeat the complete normalized record extent,
including internal, tail, and inter-element padding. Both extents come from the
shared layout representation rather than source-level size arithmetic, and
record resolution follows exact symbol identity. Eligible records may contain
recursively literal array fields ending in the same exact primitive or closed-
record shapes. A zero-length field participates only when its terminal
independently qualifies and the containing record remains nonzero; it adds no
leaves but its element alignment can still induce protected padding. Total
zero-size targets remain conservative on this indexed-loan path. A fully
specialized type plus scalar-integer `const` or exact-replayed acyclic
structured-data `const` instance uses its exact synthesized symbol and
already-substituted fields as the schema. Its retained base and argument tuple
are validated provenance. Scalar values are canonical decimals within their
exact integer carriers. A structured atom is completely decoded under fixed
byte/depth/node bounds, then its ordered fields, selected pure-sum case and
payload, nested literal arrays/records/sums, and integer/Boolean leaves are
replayed against the exact resolved monomorphic carrier. Layout comes only from
the substituted instance field types, never the encoded value or rendered
name. A direct lifetime-only application of an otherwise eligible synthesized
record instance also participates when the application has the exact nonempty
declared lifetime arity and no residual runtime arguments. Exactly one further
lifetime-only synthesized record shell may occur beneath that root. Both
shells independently replay their exact synthesized symbol, generic origin,
declared lifetime arity, and empty residual runtime-argument list. Checked
trees retain the authored lifetime spellings while erased physical
representation comes only from those synthesized symbols. Ordinary recast
validation and precise loan sizing share this bounded resolver. A
lifetime-generic array or a third lifetime shell remains fenced; an ordinary
named record wrapper beneath an array does not reset that fence. Open or
unresolved applications and mixed, recursive, custom-canonical
structured-const, malformed/nonphantom lifetime, machine, or proposition
generic instances remain conservative.

An interior slice recast starts at a proven index in a fixed byte array and
consumes the complete remaining region. Its descriptor is
`{ pointer = &bytes[offset], length = (capacity - offset) / element_size }`.
Raw targets remain recursively fact-free and exactly tiled. A runtime offset is
therefore sufficient for byte elements; a multi-byte or aggregate element
requires a statically exact offset unless the proof system can establish the
needed congruence. Merely proving the footprint is in bounds is not enough.
Both native backends and the interpreter preserve this tail descriptor through
mutable state forwarding.

The same judgment applies to scalar aliases. `bool` has the exact established
representation set `{0,1}`: it may be viewed through a shared unconstrained byte
because that forgets a fact, and a typed `u8 [0..=1]` may be shared or mutably
aliased as `bool`. An unconstrained byte or arbitrary byte region cannot be
viewed as `bool`, and a mutable `bool`/unconstrained-byte alias rejects, because
either would permit a write that invalidates the other shape. No scalar recast
is fact establishment. Constant integer ranges compare their canonical
two's-complement bit-pattern sets, merging adjacent or overlapping intervals;
therefore `i8 [-128..=127]` and unconstrained `u8` may alias mutably, while
equal-cardinality sets at different bit positions still reject.

See [`Programmable Layouts`](../design_briefs/programmable_layouts.md) and the
[`OS Memory And Hardware Foundation`](../design_briefs/os_memory_and_hardware_foundation.md)
for the settled public model and remaining engineering work.

## Endianness

Native layout follows the target. Wire protocols must declare byte order or use
field encodings that define byte order independently.

## Relationship To Serialized Bytes

Serialized layout is not native layout.

Native layout optimizes in-memory access; a serialized layout (a *layout
policy* chosen at the carrier — [Wire Protocols](chapter_21_wire_protocols.md),
`design_briefs/programmable_layouts.md`) optimizes compatibility and decoding.
A value has exactly one in-memory form; a schema may serialize through many
policies. The two coincide only by explicit contract: a fully static policy in
type position makes the plan *be* the in-memory layout, and crossing a
boundary with such a value is a borrow, not an encode — the copy vanishes by
theorem, not by accident.
