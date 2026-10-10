# Binary32 batch of a quadratic

Ascending coefficients [1,2,3] represent 1+2x+3x^2.
Queries [-1,0,0.5,2] produce [2,1,2.75,17] and slopes [-4,2,5,14].
The caller explicitly allocates output arrays. A length-mismatch try call
preserves the short output sentinel 99 and all previously computed derivatives.
A zero-query batch succeeds, and empty coefficients take precedence over output mismatch.
Success prints `nf17 batch: quadratic values and slopes` and exits 0; failure exits 72.

See [the shared instructions](../README.md) for preparation and exact check, native,
and Terminal commands. Numerical fixtures and tolerances were frozen before execution.
