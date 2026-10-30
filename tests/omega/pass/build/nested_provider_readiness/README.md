# Nested Build readiness

The root pauses for a provider whose own Build pauses for a second provider.
Each activation must retain its receiver, code, request ledger and log while
the child executes. The root Product runs with native and Terminal exit 0.

The `build_charge --readiness-jobs` check observes three Build executions,
and challenges the same source with a dependency
cycle. This complements the private-provider and wrong-owner rejection fixtures.
It establishes application scheduling, not removal of recursive source checking
or the compiler's large stack reservation.
