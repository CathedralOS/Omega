# Qualified storage and linear custody

Run `omega run --both main.omg` from this directory. Both engines must exit 0.
The canonical Acknowledgement trait is installed by `build.omg`.

The program observes qualified returned owners, nests a completed qualified
child in a generic record, extracts and replaces a qualified field, and repairs
caller storage through a mutable loan. Each Receipt is acknowledged once.
Counter borrows distinguish direct construction, omitted-field construction
through a normalized alias, and a nested qualified child. A local annotation
cannot substitute for the checked initializer judgment or an established child.

The final Slot receives a child with an additional predicate-only membership.
Repair through the weaker Slot parameter owes its declared Issued membership,
not the detached old value's extra predicate. The replacement must remain
usable and the old owner must still be acknowledged exactly once.
