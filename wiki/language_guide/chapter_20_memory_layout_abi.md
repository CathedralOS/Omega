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
    label: [u8; 64];    // zero carrier: live length 0
}
```

What makes this hold layer by layer:

- Integers, floats, and booleans: zero is an ordinary value.
- Fat descriptors (slices and borrowed text windows): `{ ptr: 0, len: 0 }` is
  the canonical empty carrier. Reads see emptiness; nothing dereferences a
  zero pointer with a zero length.
- Bounded carriers `[T; N]` use `{ len, inline elements }`; all-zero storage has
  live length zero even though its inline capacity is `N`.
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

Direction (settled 2026-07-02, `design_briefs/calling_plans.md`): a calling
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

`Placed<P, T>` is a compiler-derived, borrow-carrying view, not ordinary record
storage. Projection is pure and yields an accessor rather than an lvalue:

```omega
let status = uart.status.read();
uart.transmit.write(byte);
```

The projection performs no access. `read` or `write` performs the authorized
event. The accessor carries the exact field identity, address, width,
observation model, plan identity, source loan, and reach needed by lowering; it
cannot outlive or exceed the placed view.

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
        write: bool,
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
the active loan. With write permission and both an exclusive current borrow
and an exclusive source loan, it derives ordinary compound mutation.

`External` means each primitive transfer occurs exactly once at an admitted
width and is neither elided nor combined with another transfer. A narrow read
may read its whole transfer container once and then project bits from the
snapshot. A write must cover a complete admitted container; the compiler never
synthesizes a read-modify-write for external storage.

`Atomic` exposes only the individually admitted atomic operations and the
ordinary atomic ordering vocabulary. Load, store, swap, compare-exchange, and
the fetch operation families remain distinct permissions. Stable access does
not imply atomic access. An active atomic loan pins one transfer granularity for
each overlapping location; simultaneously live atomic views of the same bytes
cannot select different widths.

The compiler derives small operation requirements from accepted fields:
`Readable<T>`, `DestructiveRead<T>`, `Writable<T>`, and the atomic operation
families. Helpers may accept one such accessor instead of the whole view:

```omega
machine send_byte<T>(transmit: T, byte: u8)
where
    T satisfies Writable<u8>
{
    transmit.write(byte);
}
```

A destructive read derives `DestructiveRead<T>::take(&mut self)`, never
`Readable<T>`. Whether that operation is exported or binding-private remains a
separate policy choice: a FIFO pop may be public, while a read-to-clear status
container may be wrapped by one package machine that reads once and returns an
owned snapshot.

Device protocol meaning does not become an access-plan case. W1C,
read-back-to-flush, FIFO, doorbell, lock, and coherent-snapshot behavior belong
to authored package machines over the permitted primitives.

### Admission and placement

An `Extent` is transparent geometry:

```omega
data Extent [linear] {
    base: addr;
    length: u64;
}
```

Anyone can spell the fields. That does not establish the provider-originated
facts that make the range usable. Reconstructing the same numbers similarly
does not copy authority. Useful operations require an admitted grant and an
active loan carrying space, rights, provenance, range, mapping era, reach, and
the effective `ResourceProfile`.

`ExtentLoan` below is the borrow-carrying value produced from such a qualified
Extent. It names the exact range and shared or exclusive source polarity; it is
not another root grant.

Placement checks consumer demand against provider supply once:

```omega
machine admit<P, T>(
    loan: ExtentLoan
) -> result: PlacementAdmissionResult<P, T>;

machine place<P, T>(
    admission: PlacementAdmission<P, T>
) -> view: Placed<P, T>;
```

The accepted token owns the exact loan and records the admitted provider
receipt, normalized placement identity, range and mapping era, and discharged
alignment constraint. It is consumed by `place`. Rejection returns the moved
loan with a diagnostic. A forged lookalike token carries no receipt and
establishes nothing.

The compatibility check requires every requested field interval to fit an
admitted region; every requested operation, transfer width, alignment, and
reach to be supplied; and the requested observation to be a safe use of that
backing. Stable backing may be observed conservatively through `External`
operations. External backing cannot be treated as stable. Atomic access is
available only when explicitly supplied.

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
address.

### Loans, aliases, and phases

Write authorization is the conjunction of three facts:

1. the access plan permits the write;
2. the current borrow of the placed view is exclusive; and
3. the placed value retains an exclusive source loan.

Taking `&mut` to a view placed from a shared loan does not upgrade the source.
Loan polarity is compile-time provenance carried by the view, not a runtime
flag. Multiple read-only views may coexist. Disjoint exclusive subrange views
may coexist only when a validated layout certificate or checked interval proof
establishes non-overlap. A child loan receives only the parent's profile
restricted to the child interval and attenuated rights, reach, and operations.

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
operations are explicit, and the primitive vocabulary is compiler-owned. The
provider's claim that the backing really behaves as its admitted profile says
is trusted and recorded by a receipt.

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

The source and target must cover the same bytes under their normalized layout
plans. A shared recast may only weaken facts (source implies target). A mutable
recast requires implication in both directions so writes through the view
cannot invalidate the source type. This is a static representation judgment,
not an unchecked transmute or a value conversion. The implemented mutable
subset accepts equal-width fact-free primitive places, proven-in-bounds scalar
offsets into byte regions, and recursively fact-free fixed record shapes over
such regions. Those record shapes may contain literal-length fixed arrays,
including arrays of nested records. Record fields and statically or dynamically
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
