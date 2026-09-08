# Chapter 2: Ownership, Borrowing, And Moves

Ownership determines who may move a value and who owes its final disposition.
Borrowing temporarily grants access without transferring the referent. The
[ownership specification](../spec/language/ownership.md) defines these rules;
this chapter shows their use.

## Usage Multiplicity

Omega distinguishes three type properties:

- Unrestricted (`[copy]`): may copy and discard.
- Affine (the owned-data default): may move at most once and use eligible cleanup.
- Linear (`[linear]`): must transfer or consume exactly once.

`[copy]` and `[linear]` are mutually exclusive. Multiplicity is not a trait or
a qualifier repeated on every binding. Records, active sum payloads and generic
containers retain their contents' obligations.

Constructing a valid linear value creates one obligation, even if its bits are
all zero. Implicit zero-filling creates none, and zero is not a universal
“already consumed” value.

## Owned Values

An owned value can move into another location:

```omega
machine InventorySystem::repair(&mut self, replacement: Inventory) {
    self.inventory = move replacement;
}
```

After the move, the old binding is unusable. Replacement must also account for
the old inventory's disposition.

A field selected from an owned call result belongs to that result's storage;
selection through a reference or slice reaches borrowed storage. Partial moves
must retain the remaining obligations. A type with a nominal whole-value cleanup
hook instead requires an explicit consuming decomposition, because its hook is
entitled to a whole valid value.

## Copy Values

```omega
let depth: u32 = self.level_depth;
let next_depth: u32 = depth + 1;
```

Copying `depth` does not consume it. The addition still needs its Exact overflow
proof. Integers and Booleans are common unrestricted values; unique cleanup
responsibility cannot be copied.

## Linear Values

Task lifecycle claims, transactions, acknowledgements and DMA submissions often
need an explicit conclusion. Moving one transfers its obligation; reaching a
normal exit without a legal disposition rejects. Automatic cleanup is available
only when the type owner authorizes that exact plan—not merely because a hook
exists or the bits can be discarded.

Conditional ownership uses an ordinary sum such as `Idle | Running(Task<T>)`.
Only the active payload owes the task. Substitution of a linear payload into a
generic sum cannot erase that debt.

Logical facts and permissions are separate: a proof may be reused without
duplicating the resource it describes. [Carry policy](../spec/resources/carry.md)
is independent too. Ownership transfer alone does not prove that a value may
cross a suspension, CPU, thread or storage boundary, or that shared use is safe.

## Shared Borrows

Shared borrows grant observation under their access contract:

```omega
machine RoomFormatter::render(&self, room: &Room, out: &write [u8]) {
}
```

Several shared borrows may coexist when no conflicting exclusive access exists.
A reference always denotes the original storage, not a snapshot chosen because
the object is small enough for registers. If synchronized mutation is permitted,
a shared reference still observes that same location. Its concurrency contract
remains separate from copyability.

## Mutable Borrows

A mutable borrow is exclusive for its borrowed place:

```omega
machine Player::heal(&mut self, amount: i32) {
    self.health += amount;
}
```

While the place is mutably borrowed, another active route cannot read or mutate
it incompatibly. Arithmetic and value invariants remain ordinary obligations;
exclusive access does not prove the new health fits.

## Write-Only Borrows

Write-only access exclusively lends an existing valid value without permitting
observation of its contents:

```omega
machine fill(destination: &write [u8])
requires
    destination.len > 0;
{
    destination[0] = 42;
}

fill(&write buffer[..]); // caller must establish nonempty buffer
```

`&write` has the exclusion and lifetime rules of `&mut`, with a narrower set
of operations. Mutable access can explicitly attenuate to write-only; write-only
access cannot become readable or mutable. Its receiver spelling is `&write self`,
and an explicit lifetime precedes the modifier: `&'buffer write T`.

It is not vacant output storage: the referent is a live `T` before and after
the loan. Stores, content-independent projection and view metadata such as
length are available. Loading, comparing, hashing, pattern matching, taking,
swapping and read-modify-write are not. Replacing a value does not make the
write-only loan readable afterward.

A known field can be located without inspecting content. A sum payload that
needs a tag read cannot, unless an already-established refinement fixes the
case. Whole replacement writes the tag and payload together. Displaced custody
and invariant-window obligations still apply: write-only access cannot silently
discard a linear value or prove a cross-field invariant by reading another field.

A prefix-writing contract can state that `[0..count)` changed while
`[count..len)` stayed unchanged. That preserves the caller's suffix facts; it
does not claim to construct previously nonexistent values.

Checked helpers must preserve non-observation transitively. A foreign provider
may physically be able to read the address, so its restriction needs admitted
provider evidence or enforced isolation. See the [structural access contract](../spec/terminal-psi/structural_access.md#write-only-authority).
Current source, artifact and native support are distinguished
[beside Terminal production](../../omega-rust/psi/compiler/terminal-production/README.md#structural-access-and-stores).

## Reborrow Authority and Restoration

A reborrow comes from one exact parent loan, not merely from borrowing the
reference carrier itself:

| Parent | Permitted child |
| --- | --- |
| Shared | Shared. |
| Mutable | Shared, mutable or write-only. |
| Write-only | Write-only. |

A shared child of mutable access freezes parent mutation; all shared descendants
in that cohort must end before mutation returns. An exclusive child suspends
the corresponding parent branch until it closes. Shared-to-shared release does
not restore exclusive authority that never existed.

“The child ended” is therefore not enough by itself. Restoration needs the exact
lineage, access, formation and closure evidence, including the complete shared
cohort. A parent going out of lexical scope does not magically return authority.
Root handoff, restored use, ownership transfer and cleanup are different events.
The [loan specification](../spec/terminal-psi/loans.md) gives their full rules.

## Transitions And Ownership

A transition transfers to a state within the same machine. The target's bindings
are explicit:

```omega
machine forward(value: Inventory) -> Inventory {
    transition {
        _ -> done(move value)
    }

    state done(value: Inventory) {
        value
    }
}
```

Copy values may copy into state arguments; owned values move. A reference may
cross only while its storage outlives the target path. Locals not transferred
need eligible cleanup or another authorized disposition; an outstanding linear
claim cannot disappear on the edge. See [state arrivals](../spec/language/state_contracts.md#bindings-and-arrivals).

## Borrow Facts

Separate loans may require a disjointness proof:

```omega
let a = &mut items[i];
let b = &mut items[j];
```

Bounds and `i != j` can establish that the elements differ. They cannot create
the owner's loan authority or widen its access. Arithmetic, domains and theorem
calls are ways to establish relations between already-existing subjects.

A loan captures its place when formed:

```omega
let view = &mut buffer[table[index]];
```

Later changing `table[index]` does not retarget `view`. Compatibility premises
must be valid for the captured value/place versions at formation; later expiry
does not move those frozen places.

Half-open windows illustrate a simple automatic proof:

```omega
let left = &mut items[start..mid];
let right = &mut items[mid..end];
```

Once their bounds are valid, the shared immutable boundary `mid` establishes
adjacency. An immutable integer copy keeps its captured value after its source
changes; two separate captures do not establish equality automatically.
Capturing a reference, unlike an integer, does not snapshot its referent.

Facts about storage survive only writes proved disjoint from their dependencies.
A write to `values[1]` may preserve a bound reading `values[0]`; an overlapping
or unknown write cannot. Equal expression text is not evidence of equal captured
places. [Live facts](../spec/language/state_contracts.md#mutation-and-subject-identity)
and [frozen loan places](../spec/terminal-psi/loans.md#frozen-places-and-proof-replay)
describe these separate obligations. General proof-derived compatibility remains
incomplete in the compiler; ordinary automatic checking is not a second semantic
system preceding proof.

## Owners And Borrowed Views

A borrowed window keeps its backing valid and rejects overlapping writes. For
copy-eligible `Entry`:

```omega
let view: &[Entry] = self.entries.as_slice();
self.entries[0].value = 7; // rejected: view is used below
let first: Entry = view[0];
```

A view restricted to `self.entries[1..]` need not conflict with a write to
element zero. Reallocation that invalidates a live vector view rejects too.
The rule concerns the actual borrowed window and backing, not a blanket ban on
every operation bearing the vector's name.

## Lifetime Parameters

An explicit lifetime names the input a returned view borrows:

```omega
machine header<'buffer>(
    buffer: &'buffer [u8], scratch: &mut [u8]
) -> &'buffer [u8] {
    return buffer;
}
```

The result retains `buffer`'s loan, not `scratch`'s. Common single-reference-input
and borrowed-receiver cases use elision, so most signatures need no tick. Prefer
descriptive lifetime names such as `'buffer` or `'arena`.

Borrow-carrying data makes zero-copy results ordinary values:

```omega
data ChatMessage<'buffer> {
    sender_id: i64;
    body: &'buffer [u8];
}

machine select<'left, 'right>(
    first: &'left [u8], second: &'right [u8]
) -> ChatMessage<'left> {
    return ChatMessage { sender_id: 0, body: first };
}
```

The message carries the original source loan. Moving it, putting it inside
another aggregate, or erasing a non-owning qualification cannot erase that loan.
Replacing one field ends that field's former loans after evaluating the new
value; unrelated siblings retain theirs.

Different result fields can name different inputs:

```omega
data Pair<'left, 'right> {
    left: &'left mut i32;
    right: &'right mut i32;
}

machine pair<'left, 'right>(
    left: &'left mut i32, right: &'right mut i32
) -> Pair<'left, 'right> {
    return Pair { left: left, right: right };
}
```

Using `result.right` need not keep an unrelated `result.left` loan live. Nested
records, active sum payloads and array positions retain the same structural
correspondence. A dynamic index conservatively includes all possible sources.

The [lifetime specification](../spec/language/lifetimes.md) owns binder syntax,
elision and carried-loan transport. The compiler's current multiple-input and
generic returned-view limitations are documented [beside checking](../../omega-rust/psi/pipeline/typed-trees-to-checked-trees/README.md#lifetime-source-correspondence).
Those limitations are not permission to forget unresolved borrows.

## Storage Carried By Placed Views

A borrowed `Placed<P, T>` retains its source borrow. An owned placement instead
carries an extent claim that needs an authorized return or release:

```omega
machine inspect(uart: &Placed<UartMmio, UartRegisters>) {
}

machine configure(uart: &mut Placed<UartMmio, UartRegisters>) {
}
```

Exclusive access to the view value does not upgrade a shared source borrow.
Ordinary stable mutation needs permission from its access plan and exclusivity
of both current view and retained source. A device is not excluded by an Omega
`&mut` borrow; external and atomic operations use their admitted contracts.

Projected accessors cannot outlive the view or escape the planned field.
Disjoint subviews need both logical non-overlap and compatible physical effects:
bitfields sharing a transfer word are not independently exclusive for destructive
reads or read-modify-write. Child views only narrow the parent's rights.
See [placed memory](chapter_20_memory_layout_abi.md#placed-and-externally-mutable-memory).

## Relationship To Drops

Ownership determines which dispositions are owed. [Drops And Cleanup](chapter_17_drops_and_cleanup.md)
explains eligible automatic cleanup, early disposal and edge order.
