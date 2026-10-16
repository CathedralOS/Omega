# Bounded exact Bareiss step

One fraction-free elimination update under Exact `i64`. A checked store
writes the factors into a mutable fixed array and cites each element. The
caller reads those elements back and forms `(6 * 4 - 2 * 3) / 3`. A second
store cites one element through a parameter index and the caller doubles it.

`metabolic_flux_analysis` performs the same update on Wrapping carriers and
checks a zero remainder by hand. This sample asks citation of a mutable
array's `ensures` to discharge Exact multiplication and division.

Expected exit: 0. Expected output contains: `exact=6`.

The invalid control divides by a provably zero pivot and must be rejected
at checking.
