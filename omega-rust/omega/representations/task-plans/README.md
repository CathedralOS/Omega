# Task activation planning

Contract: [task activation and lifecycle](../../../../wiki/spec/build/task_runtime.md).
[lib.rs](src/lib.rs) owns activation plans, selected-runtime facts, exact
preservation/invocation joins, and provider-instance lifecycle accounting.
[stack_composition.rs](src/stack_composition.rs) owns provider-independent same-stack composition.

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
The ledger is downstream of this selection, not a generalized runtime-behavior
or continuation-capacity admission layer. It rejects premature close/reclaim
and cross-instance settlement, and requires fresh storage lease eras on reuse.
Its start boundary is the contract's ownership transaction: moved-argument
custody and the supplied nonmoving `StackLease` enter `accept_invocation`
together, every rejection returns them whole, the ledger retains the lease
while the claim lives, and settlement releases the spent authority. The
moved arguments are no token stub: `MovedTaskArguments` is marshalled under
the plan's exact `TaskArgumentLayout` — each source parameter's bytes packed
at its canonical field offset with zeroed padding — the provider holds that
image as activation custody while the claim lives, the claim fingerprint
commits to it byte-for-byte, and a rejected start returns the same image
unchanged. A
cancellation request is a recorded provider-side transition on the exact
live claim — never a disposal — and settlement reports a `Cancelled`
outcome only when that request was recorded and then observed at a
canonical safe point of the plan, so the cooperative outcome cannot be
fabricated, a never-suspending activation can never settle cancelled, and
an inline completion can never settle cancelled.

Live activations carry an execution state: a claim can `park` only at a
canonical suspension crossing of its plan, `resume` continues that same
invocation under unchanged bindings, and a parked claim cannot settle —
resumption must precede the terminal outcome. `observe_cancellation`
records where a recorded request was observed: at the parked crossing for
a suspended activation, or at any canonical crossing a running activation
traverses.

[provider_admission.rs](src/provider_admission.rs) is the provider-side
gate consuming those carriers: one admitted runtime instance owns its
ledger plus its fixed stack provisioning, mints fresh lease eras, and
commits the slot spend and the claim inside one admission, so capacity is
real authority rather than an `available >= 1` observation. Provider-
provisioned rejections conserve the moved arguments (the minted lease was
never caller custody); caller-supplied storage rejections return it whole
through `TaskStartRejection`. Settlement returns provisioned backing to
the free set while the spent era stays burned.

These carriers still do not establish a source `Task<T>`, and no selected
runtime executes the transitions the ledger models: the marshalled argument
image is provider-domain custody a real runtime would write into the
activation's argument area, but real park/resume of a native stack and
observation at a checked-source safe point remain separate consumers. The
bounded
scalar suspension carrier likewise does not license receiver/structural/claim
frontiers without their exact joins; see the
[Terminal producer](../../../psi/compiler/terminal-production/README.md#structural-results-and-suspension).
