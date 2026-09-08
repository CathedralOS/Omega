# Chapter 20: Memory Layout And ABI

A type describes a value; a layout describes its representation. Layout alone
does not establish that bytes contain a valid value, that you own the backing,
or that a device permits the access you want to perform.

This chapter introduces ordinary storage, boundary calls and placed memory.
The reference contracts are [layout plans](../spec/layouts/plans.md),
[calling plans](../spec/build/calling_plans.md),
[placed access](../spec/resources/placed_access.md), and
[recasts](../spec/layouts/recasts.md). Examples explain intended semantics,
not a claim that every placement operation has native implementation.

## Zeroed Storage And Establishment

The all-zero bit pattern is safe storage for every checked-shape `data` type.
Whether it already establishes an accessible value is a separate question.

```omega
data Inventory {
    gold: u32;          // zero: 0 gold
    items: [Item; 8];   // zero: 8 zeroed items
    label: [u8; 64];    // zero: 64 zero bytes
}
```

Integers, floats and Booleans have ordinary zero values. Fixed arrays contain
exactly their declared number of inline elements, with no hidden live-length
word. A collection with a live length is a different record: zeroing its length
may make it empty, but does not change the array's capacity.

Aggregates zero their fields recursively. A sum's zero tag selects its first
declared case, including a zeroed payload. That payload need not be empty:
the first case could be `Integer(0)`. Layout optimization must not repurpose
zero into a different meaning or an invalid representation.

A field constrained to `1..=100` illustrates the establishment gate. Zero remains
safe storage, but code cannot observe that field as an established value until
construction proves the constraint. The gate propagates through common fields
and the active first-case payload, not inactive later cases. See
[default domains and zero initialization](../spec/language/dependent_values.md#default-domains-and-zero-initialization).

Fields have no declaration-site default initializers. Nonzero or computed
construction belongs in ordinary machines. Zeroing does not create authority,
validation history, or semantic emptiness. Publish the latter explicitly:

```omega
data Command {
    case None;
    case Say(text: [u8; 256]);
}

pub domain Command::Inert
requires
    self in Command::None;

pub machine Command::empty() -> command: Command in Command::Inert {
    Command::None
}
```

Consumers requiring inertness ask for `Command in Command::Inert`; they do not
infer it merely from a payload-free first case.

### What a zeroed value is, concretely

A newly zeroed scalar reads as zero only when its type is established. A zeroed
sum exposes its first case only when that case's required payload is established.
An empty borrowed descriptor does not dereference its zero pointer.

Changing a sum from a larger case to a smaller one may leave stale tail bytes.
Those bytes are not part of the new value: destructuring and equality inspect
only the active case. Padding and inactive payload are not a serialization API.

## Default Layout

Ordinary data leaves representation choices to the compiler:

```omega
data Player {
    health: i32;
    gold: u32;
}
```

Do not infer a foreign record layout from declaration order, total size or a
particular compiler's current offsets. Internal layout choices may change unless
the declaration selects a stable representation policy.

## Stable Representation

A fully static policy can make a declared layout the value's in-memory form:

```omega
data WinHandle in CLayout {
    value: u64;
}
```

Here `CLayout` denotes a layout policy, not a new modifier or unchecked ABI string.
The policy receives the schema and returns a plan that must validate. The semantic
field remains `value: u64`. Platform and wire formats use the same policy mechanism,
with different applicability requirements.

The [placement vocabulary](../spec/layouts/plans.md#placement-vocabulary) covers
whole fields, stored integers and fragmented bits. A new format normally needs
a policy; a new primitive needs compiler support. Programmable union and
runtime-stride source forms remain unspecified; conventional sum reports do not
define them.

## Alignment And Padding

Review field offsets and sizes, alignment, padding, total extent and target
assumptions together. A correctly sized buffer can still have the wrong alignment
or field encoding.

An `[erased]` binding remains part of semantic identity and custody but contributes
no physical field, padding or transfer. Its proof or authority does not occupy
hidden bytes. A calling policy classifies the
[erased-stripped public shape](../spec/build/boundary_shapes.md#public-aggregates),
not a guessed private representation. An all-erased record has no by-value ABI
carrier. Padding is not semantic data; protocols must not depend on uninitialized
padding bytes.

## Fat Descriptors

A slice or text window can be understood as a pointer paired with a length.
For slices the length counts elements; for text windows it counts bytes.
Ownership is a semantic obligation, not something established by copying those
two words.

Subslicing conceptually selects a starting element and a shorter length:

```text
start_byte_offset = start * element_byte_size
new_length = end - start
```

Bounds, source lifetime and element stride must agree. An empty view must not
force a dereference or an unjustified one-past address.

The compiler's descriptor is not a public foreign ABI. An API may expect
separate pointer/length arguments, a particular descriptor record, or a
terminated string. A checked adapter exposes the safe Omega view while the
native leaf declares the actual [foreign shape](../spec/build/boundary_shapes.md#foreign-declarations).

## Calling Conventions

Omega's internal calling convention is compiler-owned. A boundary pins an
observable promise through `Calling<C, Policy>`, with an exact named
`Policy: C satisfies CallingPolicy` conformance.

| Plan | Question |
| --- | --- |
| `CallPlan` | Where do arguments/results go, and what registers, stack space and control transfers does the ABI require? |
| `StatePlan` | What machine state exists on entry, what must be preserved, and what transitive state use is permitted? |

An interrupt entering an existing activation shows why these differ. Knowing
argument registers does not describe interrupted-state preservation. Likewise,
an entry-stack choice does not imply that hardware arrival, adapter work and the
body all occupy that stack. Their [machine-state evidence](../spec/build/machine_state_evidence.md)
is checked against the published promise.

The policy may reject an incompatible signature. Accepted plans are validated
and canonicalized before inbound or outbound machinery uses them. Refactoring
policy source without changing the normalized promise preserves ABI identity;
changing observable placement or preserved state does not.

The requirement selects the convention. A syscall number, DLL locator or provider
cannot silently replace it. Fixed records and arrays may be structurally classified,
but equal size does not imply equal ABI class, and Omega does not infer C array
decay. Safe slices and text do not choose foreign length types or retention rules.

Opaque values passed by value first need an exact target-closed
[representation application](../spec/build/opaque_representations.md).
Reference-only pointees need no by-value representation. The calling policy
places the derived shape; it cannot inspect private fields or invent their ABI.
Representation agreement establishes neither validity nor ownership.

Private callback destinations are declared native parameters or named private
layout demands, not source-visible function pointers. A calling policy places
those demands but cannot create or reorder them. See the worked
[callback example](chapter_19_capabilities_effects_boundaries.md#foreign-callbacks-through-platform-adapters),
and the [calling-plan](../spec/build/calling_plans.md) and
[private-callback](../spec/build/private_callbacks.md) contracts.

## Placed And Externally Mutable Memory

Ordinary values remain ordinary. A local `Point`, an owned array and `&mut T`
use normal field access, not placement accessors. Placed storage is useful for
MMIO, DMA-visible memory and concurrently shared pages whose authority and
observation behavior must remain explicit.

Starting with `&mut [u8]` already grants ordinary byte access. A later cast cannot
retract that grant or turn it into device-safe access. Ordinary `&write T` lets a
checked callee overwrite an existing valid value without observing it; it does
not describe device access or uninitialized storage.

### Geometry, demand, and supply

Keep three questions separate:

- `LayoutPlan`: where are the fields and bits?
- `AccessPlan`: which primitive operations does this view request?
- `ResourceProfile`: what can the admitted backing actually support?

A nominal placement policy combines layout, access and required reach. This
schematic policy assumes the named layout and access machines are in scope:

```omega
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

`Placed<UartMmio, UartRegisters>` interprets qualified backing; it is not a newly
allocated record. Projection is pure and returns an accessor:

```omega
let status = uart.status.read();
uart.transmit.write(byte);
```

Only the explicit operation performs the event. Its accessor stays tied to the
view's exact field, plan, range, lifetime and authorization. Nominal policies
remain distinct even if their geometry matches: each owns its binding and
private projection surface.

### Access plans

Access decisions use opaque schema-field keys, not integer positions. Starting
from inaccessible defaults makes omission deny access; reordering a declaration
cannot transfer permission to another field. Access policy selects operations,
not a second copy of the geometry.

| Observation | How to think about it |
| --- | --- |
| Stable | Backing does not change behind the loan. Mutation still needs both source and current exclusivity. |
| External | Each admitted transfer occurs once at its specified width. Device access is an explicit event. |
| Atomic | Only individually admitted atomic operations and widths are available. |

Two bitfields in one device word do not necessarily permit independent writes:
read-patch-write touches the whole container. Stable exclusive storage may permit
it; External storage needs a complete transfer or an explicitly supplied masked
operation. A generated write must not hide a compare-exchange retry loop.

A destructive read consumes one whole container snapshot. Read once, then project
fields from the owned result. Separate destructive accessors cannot each consume
the same event. W1C, read-back-to-flush and coherent snapshots belong in authored
protocol machines, not additional access-plan cases.

Helpers can accept just the operation they need:

```omega
machine send_byte<T, Write: T satisfies Writable<u8>>(
    transmit: T,
    byte: u8
)
{
    Write::write(transmit, byte);
}
```

An exclusive borrow of an External accessor serializes that value, not the device.
Binding-private exposure controls naming and issuance; passing the accessor
delegates its public operation requirements. Copyability, concurrent sharing and
bounded-use permits remain separate concerns.

Atomics preserve ownership too. Load needs a copyable resident; store needs
permission to discard displaced content; swap returns it. Observing compare-exchange
failure exposes the resident and requires copyability. Non-observing variants
return proposed custody on failure and displaced custody on success. `Once` means
single-attempt, not non-observing. The [exact outcomes](../spec/resources/placed_access.md#atomic-compare-exchange-outcomes)
explain these independent choices.

### Admission and placement

An extent's address and length describe geometry, not authority:

```omega
pub data Extent [linear] {
    base: addr;
    length: u64;
}
```

Placement needs the exact source qualified as `Extent in Granted`. Reconstructing
numeric fields does not reconstruct the grant. A caller-authored profile is not
an admission receipt: the selected provider binds its normalized profile to that
qualified range.

| Family | Use it when |
| --- | --- |
| `view_borrowed` / `view_owned` | Content is already established for this interpretation. |
| `initialize_borrowed` / `initialize_owned` | You own a new `T` and exclusive vacant Stable storage. |
| `validate_borrowed` / `validate_owned` | Stable bytes need inspection by a checked static validator. |

Validation establishes representation and predicates, not ownership or uniqueness.
Represented non-copy content needs initialization, existing resident custody or an
admitted transfer. Device programming is an authored protocol, not generic External
initialization.

`Vacant` means no live established value occupies the range, not that bytes are
zero. `Resident<P, T>` owns one complete value under that exact placement. Opening
and retiring a resident view transfers or borrows the same claim, never a fresh
one. Extent split/merge cannot cut or combine resident objects by geometry alone.

Non-runtime Type fields still need custody. An authored record and explicit named
conformance account for them without hidden representation bytes:

```omega
data PacketCustody {
    authority: DeviceAuthority;
}

pub PacketNativeCustody:
    PacketCustody satisfies PlacementCustody<Native, Packet>;
```

The conformance checks exact field paths, types, multiplicities and plan decisions.
The custody type alone does not trigger ambient search. Case-dependent custody
follows an authored operation identifying the case. Proof conclusions cannot
replace owned Type values.

Dynamic rejection returns what the operation took:

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

Owned rejection returns source and custody. Borrowed rejection ends the loan and
returns moved custody. Every other input also needs an explicit disposition;
missing output is not proof of consumption. The selected validator's full
contract and declared error sum remain visible, as specified by
[establishment and retirement](../spec/resources/placed_access.md#establishment-and-retirement).

This schematic transition handles both outcomes; the caller supplies its policy,
schema, exact custody conformance and validator:

```omega
transition Placement::validate_owned<
    Native, Packet, PacketCustody, PacketNativeCustody, Validator
>(move source, move custody, move revision_ticket) {
    PlacementOutcome::Ready { view } ->
        use(view)

    PlacementOutcome::Rejected {
        returned: { source, custody },
        reason
    } ->
        recover(move source, move custody, reason)
}
```

Success embeds source and custody in the view; rejection returns them. If revision
checking consumes the ticket on both paths, its contract must justify both
dispositions. A statically proved check may make rejection impossible, but does
not silently unwrap the sum: an irrefutable case pattern still needs that proof.

Retirement accounts for everything embedded. Complete owned Stable content can
be destroyed or moved out, returning `Vacant`, or retained with the same resident
claim. A borrowed resident view ends only its loan. Partial content must be restored
or fully disposed of before retirement; a crash record proves neither vacancy nor
a complete resident. In-place migration takes out the old value, reaches vacancy
and initializes the new value only if its footprint fits. Otherwise use another range.

### Stored integers

A foreign record can keep one semantic type across targets despite different
stored widths. For example, an `i64` field may use signed 32-bit storage on one
target and signed 64-bit storage on another.

`IntegerAt(offset, stored_width, interpretation)` expresses that choice. Offset
counts bytes; width counts bits. Reading loads exactly those bits and sign- or
zero-extends into the semantic carrier. Writing the signed 32-bit form needs
proof that the value lies in `-2147483648..=2147483647`, unless its admitted type
already guarantees that range. There is no implicit truncation or invented
fitting domain.

Fit is not transfer permission. It does not authorize a device write, a partial
container update or mutation through a shared Stable source. By-value ABI
classification uses the physical stored width and alignment; field projection
recovers the wider value after transfer. A mutable raw-byte view preserves the
encoding and write-fit obligation; differently represented typed records do not
thereby become interchangeable aliases.

More complex conversion can use a checked adapter. The
[layout contract](../spec/layouts/plans.md#placement-vocabulary) owns encoding;
the [implementation note](../../omega-rust/psi/semantics/build-time-evaluation/layouts.md)
records supported widths and materialization shapes without narrowing the language.

### Loans, aliases, and phases

Stable mutation needs plan permission plus exclusive current and source borrows.
Taking `&mut` to a view derived from shared backing cannot upgrade it. Conflicts
cover complete effect footprints, not just named logical fields.

Alignment is a whole-placement question. Every access contributes:

```text
(base + field_offset) mod alignment = 0
```

Inconsistent requirements reject the plan. Admission checks the combined base
condition against geometry or provider evidence. It does not prove that the
schema describes the device: correspondence is separately admitted for the exact
device instance and grant.

A DMA buffer illustrates observation phases. While the device owns it, CPU
access may be absent or External. Exact completion and release evidence can restore
a Stable CPU loan. A status read alone does not release custody, and a publication
proof cannot replace its ordered operation. See [device protocols](../spec/resources/device_access.md).

Facts and non-runtime bindings remain tied to exact places, mapping, lease and
revision. Writes must preserve those dependencies or supply replacements; an
accessor cannot silently weaken the semantic type or discard custody. Relationships
spanning several views use ordinary carriers borrowing them, with the external
stability evidence needed for their lifetime.

### Applicability

Placed primitives need stable addressable lifetimes and finite, non-suspending
operations with no recoverable failure under the admitted contract. This bounds
program work, not cache or fabric latency. MMIO and resident mapped storage can
fit; disks, streams, RPC and device-only GPU storage need fallible services.
Durability and device completion remain protocols.

A live placed mapping retains a lease or claim preventing asynchronous revocation.
Revocable mappings use a distinct fallible protocol, not surprise failure on every
field access. Shared-memory layout agreement does not establish a lock protocol
or make a hostile peer cooperative.

A declared API or validated plan is not end-to-end execution support. The
[placement contract](../spec/resources/placed_access.md) links remaining work;
code-adjacent notes describe current lowering and custody limits.

### Recast views

A recast borrows the same storage under another checked shape:

```omega
let read: &u32 = &self.word as &u32;
let write: &mut u32 = &mut self.float as &mut u32;
let header: &mut Header = &mut self.bytes[offset] as &mut Header;
let words: &mut [u16] = &mut self.bytes as &mut [u16];
```

These require compatible representations, not just equal byte counts. Shared
views may forget facts; mutable views need equivalence in both directions so
writes cannot invalidate the source when the loan ends. Recasting performs no
validation or executable conversion.

For example, `bool` has representations `{0,1}`. A shared unconstrained byte view
forgets a fact. A mutable unconstrained byte view would permit writing `2`, so
it rejects. Raw bytes cannot establish Boolean validity merely by receiving a type.

An interior recast borrows its entire target footprint, including padding, not
only the selected byte. A slice must exactly tile the source region: length is
byte extent divided by element stride, with no ignored remainder. Bounds alone
do not establish alignment or the congruence needed for multi-byte elements.

Recasts exclude `Placed<P, T>` and its accessors: another shape could expose a
field denied by the access plan. Recast a detached ordinary snapshot, or request
another placement through the underlying qualified extent. See the
[recast judgment](../spec/layouts/recasts.md) and
[implementation limits](../../omega-rust/psi/semantics/validation/recasts.md).

## Endianness

Native layout follows the target. Wire protocols declare byte order or select
field encodings that define it independently.

## Relationship To Serialized Bytes

Native and serialized layouts solve different problems. A value has one in-memory
form, while its schema may support several encoding policies. They coincide only
through explicit agreement: a fully static type-position policy can make the
encoded representation the in-memory layout, allowing a compatible borrow rather
than an encode.

A codec still owes its agreement and validation contract. Generated code does
not automatically have derived trust; decoding establishes neither authority nor
device correspondence. Checked validation establishes its promised predicates;
routed provenance additionally needs its declared authorized establishment route.

Wire formats and historical migration use ordinary declarations and machines:

```omega
data CounterDiskV1 {
    #1 counter: i32;
}

data CounterDiskV2 {
    #1 counter: i64;
    #2 timestamp_seconds: u64;
}

data CounterDisk {
}

machine counter_v1_to_v2(
    old: CounterDiskV1,
    out: &mut CounterDiskV2
) satisfies FormatMigration<
    CounterDisk,
    CounterDiskV1,
    CounterDiskV2
>::migrate {
    out.counter = old.counter as i64;
    out.timestamp_seconds = 0;
}
```

Stable `#N` identities survive renaming; they are not offsets or runtime
discriminants. Number every member or none within each record, sum, or payload
scope; `retired #N;` reserves a removed identity. Erased numbered fields retain
schema identity but emit no wire bytes. See [schema policies](../spec/layouts/plans.md#policy-evaluation-and-schema).

Published shapes stay immutable. Here the ordinary `CounterDisk` marker selects
one lineage and the standard `FormatMigration` requirement binds its exact
conversion. The new timestamp is an authored semantic choice, not an implicit
wire default. Reverse conversion is separate and may fail. Independent disk,
network, and snapshot lineages need not share a runtime type's history.

An omitted optional field means `None`; a missing required field is invalid.
Strict decode rejects unknown members, projecting decode discards them, and
preserving decode retains exact unknown bytes and ordering in a codec-bound
opaque remainder. A borrowed remainder retains its input loan; an owned copy
has allocation obligations. None grants facts about unknown fields.

The [codec contract](../spec/layouts/codecs.md) owns these policies and migration
requirements. A channel or store separately requests directional reading,
writing, preservation, canonicalization, or complete peer-to-local migration
through [build compatibility](../spec/build/configuration.md#directional-wire-compatibility).
These are package patterns, not intrinsic versions or special migration syntax;
the example illustrates the contract without promising every codec realization
is implemented.
