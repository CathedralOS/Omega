# Chapter 2: Ownership, borrowing, and moves

Ownership determines who may move a value and who owes its final disposition.
Borrowing temporarily grants access without transferring the referent. The
[ownership specification](../spec/language/ownership.md) defines these rules;
this chapter shows their use.



## Owned values

An owned value can move into another location:

```omega
machine repair_inventory(system: &mut InventorySystem, replacement: Inventory) {
    system.inventory = move replacement;
}
```

After the move, the old binding is unusable. Replacement must also account for
the old inventory's disposition.

Moving one field leaves the remaining fields to be used or cleaned up. A type
with a selected cleanup machine must instead provide a consuming operation that
handles the complete value; only its own package and its cleanup machine may
take it apart ([Chapter 17](chapter_17_drops_and_cleanup.md)).

## Copy values

```omega
let depth: u32 = level.depth;
let next_depth: u32 = depth + 1;
```

Copying `depth` does not consume it. The addition still needs its Exact overflow
proof. Integers and Booleans are common unrestricted values; unique cleanup
responsibility cannot be copied.

## Values that must be finished

A task handle or transaction often needs an explicit conclusion. Marking its
type `[linear]` makes the compiler check that every normal path transfers or
consumes it exactly once. Put an optional task in a sum such as
`Idle | Running(Task<T>)`; only the active case carries a task to finish.

| Type property | What you may do |
| --- | --- |
| `[copy]` | Copy and discard the value. |
| Ordinary owned data | Move it; clean it up through its eligible cleanup operation. |
| `[linear]` | Transfer or consume its obligation exactly once. |

Constructing a linear value creates the obligation even if its bits are zero.
Use the type's authorized operation to finish it. [Chapter 17](chapter_17_drops_and_cleanup.md)
explains cleanup; [Chapter 18](chapter_18_concurrency.md) shows task handles.

## Shared borrows

Shared borrows grant observation under their access contract:

```omega
machine render_room(formatter: &RoomFormatter, room: &Room, out: &write [u8]) {
}
```

Several shared borrows may coexist. While they remain live, ordinary conflicting
mutation rejects. Each reference observes the original storage.

## Mutable borrows

A mutable borrow is exclusive for its borrowed place:

```omega
machine heal(player: &mut Player, amount: i32)
requires player.health in 0..=100
requires amount in 0..=100
{
    player.health += amount;
}
```

While the place is mutably borrowed, another active route cannot read or mutate
it incompatibly. Arithmetic and value invariants remain ordinary obligations;
exclusive access does not prove the new health fits.

### Temporarily moving out A field

An exclusive borrow may temporarily leave a field empty while you own its former
contents:

```omega
machine rearrange_inventory(system: &mut InventorySystem) {
    let inventory: Inventory = system.inventory;
    // inventory owns the value; system.inventory is temporarily absent.
    system.inventory = move inventory;
}
```

This opens an ownership invariant window. You may transform the detached value
and install a different valid replacement, provided every returning path repairs
the original field. While it is absent, reading it, borrowing it, or passing
`system` as a whole valid object rejects. Unaffected fields remain usable where
their borrows and invariants permit. A selected cleanup machine
prevents partial extraction.

Restore the field before returning or ending the borrow. An early failure does
not put the moved value back. [Invariant windows](chapter_11_invariant_windows.md)
show how to finish a multi-step repair.

## Write-only borrows

Write-only access exclusively lends an existing valid value without permitting
observation of its contents:

```omega
machine fill(destination: &write [u8])
requires
    destination.len > 0
{
    destination[0] = 42;
}

fill(&write buffer[..]); // caller must establish nonempty buffer
```

Use `&write` for an operation that overwrites without reading. The referent is
already a valid value, and must be valid when the loan ends. You can locate a
known field and read view metadata such as length, but cannot load its contents
or use a read-modify-write operation such as `+=`.

Mutable access can be narrowed explicitly to write-only access. The reverse
conversion is unavailable, even after a write. An explicit lifetime is written
`&'buffer write T`.

The [write-only access rules](../spec/terminal-psi/structural_access.md#write-only-authority)
cover sum payloads, displaced owned values, and foreign providers.

## Lend a borrow to a helper

A mutable borrow can temporarily lend shared, mutable, or write-only access.
Shared access can lend shared access; write-only access can lend write-only
access. The parent must wait while a child has conflicting access.

```omega
machine clear_first(items: &mut [u8])
requires items.len > 0
{
    let first: &mut u8 = &mut items[0];
    first = 0;
    // The child loan ends after its last use.
    items[0] = 1;
}
```

If several shared child loans exist, all conflicting uses must end before the
parent can mutate again. The compiler tracks this through helper calls too.

## Transitions and ownership

A transition arm invokes its target machine and forwards its result. Nested
targets obey the same argument and ownership rules:

```omega
machine forward(value: Inventory) -> Inventory {
    transition {
        _ -> done(move value)
    }

    machine done(value: Inventory) -> Inventory {
        value
    }
}
```

Copy values may copy into arguments; owned values move. A reference may
cross only while its storage outlives the target path. Locals not transferred
need eligible cleanup or another authorized disposition; an outstanding linear
claim cannot disappear on the edge. See [machine arrivals](../spec/language/state_contracts.md#bindings-and-arrivals).

Loans remain live through the final call, including any suspension;
final position does not authorize early cleanup or release of borrowed backing.

## Borrow facts

Separate loans may require a disjointness proof:

```omega
let a: &mut i32 = &mut items[i];
let b: &mut i32 = &mut items[j];
```

Bounds and `i != j` can establish that the elements differ. They cannot create
the owner's loan authority or widen its access. Arithmetic, domains and theorem
calls are ways to establish relations between already-existing subjects.

A loan captures its place when formed:

```omega
let view: &mut u8 = &mut buffer[table[index]];
```

Later changing `table[index]` does not retarget `view`. Compatibility premises
must be valid for the captured value/place versions at formation; later expiry
does not move those frozen places.

Half-open windows illustrate a simple automatic proof:

```omega
let left: &mut [i32] = &mut items[start..mid];
let right: &mut [i32] = &mut items[mid..end];
```

Once their bounds are valid, the shared immutable boundary `mid` establishes
adjacency. An immutable integer copy keeps its captured value after its source
changes; two separate captures do not establish equality automatically.
Capturing a reference, unlike an integer, does not snapshot its referent.

A write preserves facts about disjoint storage. If checking cannot establish
that two writes are disjoint, reestablish the facts before relying on them.
See [facts after mutation](chapter_7_types_constraints_invariants.md#fact-propagation).

## Owners and borrowed views

A borrowed window keeps its backing valid and rejects overlapping writes. For
copy-eligible `Entry`:

```omega
let view: &[Entry] = &entries[..];
entries[0].value = 7; // rejected: view is used below
let first: Entry = view[0];
```

A view restricted to `entries[1..]` need not conflict with a write to
element zero. Reallocation that invalidates a live vector view rejects too.
The rule concerns the actual borrowed window and backing, not a blanket ban on
every operation bearing the vector's name.

## Lifetime parameters

An explicit lifetime names the input a returned view borrows:

```omega
machine header<'buffer>(
    buffer: &'buffer [u8], scratch: &mut [u8]
) -> &'buffer [u8] {
    buffer
}
```

The result retains `buffer`'s loan, not `scratch`'s. A single unambiguous reference
input permits elision; multiple possible sources need an explicit relation. Prefer
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
    ChatMessage { sender_id: 0, body: first }
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
    Pair { left: left, right: right }
}
```

Using `result.right` need not keep an unrelated `result.left` loan live. Nested
records, active sum payloads and array positions retain the same structural
correspondence. A dynamic index conservatively includes all possible sources.

The [lifetime specification](../spec/language/lifetimes.md) covers explicit
parameters, elision, and returned views.

## Device and external-memory views

An access view retains the borrow or storage claim from which it was created.
Its access policy controls which operations are permitted. A mutable Omega
borrow cannot stop a device from changing memory, so device reads and writes
use the selected access operations. [Chapter 20](chapter_20_memory_layout_abi.md#access-policies-and-externally-mutable-memory)
shows those views.

## Relationship to drops

Ownership determines which dispositions are owed. [Drops And Cleanup](chapter_17_drops_and_cleanup.md)
explains eligible automatic cleanup, early disposal and edge order.
