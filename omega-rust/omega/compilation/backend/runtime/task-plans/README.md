# Task activation planning

Contract: [task activation and lifecycle](../../../../../../wiki/spec/build/task_runtime.md).
[lib.rs](src/lib.rs) owns activation plans, selected-runtime facts, exact
preservation obligations and stack composition. Runtime invocation and
provider-instance lifecycle accounting are not implemented here.
[stack_composition](src/task_plans/stack_composition/mod.rs) owns provider-independent same-stack composition.

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
new-activation transfers have no child edge in that stack domain. A live call
the producer cannot resolve to a checked callee or an admitted contribution —
a requirement slot, a machine parameter, a dynamic descriptor, or a
non-checked supply mode — is never dropped from the bound: it enters the
frame's unresolved-call roster, the composed demand publishes as partial, and
the roster rides the sealed projection so `establish_stack_lease` refuses a
partial bound rather than minting authority for the covered subgraph alone.

Static runtime binding and instance invocation are different evidence. The
specialization SHA-256 commitment is authoritative; historical compact values
are report fingerprints. Each demanded CPU/thread axis has exact evidence.
An invocation receipt still needs to join the exact selected provider,
activation plan, runtime instance and preservation evidence; `runtime_invocation`
currently contains only diagnostic axis labels. This crate does not execute a
task, allocate a stack, retain a live
claim or implement park, resume, cancellation and settlement. Those are
ordinary selected-runtime responsibilities.

Each canonical crossing also retains its exact live frontier — one
`LiveCarryDemand` row per live place naming the place, its checked type,
the storage class whose lifetime and pinning the park relies on, the
complete linear-claim identities attached to it, and the four-axis demand
it contributes. Nonempty claim rows retain established ownership; they do
not by themselves establish a loan or prove its backing is safe to retain.
Loans also need the checked storage lifetime, pinning, aliasing and address
stability obligations. `parked_frontier` hands a provider that exact roster
while the activation is suspended. Plan
validation binds the crossing's suspension permission and preservation
bits exactly to the join of its frontier, so a candidate cannot publish
a frontier that disagrees with the demand it declares, and a live place
whose policy forbids suspension fails closed rather than licensing or
dropping the loan.

Unresolved call sites remain explicit in the sealed stack demand until provider
planning supplies exact target bindings and recomposes the WCSU projection.
The selected runtime consumes the validated plan and invocation receipt, owns
capacity and backing, and returns ordinary source-visible task outcomes. The
producer joins scalar values, structural homes and paths, and established
machine-local claim identities through the ordinary call's exact argument
lanes, retaining address-backed scalar locals and mutable formals in their
existing homes rather than their initializer values. Retained reference/slice
loans still need their own checked backing and carry evidence. None of those
joins supplies runtime backing or licenses parking; see the
[Terminal producer](../../../../../psi/compilation/driver/terminal-production/README.md#structural-results-and-suspension).
