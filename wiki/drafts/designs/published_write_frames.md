# Published write frames

Ratified 2026-11-10. A caller keeps its facts about storage a call does not
write. A checked body's set of written places, its frame, was already derived
by the compiler, but the spec classed it as implementation evidence outside
the machine's contract. Editing a body to write one more field therefore
broke dependent proofs with no interface change to point at. The inferred
frame is now part of the published contract, the way an inferred reach row
is. Source does not change and no syntax is added:

```omega
public machine set_time(entry: &mut Entry, time: u64) {
    entry.at = time;
}
// published frame: entry.at

set_time(&mut e, 5);
use(e.owner);        // facts about e.owner survive

// Editing set_time to also write entry.owner changes its published
// contract. Dependents revalidate and the widening is reported.
```

The normative owner is
[mutation frames](../../spec/language/dependent_values.md#mutation-frames).
Track implementation under `PUBLISHED-WRITE-FRAMES` in [TASKS.md](../../../TASKS.md).
Delete this record when the implementation and acceptance are complete.

## Decisions

1. A checked body's frame is inferred and published, for public and private
   machines. It has no authored form.
2. The published frame is part of the contract and of interface identity.
   Widening it is an interface change even with an unchanged header;
   narrowing is compatible.
3. A static requirement or callable parameter defers to the selection: an
   application substitutes the selected machine's published frame.
4. A runtime callable value and a boundary or foreign operation use the
   ceiling, all existing storage the callee is admitted to write.
5. A generic machine's frame is a term over its static callbacks, published
   as a row of static binder contract dependencies.
6. A published frame is a set of static paths: a parameter followed by field
   selections. An indexed write lists the indexed place whole and a sum is
   written whole.
7. A path covers the place, the backing it owns by custody and, through a
   reference, its referent, identified at entry and followed through moves.
8. Every access path counts: mutable and write-only borrows, owned backing,
   reference fields, atomic or synchronized access through a shared
   reference, and capability-held authority. Unknown storage effects
   contribute the ceiling. Service reach neither bounds nor exempts a
   storage write.
9. Writes to fresh private storage are outside every frame. Moving
   pre-existing storage into a place does not bring it under that place's
   path.
10. A write that restores the earlier value is still a write.
11. For an opaque call, borrowing the subplace narrows the ceiling, and a
    requirement's `ensures place == old(place)` returns value facts to the
    caller. That postcondition permits a write in between and does not
    re-establish an authority-backed qualification the caller held beyond
    the field's declared type.

## What changes

| | Before | After |
| --- | --- | --- |
| A checked body's frame | Derived, outside the contract | Derived and published |
| Body edited to write another field | Dependent proofs break with no interface change | An interface change that dependents see |
| Static requirement or callback | Derived from the selected body, outside the contract | The selection's published frame |
| Runtime callable, boundary operation | Ceiling | Ceiling |

Source is unchanged. Publishing the frame makes every change to it an interface change, as an
inferred reach row already is. A machine cannot publish a frame wider than
its body's.

## Acceptance

- A caller in another package keeps a fact about `e.owner` across
  `set_time(&mut e, 5)` under `--both`, using the imported published frame
  and not the callee's body.
- Editing `set_time` to write `entry.owner` changes its interface identity
  with the header unchanged, and the dependent that relied on `e.owner`
  fails against the new published frame.
- `apply<set_time>` publishes `e.at` for that application; a second selection
  that writes the whole entry publishes the whole entry.
- A static requirement call takes the selected conformer's published frame.
- A call through a runtime callable field and a boundary operation uses the
  ceiling; a requirement stating `ensures entry.owner == old(entry.owner)`
  returns the value fact.
- An atomic store through a shared reference appears in the frame.
- `push` that allocates fresh backing, fills it and installs it publishes
  `vec`; a machine that writes only locals publishes an empty frame.
- A save and restore of one element lists that element's place.

## Left out

An authored clause. It would let a bodyless contract promise that a place is
never written, and let an author publish a frame wider than the body's. No
case in hand needs either.
