# Chapter 2: Ownership, Borrowing, And Moves

Values have an owner. Ownership determines who is responsible for moving,
mutating, and cleaning up a value.

This chapter is the place for the rules other chapters rely on.

> **Core multiplicity settled 2026-07-18.** Usage is an explicit type
> property with three cases: unrestricted, affine, and linear. Facts and
> permissions share control-flow/place infrastructure but not one algebra. See
> [core_multiplicity_and_linearity.md](../design_briefs/core_multiplicity_and_linearity.md).

## Usage Multiplicity

- **Unrestricted** values may be copied and discarded. `[copy]` establishes
  this property.
- **Affine** values may be moved at most once and may be discarded. This is
  the default for owned data.
- **Linear** values must be transferred or explicitly consumed exactly once.
  `[linear]` establishes this property.

Multiplicity is a type property, not a trait and not a qualifier repeated at
every binding. It composes structurally through records, sums, and generic
containers. `[copy]` and `[linear]` are mutually exclusive.

Establishing a new linear value creates exactly one obligation. Moves, calls,
returns, receives, and storage operations transfer that obligation; terminal
consumers discharge it. Implicit zero-filling does none of these. Linearity is
about use, not bit patterns: an explicitly constructed all-zero linear value
is owed if that bit pattern is valid for its type.

## Owned Values

An owned value may be moved into another location.

```omega
machine InventorySystem::repair(&mut self) {
    let replacement: Inventory;

    self.inventory = move replacement;
}
```

After a move, the old binding is no longer usable.

## Copy Values

Some values are copied instead of moved.

```omega
let depth: u32 = self.level_depth;
let next_depth: u32 = depth + 1;
```

Copy is a type property. Machine integers, booleans, and small proof values are
natural copy candidates. A copied value is unrestricted. Data with unique
cleanup responsibility is not.

## Linear Values

Linear values represent protocols that must reach an explicit conclusion: task
lifecycle claims, transactions, acknowledgements, DMA submissions, and similar
resources. A move transfers the live obligation. Ordinary scope exit with an
unconsumed linear value is a compile error; automatic drop cannot silently
discharge it.

Conditional ownership uses an ordinary sum such as `Idle | Running(Task<T>)`.
The obligation belongs only to the live payload. Zeroed storage is not a
universal consumed linear value.

Flow analysis therefore carries two different kinds of context. Propositions
can weaken or duplicate where logic permits. Permissions track establishment,
multiplicity, access (`owned`, `shared`, `exclusive`), and provenance with
their own path-join rules. One CFG walk may carry both, but a fact catalog must
never silently forget a resource obligation.

## Shared Borrows

Shared borrows allow read-only access.

```omega
machine RoomFormatter::render(
    &self,
    room: &Room,
    out: &mut String
) {
}
```

Many shared borrows may coexist if no mutable borrow conflicts with them.

## Mutable Borrows

A mutable borrow is unique for the borrowed place.

```omega
machine Player::heal(
    &mut self,
    amount: i32
) {
    self.health += amount;
}
```

While `self.health` is mutably borrowed, code cannot also read or mutate the
same place through another active reference.

## Transitions And Ownership

A transition is a jump. Arguments passed to the target state must be valid on
the target edge.

```omega
machine InventorySystem::repair(&mut self) {
    transition {
        _ -> build_inventory()
    }

    state build_inventory(&mut self) {
        let replacement: Inventory;

        transition self.inventory_valid {
            true -> done()
            false -> copy_default_items(move replacement)
        }
    }

    state copy_default_items(replacement: Inventory) {
        self.inventory = move replacement;
    }

    state done(&mut self) {
    }
}
```

Working rules:

- Copy values may be copied into transition arguments.
- Owned values may be moved into transition arguments.
- References may cross a transition only when the referenced storage outlives
  the target path.
- Owned locals not moved into the target are cleaned up on the transition edge.

## Borrow Facts

Borrowing contributes facts to the proof system.

```omega
let a = &mut items[i];
let b = &mut items[j];
```

The checker must know that `i` and `j` refer to disjoint places. That fact may
come from arithmetic, from a domain, or from a helper machine that establishes
`i != j`.

## Owners And Borrowed Views

A borrowed view (a slice over an array, a `&str` over a `String`, or a slice over
a `Vec`) keeps the owner pinned for the view's lifetime. While such a view is
active the checker rejects any write to the owner that overlaps the borrowed
window:

```omega
let view: &[Entry] = self.entries.as_slice();
self.entries[0].value = 7; // rejected: view is still active
let first: Entry = view[0];
```

Disjoint windows are allowed when disjointness is provable from compile-time
bounds. A subslice `view[1..]` does not conflict with a write to
`self.entries[0]`, because index `0` is provably outside the `1..` window.

The same rule applies to `Vec`: a `Vec` mutation or reallocation
(`push`, `pop`, or anything that may move the backing storage) must reject while
a slice view derived from that `Vec` is still active, because the view may be
invalidated by the reallocation. This is the borrow-conflict rule for `Vec`; its
canary is parked under `canaries/pending/borrow/vec_view_invalidated_by_push`
until the `Vec` runtime/lowering is ready to exercise it end to end.

## Lifetime Parameters

Omega adopts the Rust lifetime model wholesale (frozen decision 15): a call's
output may borrow an input, and LIFETIME PARAMETERS — declared in the same
`<>` list as type and `const` parameters, tick spelling — say which:

```omega
machine header<'buf>(buffer: &'buf [u8], scratch: &mut [u8]) -> &'buf string {
    // the returned view aliases `buffer`; the checker extends buffer's loan
    // for as long as the result lives. `scratch` is unentangled.
}
```

ELISION keeps the common cases annotation-free, exactly as in Rust: a single
ref input means the output borrows it, and a `&self` method's output borrows
self. Most signatures therefore never write a tick:

```omega
machine decode_body(buffer: &[u8]) -> &string { ... }       // borrows buffer
machine Level::find_room(&self, id: CellId) -> &Room { ... } // borrows self
```

Borrow-carrying data is in-model from day one — a type holding views is
generic over the lifetime of what it views, which is what makes zero-copy
decoding spellable:

```omega
data ChatMessage<'buf> {
    sender_id: i64;
    body: &'buf string;     // view into the receive buffer; zero bytes copied
}
```

House style: descriptive lifetime names (`'buf`, `'arena`, `'msg`), never
`'a`. The tick was kept deliberately after surveying the alternatives
(argument-naming clauses, keyword region/origin parameters, Mojo-style
bracket origins): it is lexically self-identifying at use sites, collides
with none of Omega's bracket meanings (slices, properties, invariant
parameters), and elision makes it rare. Implementation is staged; until the
checker learns the linkage it conservatively treats a returned view as
aliasing every ref argument.

## Relationship To Drops

Ownership decides who must clean up a value. The cleanup machinery itself is
covered later in [Drops And Cleanup](chapter_17_drops_and_cleanup.md).

The compiler's current move/drop summaries predate first-class multiplicity
and do not yet represent establishment or linear consumption. The migration is
tracked in
[semantic_taxonomy_representation.md](../architecture/semantic_taxonomy_representation.md).
