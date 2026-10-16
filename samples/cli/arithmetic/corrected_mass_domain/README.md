# Corrected mass domain

A corrected gravimetric reading is an `f64` predicate domain. `corrected`
returns `gross - tare` in `NonnegativeMass` when both readings are finite and
`gross >= tare`. `6.0` establishes `AdmissibleTotal` (`value >= 0.01`). The
program then observes the mass through a field, passes it to `fraction` with
that total, and returns it again through `carried`.

On agreement it prints `MASS 2` and exits 0. The mass is 2 and the fraction
is 2/6.

`invalid/` assigns `0.0 - 1.0` to `f64 in NonnegativeMass` and must be
rejected because the annotation does not establish the predicate.

```sh
omega run samples/cli/arithmetic/corrected_mass_domain/main.omg < /dev/null
```
