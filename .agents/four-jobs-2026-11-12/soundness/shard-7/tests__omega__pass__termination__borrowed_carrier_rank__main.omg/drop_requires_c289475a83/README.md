# borrowed_carrier_rank

The nested carrier formal may itself be borrowed: `holder` is
`&Pair`, so the `bag` role arrives through one reference boundary
and the produced length still reads `holder.bag.items` under the
borrow.

## Gaps found while authoring (filed)

- Regressed surface (corpus `regressed-from` pins, NOT landed here):
  `in 0..=indirect.target.remaining` — a rank endpoint read through a
  stored `&`/`&mut` reference — is currently unprovable; the corpus
  fixtures `stored_reference_endpoint_arrivals`,
  `rank_endpoint_borrows_recovery` and
  `measure_borrowed_projection_range` fail `--check` identically.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
