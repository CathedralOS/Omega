# Chapter 20: Memory layout and ABI

A layout determines how a value is stored: field offsets, alignment, and byte
order. Use an explicit layout when matching a wire format, foreign ABI, or
device register map.

[Semantic reflection](chapter_21_reflection.md) supplies the fields and cases
that a layout policy places.

This chapter introduces ordinary storage, boundary calls and access views.
The reference contracts are [layout plans](../spec/layouts/plans.md),
[calling plans](../spec/build/calling_plans.md),
[access policies](../spec/resources/access.md), and
[recasts](../spec/layouts/recasts.md). Examples explain intended semantics,
with layout, access policy, and backing authority kept distinct.

## Zeroed storage and establishment

The all-zero bit pattern is safe storage for every checked-shape `data` type.
Whether it already establishes an accessible value is a separate question.
Source literals separately require [explicit `..` for field omissions](chapter_1_data_values_literals.md#explicit-zero-defaults);
storage zero-initialization does not authorize silent omissions.

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

Aggregates zero their fields recursively. A sum's zero representation selects its first
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

public domain Command::Inert
requires
    self in Command::None;

public machine empty_command() -> Command in Command::Inert {
    Command::None as +Command::Inert
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

## Default layout

Data declares semantic structure, not an ambient representation:

```omega
data Player {
    health: i32;
    gold: u32;
}
```

Do not infer a foreign record layout from declaration order, total size or a
particular compiler's current offsets. Ordinary storage declarations use the
default representation when no explicit family is selected. That default is a
rule of the storage use, not a policy attached to the data declaration.

The compiler can optimize this home layout: pack fields, remove a separate sum
tag, or eliminate storage altogether when the program's behavior is preserved.
For an optional nonzero integer, a possible compact encoding is:

```text
stored zero       -> None
stored nonzero n  -> Some(n)
```

The compiler must also preserve legal repairs, borrows and cleanup. A predicate
that permits temporary zero during repair needs more reasoning than the table
alone supplies. The encoding is an opportunity, not a promised size for every
optional type.

A cast that shares storage constrains both layouts. The compiler can shrink both
if reads and writes through either view still agree; otherwise it keeps a
compatible layout. Two twelve-byte layouts need not be compatible. Explicit
placements and exposed bytes or addresses also constrain optimization, and
`size_of` and `align_of` describe the selected target layout. See
[home-layout realization](../spec/layouts/plans.md#home-layout-realization).

## Dense owned collections

An allocator-backed vector owns two adjacent regions: established elements and
vacant spare capacity. A core `Run<P, T>` under its protected `Live` qualification
holds the first region and its element count. The compiler understands its
ownership contract; allocation and Vec growth remain ordinary library code.

```text
one allocation: [ T | T | T | T ][ vacant capacity ]
                 live run: 4
```

Push and pop operate at the boundary between those two regions:

```text
push: take one vacant cell -> initialize it -> merge it onto the live run
pop:  split off the last live cell -> extract it -> return its vacant storage
```

Initialization can reject. Push then joins the returned vacant cell back onto
the spare tail and returns the original value to its caller. The live elements
stay intact. Core supplies the checked run split/merge operations; the vector
library handles allocation, capacity, growth and this recovery path.

There is no tag or ownership table beside each element. Each live value retains
its own obligations: four tasks can own four different resources. Pop transfers
one actual task out; release must account for every remaining task before the
backing can become vacant. Zero-sized values still count and still owe their
required disposition.

Removing the tenth element need not consume the vector. A library `swap_remove`
moves that element out, fills its place with the last element and shortens the
live range. A temporary hole uses the ordinary exclusive-borrow restoration
window. If holes must remain, use an explicit shape such as `Optional<T>`.

With ordinary native layout on Stable RAM, the live region can be borrowed as
`&[T]` or `&mut [T]`. Qualified floats remain dense floats suitable for SIMD:
a shared `&[f32 in Km]` can be read as `&[f32]` once the removal is written.
A mutable view may omit only domains its lender could add again, and writing
through it must preserve or restore the original predicates before returning it.
Casting does not discard access restrictions.

Splitting and merging runs take constant runtime work: they update descriptors.
Swaps move data, and disposal runs required cleanup for each element that needs it.
The compiler checks the repeated custody transformation symbolically; it does
not demand per-element runtime bookkeeping. See [dense owned runs](../spec/resources/typed_runs.md)
for the complete contract and the distinction from single-value resident storage.

## Stable representation

A storage or parameter declaration can select a library representation:

```omega
data WinHandle {
    value: u64;
}

machine consume(handle: &CLayout<WinHandle>) {
    inspect_handle(handle);
}
```

`WinHandle` remains layout-neutral. `CLayout<T>` is a
[transparent generic alias](../spec/language/generics.md#transparent-type-aliases)
for the general core fixed-storage constructor with a selected library C ABI
policy. A community package can define another family through the same facility;
the compiler recognizes neither C nor protobuf by its spelling. Data declarations
cannot select a layout through `in`, and domain qualification never changes
geometry. See [representation families](../spec/layouts/plans.md) for the underlying constructors.

Policies are ordinary named implementations, not empty data types with attached
machines. The compiler supplies the authorized schema and explicit target facts,
evaluates the pure terminating policy, validates its plan and realizes the
representation. Build declares dependencies and target/configuration; it does
not register or bless each layout. A foreign call retains its separate selected
provider, calling plan and authority contract.

For a complete C structure, the reusable policy computes placement and padding.
A sparse foreign view needs an explicit custom implementation: a record listing
only Darwin timestamp seconds omits the native nanoseconds fields, so a C policy
cannot infer the real structure from it. Complete declarations or explicit sparse
policies belong in the platform library, not a compiler-owned OS catalog.

The [placement vocabulary](../spec/layouts/plans.md#placement-vocabulary) covers
whole fields, stored integers and fragmented bits. A new format normally needs
a policy; a new primitive needs compiler support. Programmable union and
runtime-stride source forms remain unspecified; conventional sum reports do not
define them.

## Representation conversions

Standard fallible `TryFrom` contracts connect selected complete representations
through semantic T. Libraries author reusable conversion rules; typed reflection
and specialization supply each supported type's field and active-case work.
Conversions are explicit, not silent coercions or bitcasts. Output buffers,
allocators and custody remain explicit inputs. An infallible convenience may
delegate to the same implementation only after totality is established.

`ProtoLayout<T>` may be a community library's encoded-storage family. Its
variable-length bytes do not promise C-style field addresses or fixed size.
Unknown-field handling and historical migration are explicit codec contracts,
not an automatic consequence of sharing T. Geometry-only and one-way views do
not falsely promise complete conversion. See [representation conversions](../spec/layouts/codecs.md#representation-conversions).

## When layout is needed

Importing a library does not calculate every layout used by its unused code.
A console-only program need not lay out unused filesystem records merely
because they live in the same library. Source checks still apply to that code.

An actual storage, layout query, entry or selected Build-row closure demands
the corresponding concrete layout, which must validate before use. Rows for
other declared targets count even when only one target is emitted. A later
consumer can therefore reveal an incompatibility in a previously unused layout;
deferral does not make that layout valid or weaken memory-access requirements.
See [layout demand and validation](../spec/layouts/plans.md#layout-demand-and-validation).

## Alignment and padding

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

## Fat descriptors

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

## Size and alignment

Use the compiler-known core queries on a data type:

```omega
const player_bytes: u64 = size_of<Player>();
const player_alignment: u64 = align_of<Player>();
```

They report the selected target's validated layout, including padding in the
size. Erased fields occupy no storage. A collection's own representation is not
the total size of its allocations, and measuring a reference does not measure
its referent. Fixed size and known alignment are separate requirements; a
variable-length encoding needs an ordinary sizing machine given the value.

These names are not keywords or casts. No `usize` or alignment carrier is needed.
Interpretation uses the selected target layout, not the size of an interpreter
object. A proof-only type without a runtime layout does not receive a fake zero
size. See [layout queries](../spec/layouts/plans.md#size-and-alignment-queries).

The value form asks about an actual location instead:

```omega
machine header_is_aligned(packet: &[u8]) -> bool
requires packet.len >= 16
{
    align_of(packet) >= 16
}
```

For a borrowed slice this measures its first element's location, not the
descriptor, and requires a nonempty slice. It can establish stronger alignment
than `align_of<u8>()`; it still supplies no bounds or access authority. The
observation may need runtime computation. An offset view must establish its own
alignment, and interpreted address IDs are not physical alignment evidence.
See [location alignment](../spec/language/counts_and_addresses.md#location-alignment).

## Exposing addresses

Use `address_of` to observe an executable entry or borrowed storage:

```omega
machine identity<T>(value: T) -> T { value }

machine addresses() {
    let value: u32 = 7;
    let code_address: addr = address_of<identity<u32>>();
    let storage_address: addr = address_of(&value);
}
```

The first query selects a concrete machine application; the second observes
`value`'s location. Neither invokes the machine or grants permission to access
the observed address.

Native execution supplies addresses. Interpretation supplies stable IDs for
entries and storage in separate namespaces. Those IDs support comparison,
not native byte offsets or instruction patching. Local addresses depend on
the running invocation; final native entries may depend on loading.

Taking an entry address keeps an executable entry available even if other calls
are inlined. Code sharing can give distinct machines the same native address,
so address equality is not declaration identity. See [address exposure](../spec/language/counts_and_addresses.md#address-exposure).

## Inspecting emitted code

An exact native realization has more useful information than a total byte count:

```omega
code_layout<identity<u32>>()
```

The description contains its entry address and code regions, each with a base
and byte length. Split regions need not be adjacent; shared instructions do not
become exclusive property of the queried machine. Called machines, padding and
data pools are not automatically part of its code. `size_of<identity<u32>>()`
rejects because a machine declaration is not a data type.

These regions are finalized after lowering/layout, with addresses resolved by
linking/loading where needed. They are not constants that can determine that
same program's types or layout. A purely interpreted body has no native code
regions; an entry ID is not a bytecode pointer.

For a native memory editor, `Extent { base: address_of<identity<u32>>(), length: 2048 }`
describes a window, not the machine's actual size. Its nonwrapping geometry and
backing/write authority remain separate obligations. Code metadata grants no
patch permission and does not stop other calls from inlining. See
[code-layout queries](../spec/language/counts_and_addresses.md#native-code-layout-queries)
and [root-assumed contracts](chapter_19_capabilities_effects_boundaries.md#root-assumed-binding-contracts).
That exceptional route is not a fix for a rejected borrow or missing write
authority in ordinary code. Typical programs should reject it; a deliberate
code-modification experiment must identify its departure from the checked model
and accept the resulting conditional guarantees.

## Calling conventions

Omega's internal calling convention is compiler-owned. A boundary pins an
observable promise through `Calling<Policy>`, where `Policy` is an exact named
`CallingPolicy` conformance.

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

## Access policies and externally mutable memory

A device register can change independently of Omega's borrows. Its access
policy describes the reads and writes the hardware supports. Given a UART
view whose status is read-only and transmit register is write-only:

```omega
machine send(uart: &mut (UartRegisters in DeviceRegisters), byte: u32) -> u32 {
    write(&mut uart.transmit, byte);
    read(&uart.status)
}
```

Each explicit operation performs one device event. Reading `transmit`, writing
`status`, or borrowing either as unrestricted RAM rejects. The mutable borrow
serializes use of this view; it cannot prevent the device changing its status.

The platform's provider supplies the qualified view with its backing, lifetime
and device correspondence established. Application code passes that view to
helpers and performs the permitted operations.

### Author an access policy

A provider author connects three independently checked descriptions:

| Description | What it answers |
| --- | --- |
| `LayoutPlan` | Where and how the semantic fields are stored. |
| `AccessPlan` | Who may access them and which observations are permitted. |
| `ResourceProfile` | What the actual backing supports. |

For the UART, the access plan restricts establishment to the provider, denies
status writes, denies transmit reads, and marks the transfers External. The
layout separately supplies their offsets and widths. The provider establishes
that this layout describes the actual device.

Use [access plans](../spec/resources/access.md) for the complete construction
API and [layout plans](../spec/layouts/plans.md) for placement. Writing down a
plan alone cannot grant access to an address.

### Choose an observation contract

| Observation | Use it for |
| --- | --- |
| Stable | Backing that stays unchanged behind a shared loan. Writes need exclusive access. |
| External | Device transfers performed once at their specified widths. |
| Atomic | Individually supported atomic operations and orderings. |

Two bitfields in one device word may share a write footprint. Read-patch-write
touches the whole word and needs permission for that transfer. A destructive
read similarly consumes one snapshot: read once, then inspect its fields.
Write-one-to-clear and read-back-to-flush behavior belong in protocol machines.

### Establish and retire a view

A range's address and length describe geometry. Its `Granted` qualification
supplies the backing authority needed for access. Choose the establishment
operation according to what already occupies that range:

| Family | Starting state |
| --- | --- |
| `view_borrowed` / `view_owned` | A value is already established there. |
| `initialize_borrowed` / `initialize_owned` | A new value and exclusive vacant Stable storage. |
| `validate_borrowed` / `validate_owned` | Stable bytes to inspect with a checked validator. |

`Vacant` means no live value occupies the range; it need not contain zero bytes.
`Resident<P, T>` accounts for one complete value under the selected placement.
Validation proves representation and predicates, while ownership and uniqueness
need their own evidence.

If establishment can fail, its rejection result returns the resources it took
or accounts for their disposition. Successful establishment keeps the source
and any required custody with the view. Retirement must account for them again:
destroy or move out owned content before declaring the range vacant; ending a
borrowed view ends only its loan.

See [establishment and retirement](../spec/resources/access.md#establishment-and-retirement)
for owned rejection, retained authority and partial-content rules.

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

More complex conversions can use a checked adapter. The
[layout reference](../spec/layouts/plans.md#placement-vocabulary) defines the encoding operations.

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

Access primitives need stable addressable lifetimes and finite, non-suspending
operations with no recoverable failure under the admitted contract. This bounds
program work, not cache or fabric latency. MMIO and resident mapped storage can
fit; disks, streams, RPC and device-only GPU storage need fallible services.
Durability and device completion remain protocols.

A live access mapping retains a lease or claim preventing asynchronous revocation.
Revocable mappings use a distinct fallible protocol, not surprise failure on every
field access. Shared-memory layout agreement does not establish a lock protocol
or make a hostile peer cooperative.

The [access contract](../spec/resources/access.md) defines the complete
establishment, observation, and custody obligations for these views.

### Recast views

A recast borrows the same storage under another checked shape:

```omega
let read: &u32 = &storage.word as &u32;
let write: &mut u32 = &mut storage.float as &mut u32;
let header: &mut Header = &mut storage.bytes[offset] as &mut Header;
let words: &mut [u16] = &mut storage.bytes as &mut [u16];
```

A shared recast must preserve valid observation. A mutable recast must also
allow every permitted target write to leave storage valid for its original
owner, or open an initialization window that restores validity before handback.

For example, a `bool` contains only `0` or `1`. Viewing it as an unrestricted
mutable byte would allow writing `2`. That view needs a window and a proof of
restoration before the original Boolean can be used again.

A recast also checks alignment and the complete target extent, including padding.
A slice's bytes must divide exactly into the target element stride. Equal sizes
alone establish neither alignment nor valid contents.

Access restrictions survive the recast. Turning a shared `u32` view into an
atomic view would add write access, so it rejects. With exclusive access, an
ordinary/atomic recast can be valid if its layout, value, and outstanding-loan
requirements hold. Device views retain their planned observation operations;
use a detached snapshot when ordinary memory access is needed.

See the [recast reference](../spec/layouts/recasts.md) for the full compatibility
rules and [domain access](chapter_8_domains.md#read-and-write-authority) for
reader and writer policies.

### Initializing a stricter shape in byte backing

An exclusive cast may name a target-shaped place before its content is established.
For example, an IPC header might require `version == 1` and `used <= capacity`.
Zeroed backing fails the first predicate. The permitted sequence is:

```text
exclusive, aligned backing of sufficient size
    -> cast to Header storage: establishment pending
    -> initialize version, capacity and used with valid values
    -> prove the complete Header domain through its authorized routes
    -> ordinary typed use, then publish under the IPC custody protocol
```

This uses ordinary `as`, not a new initialization keyword. The pending place is
not a valid `Header`: you cannot pass it to an ordinary header-taking machine or
read its fields using unproved representation/domain facts. Invalid enum tags and
reference bits cannot be read first and validated afterward. Geometry and exclusive
access must be proved before the cast, not repaired inside the window.

No unrestricted byte alias remains usable beside the pending typed view. Failed
initial establishment creates no whole header and invokes no whole-header cleanup,
but any initialized resource fields and original source obligations must be settled
before returning the backing. Restricted domain minting and atomic/access policies
still apply. A cast cannot replace explicit establishment of a new
operation-governed interpretation.

See [deferred establishment](../spec/layouts/recasts.md#deferred-establishment-on-exclusive-storage-casts)
and [invariant windows](chapter_11_invariant_windows.md).

## Endianness

Native layout follows the target. Wire protocols declare byte order or select
field encodings that define it independently.

## Relationship to serialized bytes

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
A package can use separate migration lineages for disk, network and snapshot formats.
