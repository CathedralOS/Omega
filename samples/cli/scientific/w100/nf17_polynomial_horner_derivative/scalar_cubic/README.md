# Binary64 constant and asymmetric cubic

Ascending cubic coefficients [2,-3,0,1] represent x^3-3x+2.
At -2,-1,0,1,2 the values are 0,4,2,0,4 and slopes are 9,0,-3,0,9.
The degree-zero fixture is 7 with slope 0. Empty try input preserves 31,47.
Success prints `nf17 scalar: exact value and derivative` and exits 0; failure exits 71.

See [the shared instructions](../README.md) for preparation and exact check, native,
and Terminal commands. Numerical fixtures and tolerances were frozen before execution.
