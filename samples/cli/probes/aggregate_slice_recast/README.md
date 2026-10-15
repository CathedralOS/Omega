# aggregate_slice_recast

First sample re-viewing a typed fixed array as an unsized
slice of a differently named aggregate: `&source as
&[Wide]` weakens `i16 [0..=100]` → `u16 [0..=200]` on the
read side; `&mut source as &mut [Exact]` uses the same
bit-pattern set both ways; element stride proves through
trailing bool padding.

## Learnings

- Recast-carried views index like arrays
  (`read[0].unsigned_value`) — only whole-collection `&xs[..]` views
  hit the package-blocked `Slice::index`/`range`
  operators.
- A state holding a `&mut` recast view may not also read
  the original owner — reads go through the live view
  until its last use.

Compile-checked only: no standard-library dependency and no
`Main::exit` — run a check target, not native.
