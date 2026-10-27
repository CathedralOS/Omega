The producer's checked `Reading::read` establishes `result == 42`; the consumer
traps if either native or Terminal execution receives another result. The
provider uses ordinary named implementation selection under `Independent`.

`CHECKED-PACKAGE-REUSE` uses this fixture for a bounded workflow acceptance:
separate cold and warm CLI invocations must both execute with exit 0, while the
warm invocation restores the complete native component (`component.cache.hit`)
before Product checking/Terminal production. Build admission, source custody,
grants and charges still run for the new invocation. A source-drift control
changes only the producer's returned literal to 43 and must reject its unchanged
postcondition, including after a valid product was cached. Corrupt records are
misses and must return to fresh compilation. Existing foreign-binding and
callback fixtures cover those complete record inputs through the same route.

This does not establish persisted checked-package graphs or the parent task's
fixture and ten-line `std` timing acceptance.
