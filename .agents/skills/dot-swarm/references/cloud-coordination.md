# Coordinate cloud assignments before scaling

Use the executable [cloud adapter contract](../../../../tools/swarm/cloud.md)
for the staged 2 → 5 → 10 rollout. It extends the existing recovery journal,
preserving claims-v1 tickets and serialized landing publication. The parent
uses its actually exposed cloud create/read/follow-up tools; native workers share
one executor and do not represent cloud provisioning capacity.

Before dispatch, record actual producer/consumer and digest/golden dependencies.
Shared paths alone neither create a dependency nor block work. Use isolated
worktrees and reconcile at rebase. An actual producer dependency is recorded
with the exact result the consumer needs; claims never grant or refuse edits.
A dependency wait is distinct from terminal failure.

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
do not introduce routine parent sign-off or a separate reviewer. Independent
review required by the user or assignment remains mandatory.

Runtime completion is not a landed advance. Final acceptance binds before/after
program observations, authored native/interpreter result, scoped tests/negatives,
merged PR source identity, published-commit ancestry, reconciled claims/tickets
and owned-resource cleanup. Source evidence stays bound to the submitted SHA,
not relabeled as a run of the squash commit. Historical evidence remains separate;
reuse needs unchanged reviewed inputs/environment and explicit justification.

Admit only a bounded two-session canary first: two currently red real programs
with distinct outcomes and eligible hosts. Shared implementation paths are allowed. Declare a dependency
only for an actual producer prerequisite that another session is building;
shared-file closure alone does not require one. Independent assignments use an
empty dependency list. A sample and its compiler repairs stay one assignment
across stages. Required digest and golden updates remain part of that
assignment's review. Admit five, then
ten only after every outcome in the prior stage satisfies that same contract
without user rescue. Keep independent executors' build slots independent and
shared publication serialized. Simulate fault cases in tests, never by disrupting
another worker or live publication. Follow [cloud launch](cloud-launch.md) and
[recovery](recovery.md) for preflight, approval, backups and safe cleanup.
