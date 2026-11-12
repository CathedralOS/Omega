Run with `omega run --both tests/omega/pass/ownership/runtime_nested_borrowed_consumption_exit/main.omg`.

An inner identity call consumes the borrowed sum, then the outer call preserves
or retags its linear payload and restores the original caller's field. The two
calls use a projected-field destination and a whole borrowed referent. Both
normal branches owe the same storage repair; the disjoint marker remains 19.
