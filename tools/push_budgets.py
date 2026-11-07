"""Patience budgets shared by the landing and claims writers. Collisions are
lost compare-and-swap races against concurrent writers; each costs one
snapshot + push round. Push failures are pushes rejected while the remote
refs provably stayed put (network blips, ref-lock contention, object-DB lag),
so re-pushing the same commit is safe."""

COLLISION_LIMIT = 40
COLLISION_BACKOFF_BASE = 0.1
COLLISION_BACKOFF_CAP = 1.5
PUSH_FAILURE_LIMIT = 6
PUSH_FAILURE_BACKOFF_BASE = 0.4
PUSH_FAILURE_BACKOFF_CAP = 3.0
PROBE_ATTEMPTS = 3
PROBE_BACKOFF_SECONDS = 0.3
