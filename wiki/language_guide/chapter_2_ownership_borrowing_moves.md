# Chapter 2: Ownership, borrowing, and moves

Every value has one owner. The owner can move it somewhere else or lend it out
for a while. If you know Rust, most of this chapter will look familiar; the
main additions are write-only borrows and values that must be finished.

## Moving

```omega
machine repair_inventory(system: &mut InventorySystem, replacement: Inventory) {
    system.inventory = move replacement;
}
```

After `move replacement`, the name `replacement` can no longer be used. The
inventory that was in the field before is cleaned up as part of the
assignment.

Scalars and other `[copy]` types are copied instead of moved:

```omega
let depth: u32 = level.depth;
let next_depth: u32 = depth + 1;     // depth is still usable
```

## Three kinds of value

| Type is | You may |
| --- | --- |
| `[copy]` | Copy it and discard it freely. |
| Ordinary owned data | Move it. It is cleaned up when its owner goes away. |
| `[linear]` | Hand it on or finish it exactly once. The compiler rejects a path that forgets it. |

Use `[linear]` for things that need an explicit ending, such as a task handle
or a transaction. [Chapter 17](chapter_17_drops_and_cleanup.md) covers cleanup
and [Chapter 18](chapter_18_concurrency.md) shows task handles.

## Shared borrows

```omega
machine render_room(formatter: &RoomFormatter, room: &Room, out: &write [u8]) { ... }
```

`&T` lets the callee read. Any number of shared borrows can exist together,
and nothing can modify the value while one is in use.

## Mutable locals

Declare an owned local with `let mut` when its established storage will be
modified, whether directly, through a field or element, or by a helper
receiving `&mut` or `&write`. Taking a writable borrow delegates permission;
it cannot manufacture permission absent from the owned place.

This does not make everything reachable from a non-`mut` binding immutable.
A local holding an existing mutable reference can write through that reference
without being mutable itself: the reference's storage and its referent are
different places. Copying or moving an owned value follows its ordinary
multiplicity rules, independently of binding mutability.

The [precise rule](../spec/language/ownership.md#owned-local-mutability)
also preserves domain access restrictions and the separate rules for atomics.

## Mutable borrows

```omega
machine heal(player: &mut Player, amount: i32)
requires player.health in 0..=100
requires amount in 0..=100
{
    player.health += amount;
}
```

`&mut T` is exclusive: while it is in use, no other route can read or write
that place. Exclusive access is not a proof that the arithmetic fits; the
`requires` clauses do that.

## Write-only borrows

```omega
machine fill(destination: &write [u8])
requires destination.len > 0
{
    destination[0] = 42;
}

fill(&write buffer[..]);
```

`&write T` lets the callee overwrite without reading. It can read the length
of a view, but it cannot load contents or use `+=`. A mutable borrow can be
narrowed to write-only; the reverse is not available.

## Lending part of what you borrowed

```omega
machine clear_first(items: &mut [u8])
requires items.len > 0
{
    let first: &mut u8 = &mut items[0];
    first = 0;
    // The loan on items[0] ends after its last use.
    items[0] = 1;
}
```

A borrow ends at its last use, not at the end of the block. Until then the
parent waits.

Two borrows of the same collection need to be provably separate:

```omega
let left: &mut [i32] = &mut items[start..mid];
let right: &mut [i32] = &mut items[mid..end];

let a: &mut i32 = &mut items[i];
let b: &mut i32 = &mut items[j];      // needs i != j
```

The two halves are separate because they share the boundary `mid`. The two
elements are separate once `i != j` is known. A borrow is fixed when it is
formed: changing `i` afterwards does not move `a`.

## Taking a field out for a moment

```omega
machine rearrange_inventory(system: &mut InventorySystem) {
    let inventory: Inventory = system.inventory;
    // system.inventory is empty here and cannot be read or passed on.
    system.inventory = move inventory;
}
```

With exclusive access you can move a field out, work on it, and put a value
back. Every path that returns must restore the field first.
[Chapter 11](chapter_11_invariant_windows.md) covers longer repairs.

## Views keep their source alive

```omega
let view: &[Entry] = &entries[..];
entries[0].value = 7;        // Rejected: view is used below.
let first: Entry = view[0];
```

A view of `entries[1..]` would not conflict with that write. The rule is about
the storage actually borrowed, not about the name `entries`.

## Lifetimes

When a machine returns a reference and takes more than one, a lifetime says
which input the result borrows from:

```omega
machine header<'buffer>(
    buffer: &'buffer [u8], scratch: &mut [u8]
) -> &'buffer [u8] {
    buffer
}
```

With a single reference input, the lifetime can be left out. Data that holds
references carries lifetimes the same way, which is how a parser returns
zero-copy results:

```omega
data ChatMessage<'buffer> {
    sender_id: i64;
    body: &'buffer [u8];
}
```

A `ChatMessage` keeps its buffer borrowed for as long as the message exists,
wherever it is moved or stored. See
[lifetimes](../spec/language/lifetimes.md).

## Where next

[Chapter 3](chapter_3_machines.md) covers machines. The complete rules are in
[ownership](../spec/language/ownership.md); memory that a device can change
underneath you is in
[Chapter 20](chapter_20_memory_layout_abi.md#access-policies-and-externally-mutable-memory).
