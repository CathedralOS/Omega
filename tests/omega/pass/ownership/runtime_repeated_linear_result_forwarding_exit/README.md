# Repeated linear result forwarding

Prepare the package with `omega update --project .` and, if requested,
`omega update --project . --resume`, then run `omega run --both main.omg`.
Both engines must exit 0. The standalone Unit entrypoint imports only core
`drop`; wrong payload correspondence crashes `Trap`.

`combine` calls the same `issue` machine twice, establishing the right receipt
before the left, then returns them in the declared `Pair` positions. `capture`
stores that result in a local before returning it. The caller invokes `capture`
twice and keeps both pairs live, producing four distinct linear obligations
with codes 11, 12, 21 and 22. Result lineage must retain the exact call
invocations and field paths through the wrapper, rather than flattening both
children or both results onto one static producer identity.

The caller moves the first pair's right receipt through `identity`, whose
returned claim continues its transferred input. It then consumes first-left,
second-right, forwarded first-right and second-left in that interleaved order.
Every consumer reads its exact code and calls core `drop` once. The complete
runtime assertion checks all four positions, while ordinary linear checking
and Terminal custody enforce exactly-once consumption and sibling preservation.
No provider or external sink substitutes for that ownership accounting.
