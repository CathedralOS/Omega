# Invalid source: empty checked polynomial

The zero-length coefficient slice cannot satisfy the checked evaluation
precondition `coefficients.len > 0`. Intended result: check rejects that call.
Native execution and Terminal comparison are not valid acceptance routes for
this deliberately invalid program. Missing import, custody, or parse failures
do not constitute its intended rejection.

See [the shared instructions](../README.md) for preparation and exact check, native,
and Terminal commands. Numerical fixtures and tolerances were frozen before execution.
