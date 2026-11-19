# Float arithmetic inside a selected provider

`main` calls the `Reading` requirement. Its selected body adds one to an `f64`
argument; `main` traps unless the returned value is 42. This keeps the float
addition behind a provider call, so review must include selected provider bodies
when deriving numeric execution demand.

The package-review test reopens its canonical policy and requires the exact
`Float::add` application. Native and Psi execution must both exit 0. The Build
binds all four hosted targets.
