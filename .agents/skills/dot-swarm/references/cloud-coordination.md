# Coordinate cloud assignments before scaling

Use the executable [cloud adapter contract](../../../../tools/swarm/cloud.md)
for the staged 2 → 5 → 10 rollout. It extends the existing recovery journal,
without changing claims-v1 ownership or landing publication fences. The parent
uses its actually exposed cloud create/read/follow-up tools; native workers share
one executor and do not represent cloud provisioning capacity.

Before dispatch, record complete producer/consumer, digest/golden and shared-file
dependencies. Keep one writer per file; designate an integration owner or create
serial partitioned legs. A dependency wait is distinct from terminal failure.
Exact claims and bilateral handoff receipts are required; “finish” cannot imply
overlap permission. A known peer fence remains blocked until coordinated.

When the parent has no child filesystem access and read exposes only recorded
messages/results, use bounded preflight/report turns and controller-issued
continuations. Local checkpoint files alone are not remotely observable. Reports
carry assignment/thread/environment, sequence, phase, actual command/session,
start/progress timestamps, reason, next gate and confirmed artifact identity.
No event API, keep-alive control or lifecycle operation is inferred. Validated
preflight and exact child acknowledgment gate implementation continuation;
message admission alone is insufficient. Unknown recovery operations and held
executor build slots block continuation and final acceptance.

An authorized advance continues automatically through required tests, commit/push,
verified ancestry and cleanup. Bounded report turns provide observability; they
do not introduce routine parent sign-off or a separate reviewer.

Runtime completion is not a landed advance. Final acceptance binds before/after
program observations, authored native/interpreter result, scoped tests/negatives,
exact landing receipt, fetched remote ancestry, reconciled claims/tickets and
owned-resource cleanup to the final SHA. Historical evidence remains separate;
reuse needs unchanged reviewed inputs/environment and explicit justification.

Admit only a bounded two-session canary first: disjoint real outcomes and an
explicit shared-digest dependency with one integration owner. Admit five, then
ten only after every outcome in the prior stage satisfies that same contract
without user rescue. Keep independent executors' build slots independent and
shared publication serialized. Simulate fault cases in tests, never by disrupting
another worker or live publication. Follow [cloud launch](cloud-launch.md) and
[recovery](recovery.md) for preflight, approval, backups and safe cleanup.
