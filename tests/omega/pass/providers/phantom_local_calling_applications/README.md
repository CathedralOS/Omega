# Exact declaration ABI applications

The `NATIVE-ABI-APPLICATION-IDENTITY` repair retains a complete declaring scope
and ordered non-callback local arguments, independently of runtime geometry.

Three calls instantiate the same `Probe<u8>` method:

- `u32, first_handler` and `u64, first_handler` require different ABI keys.
- `u32, first_handler` and `u32, second_handler` share one ABI key.

All have identical physical storage. The runtime checks the distinct handler
results (11 versus 22), while the real package-review integration test
`review_retains_exact_same_geometry_phantom_local_applications` requires exactly
two complete applications and preserves both through canonical recovery.
The same test repeats authentic policy applications and checks complete-value
deduplication without discarding any distinct policy realization.

Native calling-application recovery controls separately change an operand,
its declared ordinal, and its declaring application while retaining an old
compact coordinate or commitment; all must refuse. This program does not
claim to exercise a foreign library or private callback-pointer installation.
