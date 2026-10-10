# fma rounding

Compile-scope probe. Imports `omega::language::core::float_operations`.

- `F32::fused_multiply_add`/`F64::fused_multiply_add` round once —
  positive residual where `multiply_then_add` (two roundings)
  cancels to exactly 0.
- x86-64 requires `builder.x86_deployment_features =
  X86DeploymentFeatures::AvxFma3` in build.omg — fused FMA is a
  deployment-feature admission, not ambient.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/fma_rounding/main.omg
mbx run -p omega -- update --project samples/cli/probes/fma_rounding
```
