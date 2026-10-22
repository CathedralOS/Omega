# float_computation_route_exit

The reduced float-computation matrix routes without importing
`omega::language::core::float_operations`: a copy (`main.d = main.e`), an
integer comparison (`main.n > 5`), float arithmetic
(`main.d = main.d + main.e`), a float ordered comparison
(`main.d > 211.5`) and float equality (`main.d == main.e`) all execute.

The matrix computes `d == 4.0` with `flag == false`; the authored native exit is
1 and a wrong observation exits 2. The fixture was promoted from
`tests/omega/pending/arithmetic/float_computation_no_route` once every position
routed; the reject control is
[`fail/float/float_computation_route_mixed_arith_rejected`](../../../fail/float/float_computation_route_mixed_arith_rejected/main.omg),
where the arithmetic position mixes `f64` and `f32`.

## Lowering evidence

The retained Terminal Psi keeps each position distinct rather than folding it:
the copy is a field read and store, the integer comparison is
`IntegerLessThan` with the normalized operand order, `d + e` is a `BoundaryCall`
to the declared `Float::add(f64, f64)` boundary operator, and `d > 211.5` and
`d == e` are `IeeeFloatCompare` with `Greater` and `Equal`. No
`omega::language::core::float_operations` import appears in the source.
