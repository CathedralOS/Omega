# One dependency alias in Build and Product

Both the Build and the product import `{kit}/generated.omg` and call its
public `value()` machine. The Build registers the dependency with
`Build::build_dependency` before waiting for it. A separate `Build::dependency`
registration gives the product access under the same alias.

The dependency reads a source template, publishes those bytes as
`generated.omg`, and writes the same bytes to a promised file. The generated
`value()` imports the dependency’s authored `main.omg` and calls `seed()`,
which returns 7.
