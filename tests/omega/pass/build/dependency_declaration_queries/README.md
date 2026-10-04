# Dependency declaration queries

The consumer imports only `value.omg`. The dependency Build dynamically
describes `launch` from its Product declarations; this does not require checking
the unrelated false contract in the dependency's package root. A full review
of that dependency must still reject the false contract.

Run `omega --check tests/omega/pass/build/dependency_declaration_queries/main.omg`
to exercise dependency Build execution through package orchestration. The
corpus check also pins the consumer's imported-module behavior, but does not
execute dependency Builds and therefore does not replace this CLI check.

The entry query's target names a declaration contract, not native execution.
It is intentional that the returned description is unused: the checked query
must still execute and reject a nonexistent declaration.
