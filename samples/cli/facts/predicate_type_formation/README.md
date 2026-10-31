# Predicate type formation

Case membership `Boolean` plus the formed default domain recovers `T == bool`,
so the Boolean payload is a `bool`.

Parent: `DOMAIN-DEFAULT-PREDICATE-UNIFICATION` in
[TASKS.md](../../../../../TASKS.md). This witness is formation of that
predicate. `Value::Boolean { value: true }` and `Value::Boolean { value: false }`
are `Value<bool>`. The match reads only `Boolean` because `T == bool` refutes
`Integer`.

The positive binds the four hosted targets. `omega run --both` requires
native and Terminal Psi agreement on exit 0. The twin changes only the `true`
formation to `Value::Integer { value: 1 }`.

Construction, calls, matching, coverage, writes, and erasure of the same
predicate remain on the parent bullet.
