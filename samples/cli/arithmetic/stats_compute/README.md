# stats_compute

A statistics compute kernel that exercises f64 arithmetic end-to-end.

Computes mean, variance, and a weighted sum over a fixed 8-element dataset
(values: 2, 4, 4, 4, 5, 5, 7, 9).

- **Mean**: 5.0 (sum=40, count=8)
- **Variance**: 4.0 (sum of squared deviations = 32, divided by 8)
- **Weighted sum**: 82.5 (weights: 1.0, 1.0, 1.5, 1.5, 2.0, 2.0, 2.5, 3.0)

All checks pass via guard ladders and the program exits 70.

## What it tests

- f64 field arithmetic (`field + field`, `field * scalar`, `field - field`)
- f64 accumulation across multiple dispatched machine calls
- f64 comparison guards (`> 4.9`, `< 5.1`, etc.)
- Structs with `[copy]` property containing f64 fields
- Multiple sequential machine calls each updating shared float state
