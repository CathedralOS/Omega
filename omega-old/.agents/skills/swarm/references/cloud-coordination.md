# Coordinate cloud assignments before scaling

Follow [advance](../../advance/SKILL.md) preflight and
[claim registration](../../../../tools/docs/claims.md), retaining assignment IDs,
runtime handles and publication evidence in the wave's recovery journal. The
parent uses its actually exposed cloud create/read/follow-up tools; native
workers share one executor and do not represent cloud provisioning capacity.

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
Do not assume an event API, keep-alive control or lifecycle operation exists.
A worker continues to implementation only after its preflight is validated and
it has acknowledged the exact assignment; an accepted message is not enough. A
recovery operation with an unknown result, or a build slot still held on the
executor, blocks continuation and final acceptance.

An authorized advance continues automatically through required tests, commit/push,
verified ancestry and cleanup. Bounded report turns provide observability; they
do not introduce routine parent sign-off or a separate reviewer. Independent
review required by the user or assignment remains mandatory.

A session finishing is not a landed advance. Final acceptance needs the
program's observed behavior before and after, native and Psi results matching
the authored exit, the scoped tests and refused programs, the merged PR's
source, proof that the published commit is on `main`, reconciled
claims/tickets, and cleanup of the resources the assignment owns. Source evidence stays bound to the submitted SHA,
not relabeled as a run of the squash commit. Historical evidence remains separate;
reuse needs unchanged reviewed inputs/environment and explicit justification.

Start with a trial of only two sessions: two real programs that currently fail,
with different outcomes and hosts that can run them. Shared implementation paths are allowed. Declare a dependency
only for an actual producer prerequisite that another session is building;
shared-file closure alone does not require one. Independent assignments use an
empty dependency list. A sample and its compiler repairs stay one assignment
across stages. Required digest and golden updates remain part of that
assignment's review. Admit five, then
ten only after every outcome in the prior stage satisfies that same contract
without user rescue. Keep independent executors' build slots independent and
publication evidence reconciled. Simulate fault cases in tests, never by disrupting
another worker or live publication. Follow [cloud launch](cloud-launch.md) for preflight and approval.
