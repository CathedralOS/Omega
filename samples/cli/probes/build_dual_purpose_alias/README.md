# One alias in a Build and its application

The first Build prepares the dependency as `kit`. Its successor imports
`{kit}/generated.omg`, checks that `value()` returns 7, and forwards the
prepared binding to the application under the same alias. The application
imports that file independently and checks the same value at runtime.

The dependency seals its source template as the required output
`generated.omg`, copies the authored seed and receiver entry, and returns that
directory. The receiver declares the finished package. Removing either the
seed or the generated file breaks the consuming import.

Run `omega run --engine psi build.omg` or `omega run --engine native build.omg`.
The [negative cases](../build_dual_purpose_alias_controls/) check that a Product
registration cannot supply the executing Build and that the dependency's
declared name cannot stand in for the registered alias.
