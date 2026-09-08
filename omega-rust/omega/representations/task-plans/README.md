# Task activation planning

Contract: [task activation and lifecycle](../../../../wiki/spec/build/task_runtime.md).
[lib.rs](src/lib.rs) owns activation plans, selected-runtime facts, exact
preservation/invocation joins, and provider-instance lifecycle accounting.
[wcsu.rs](src/wcsu.rs) owns provider-independent same-stack composition.

Activation planning is Omega-owned post-check state. The compiler reports
`05_task_activations.json`; target calling/layout, stack, and runtime selection
must not become Psi checked-tree fields. The current local-frame/park-frontier
layout bridge is not complete final-physical-frame WCSU collection. Final demand,
the activation StackPlan, actual reservation, and executable fixed-stack
park/resume still need their exact producer/provider joins.

The stack composer validates a closed acyclic same-stack call graph and takes
the maximum aligned live chain. Sequential siblings share capacity. Opaque
same-stack leaves require admitted contributions binding the exact strong
provider-plan commitment, requirement, receipt, bytes/alignment, and contribution
commitment; callers cannot construct them from byte totals. Provider-stack and
new-activation transfers have no child edge in that stack domain.

Static runtime binding and instance invocation are different evidence. The
specialization SHA-256 commitment is authoritative; historical compact values
are report fingerprints. Each demanded CPU/thread axis has exact evidence.
The ledger is downstream of this selection, not a generalized runtime-behavior
or continuation-capacity admission layer. It rejects premature close/reclaim
and cross-instance settlement, and requires fresh storage lease eras on reuse.

These carriers alone do not establish a source `Task<T>`, execute a provider,
or conserve actual moved arguments through start rejection. Routed source
establishment, stack provisioning, transactional start ownership, cancellation
conformance, and real runtime execution remain separate consumers. The bounded
scalar suspension carrier likewise does not license receiver/structural/claim
frontiers without their exact joins; see the
[Terminal producer](../../../psi/compiler/terminal-production/README.md#structural-results-and-suspension).
