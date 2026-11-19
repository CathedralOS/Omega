# Calibration monomial power

A polynomial calibration basis needs `setting^0`, `setting^1`, and
`setting^2`. `step<0>` is 1 and `step<1>` is the setting. For a positive
degree, `power<Degree>` multiplies the setting by the call result
`step<(Degree - 1)>`, kept in a local. The setting 10 therefore prints
`1 10 100` and exits 0.

The constant term is that local `step<0>` result. The linear term reads the
setting field of a `Basis<1>` record. The quadratic term passes that same
setting as a parameter.

```sh
omega run samples/cli/arithmetic/calibration_monomial_power/main.omg
```
