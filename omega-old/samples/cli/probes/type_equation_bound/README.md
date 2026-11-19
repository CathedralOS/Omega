# type_equation_bound

A where-clause type equation binds a generic type parameter to declared
structure: `machine capacity<Length, const Capacity: u64> where
Length == u64::AtMost<Capacity>` — the caller supplies the concrete domain
instantiation `capacity<u64 in AtMost<257> >()`; no runtime value supplies layout.

## Learnings

- `x as T in Wrapping` qualifies the RESULT type; the cast itself stays
  exact — a u64→i32 narrowing still needs a named conversion machine or a
  representability proof.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
