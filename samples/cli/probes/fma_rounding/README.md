# Fused and separately rounded multiply-add

The program evaluates `(1 + epsilon) * (1 + epsilon) - (1 + 2 * epsilon)`
in both binary32 and binary64. Here `epsilon` is the distance from one to
the next larger representable value. Fused multiply-add must produce exactly
`epsilon * epsilon`. Rounding the product first must produce zero. The program
exits 0 when both precisions satisfy those checks and a second fused call
preserves the first result.

```sh
omega run --engine native samples/cli/probes/fma_rounding/build.omg < /dev/null
omega run --engine psi samples/cli/probes/fma_rounding/build.omg < /dev/null
```

The x86-64 build explicitly selects `X86DeploymentFeatures::AvxFma3`.
