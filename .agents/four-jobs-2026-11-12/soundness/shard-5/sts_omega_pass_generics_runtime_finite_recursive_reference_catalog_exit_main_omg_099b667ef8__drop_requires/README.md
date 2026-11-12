# Finite applied fields and recursive reference identities

Run `omega run --both tests/omega/pass/generics/runtime_finite_recursive_reference_catalog_exit/main.omg`.

Nested `Wrapper<Wrapper<&Cell>>` storage closes both by-value field views and
reads the original cell. Recursive referents, including a growing static-argument
family behind a zero-length array, retain their identities without expanding
the field catalog indefinitely. The authored result is exit 0.
