# Qualified storage and linear custody

Run `omega run --both main.omg` from this directory. Both engines must exit 0.
The canonical Acknowledgement trait is installed by `build.omg`.

The program observes qualified returned owners, nests a completed qualified
child in a generic record, extracts and replaces a qualified field, and repairs
caller storage through a mutable loan. Each Receipt is acknowledged once.
Receipt is 40 bytes, forcing indirect native result storage. Nested calls,
state forwarding and both value and named-call transition exits preserve its
four payload words. An unconditional transition's later authored arm is
unreachable, not another returning claim source.

Owned local and mutable-parameter replacements check the detached old code
after the store, then return the new code and acknowledge its current claim.
A whole local transfer after repair must not recover the owner's birth claim.
A small register-carried Counter replacement forces its home through the store
alone, without a preceding field read or extraction.
Counter borrows distinguish direct construction, omitted-field construction
through a normalized alias, and a nested qualified child. A local annotation
cannot substitute for the checked initializer judgment or an established child.

The final Slot receives a child with an additional predicate-only membership.
Repair through the weaker Slot parameter owes its declared Issued membership,
not the detached old value's extra predicate. The replacement must remain
usable and the old owner must still be acknowledged exactly once.
Moving that repaired Slot whole also must not require the retired extra fact
or carry an anonymous constructor operand already consumed by an earlier call.
Two hooked elements leave a record's fixed array whole before the next
construction; the emptied array container must not inherit their nominal hooks.
