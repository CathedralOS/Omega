# Invalid-source control: readonly matrix mutation

Intended rejection: `source[0] = 4.0` attempts a write through
`source: &[f64; 9]`. Source ownership/type checking must reject readonly mutation
before execution. The complete project has std dependency and hosted entry
bindings so it cannot substitute a missing build/entry for this rejection.
It has no success or execution oracle; a nonsymmetric or indefinite matrix is
valid runtime data and is a separate case in the correlated sample.
