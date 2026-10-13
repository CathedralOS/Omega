# erased_field_init_order

A record literal listed out of field order still supplies an `[erased]`
member from its literal expression — `proof` sits between `after` and
`before` in the source while occupying no runtime position. The erased
member's own drop hook never becomes the record's obligation.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
