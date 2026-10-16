# Fused count residual

A three-difference residual moment. One fused rounding of
`1.0000000000000002220446049250313080847263336181640625` squared, plus the
negated addend `1.000000000000000444089209850062616169452667236328125`, stays
positive. `multiply_then_add` rounds twice and cancels to zero. The same fused
call through a parameter matches the local result stored in a field. Squaring
`0.5`, `1.5`, and `2.5` with the running fused total yields `8.75`. The exact
product `2 * 3 + 4` is `10` under both operations.

`differential_expression` estimates dispersion with
`sum((count / factor - mean)^2 - mean / factor)`. Those two terms can cancel
under separate roundings. This sample is the runnable observation of the fused
residual that estimator needs. The check-only probe `probes/fma_rounding`
does not execute it.

On agreement the program prints `PASS` and exits 0.

`invalid/` stores a fourth difference into `[f64; 3]` and is rejected with
`cannot prove index `3` is within length 3`.

x86-64 builds admit AVX+FMA3 in `build.omg`. That admission is the deployment
feature for one-rounding `VFMADD`; AArch64 uses `FMADD` without it. Two-rounding
`multiply_then_add` stays a separate operation.

```sh
omega run samples/cli/independent/fused_count_residual/main.omg < /dev/null
```
