# Feature demand follows target realization

Linux selects an integer-only body and must publish without AVX/FMA admission.
Windows retains a checked FMA body in the same family; its operation must not
become a Linux feature requirement. The exit oracle is zero on Linux. Other
hosts can establish publication but not claim native execution of this image.

On producer `51bf3420d6d6804845eb553a7ad4b595b7db7e8f`, this unchanged
program rejected during Linux selection with `requires explicit AVX+FMA3
admission for slot F64::fused_multiply_add`. The source-side association pass
scanned both bodies before Terminal target selection.

The companion control
[`x86_fma_missing_deployment_features`](../../../fail/float/x86_fma_missing_deployment_features/main.omg)
executes F32 and F64 FMA on its selected Linux path and must reject missing
deployment admission. The admitted
[`fused_multiply_add_single_rounding_exit`](../fused_multiply_add_single_rounding_exit/main.omg)
checks fused residuals, separate multiply/add cancellation, and chained results
through native and Terminal execution on its host.

This is a bounded provider-boundary regression for
**PROVIDER-SELECTION-AFTER-TERMINAL** / **PIPELINE-ARCHITECTURE-ENFORCEMENT**,
not complete task or cross-host qualification. The governing contracts are
[target realization](../../../../../wiki/spec/language/numeric_values.md#target-realization-and-control-state)
and [multi-target compilation](../../../../../wiki/spec/build/configuration.md#multi-target-compilation).
