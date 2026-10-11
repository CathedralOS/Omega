# build_opaque_receipts

Two `boundary data` carriers (`Token`, `Seal`) each with an authorized
`OpaqueRepresentation` view, exercised under a freestanding multi-target
`--check`. The satisfies declarations land in different module orders, so
per-target admission receipts derive in differing orders.

The question pinned: do opaque-property receipts stay well-formed per
target when the declaration order differs from the request's target
order, without ProgramEntry binds?
