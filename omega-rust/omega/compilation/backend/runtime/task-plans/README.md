# Task activation planning

Contract: [task activation and lifecycle](../../../../../../wiki/spec/build/task_runtime.md).
[lib.rs](src/lib.rs) defines activation plans, selected-runtime facts, exact
preservation obligations and stack composition; runtime invocation and
provider-instance lifecycle accounting are not here.
[stack_composition.rs](src/stack_composition.rs) composes same-stack calls
for any provider.

Omega builds activation plans after checking and reports them in
`05_task_activations.json`; target calling and layout, stack and runtime
selection must not become Psi source fields. The current layout of local
frames and parked values does not yet collect worst-case stack use (WCSU)
from final physical frames. Final demand, the activation StackPlan, the real
reservation and executable fixed-stack park/resume still need exact
producer-to-provider connections.

The stack composer checks that same-stack calls form a closed acyclic graph
and takes the largest aligned live chain. Sequential sibling calls share
capacity. An opaque same-stack leaf needs an admitted contribution binding the
exact strong provider-plan commitment, requirement, receipt, size and
alignment, and contribution commitment; callers cannot build one from byte
totals. Transfers to a provider stack or new activation add no child edge
here. A live call the producer cannot resolve to a checked callee or admitted
contribution (a requirement slot, machine parameter, dynamic descriptor or
unchecked supply mode) is never dropped from the bound: it joins the frame's
unresolved-call list, the composed demand is published as partial, and the
list travels in the sealed projection so `establish_stack_lease` refuses a
partial bound instead of granting authority for the covered calls alone.

Static runtime binding and instance invocation are different evidence. The
specialization's SHA-256 commitment is authoritative; older compact values
are report fingerprints. Each demanded CPU or thread axis has exact evidence.
An invocation receipt still needs to tie together the exact selected
provider, activation plan, runtime instance and preservation evidence;
`runtime_invocation` holds only diagnostic labels. This crate does not run
tasks, allocate stacks, hold live claims, or implement park, resume,
cancellation and settlement; the selected runtime does.

Each canonical suspension point also keeps its exact live values: one
`LiveCarryDemand` row per live place, naming the place, its checked type, the
storage class whose lifetime and pinning the park relies on, every attached
linear-claim identity, and its four-axis demand. A claim row keeps
established ownership; alone it neither establishes a loan nor proves the
backing safe to keep. A loan also needs checked storage lifetime, pinning,
aliasing and address-stability obligations. `parked_frontier` gives a
provider that exact list while the activation is suspended. Plan validation
requires the point's suspension permission and preservation bits to equal the
combination of its rows, so a plan cannot publish live values that disagree
with its declared demand, and a live place whose policy forbids suspension is
refused, never permitted or dropped.

Unresolved call sites stay in the sealed stack demand until provider planning
supplies exact target bindings and recomposes WCSU. The
selected runtime reads the validated plan and invocation receipt, owns
capacity and backing, and returns ordinary source-visible task outcomes. The
producer passes scalar values, structural homes and paths, and established
machine-local claim identities through the call's exact argument slots,
keeping address-backed scalar locals and mutable parameters in their homes
rather than their initial values. Kept reference and slice loans still need
their own checked backing and carry evidence. None of this supplies runtime
backing or permits parking; see the
[Terminal producer](../../../../../psi/compilation/terminal-production/README.md#structural-results-and-suspension).
