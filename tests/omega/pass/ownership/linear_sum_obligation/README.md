# linear_sum_obligation

`case Live(receipt: Receipt)` — an affine sum carries a path-sensitive
linear obligation only in its Live arm: `Empty` drops normally; `Live`
transfers the contained receipt through the whole sum and settles
through the boundary-trait requirement `ReceiptStateSettle::settle(state)`.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
