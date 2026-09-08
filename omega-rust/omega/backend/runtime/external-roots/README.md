# External-root admission

Contracts: [installed roots](../../../../../wiki/spec/build/external_roots.md),
[entry stacks](../../../../../wiki/spec/resources/entry_stacks.md), and
[logical work](../../../../../wiki/spec/resources/logical_work.md).
Start at [lib.rs](src/lib.rs); [root_validation.rs](src/root_validation.rs)
owns admission and [provider_execution.rs](src/provider_execution.rs) owns the
validated execution binding.

[stack_demand.rs](src/stack_demand.rs) composes artifact/root demand.
[epoch_stack_demand.rs](src/epoch_stack_demand.rs) joins the complete entry
realization, body-domain closure, and installed-code evidence. Keep exact
retained inputs behind compact report fingerprints. Resource rows and provider
execution consume the same bound result; a scalar byte total is not a second
admission route.

The target arrival model lives in
[calling-conventions/stack_realizations.rs](../../../representations/calling-conventions/src/stack_realizations.rs).
Its x86-64 rule derives arrival from installed vector, gate/TSS and privilege
facts, not caller-authored word counts. The binder requires exact equality with
the validated context roster and the selected boundary commitment. Unknown,
omitted, padded, conflicting, or unresolved stack selections reject.

Generated adapter evidence remains origin-specific. The receiver-free x86
ProgramStorage wrapper's template/call replay accounts for its live frame
through enter/body/exit and compares the exact installed interval. That proves
stack geometry, not firmware invocation or a physical stack switch. Broader
adapter coverage must derive its own emitted epochs; it cannot copy this
wrapper's fixed count or generated-origin label.

[fixed_fuel.rs](src/fixed_fuel.rs) retains schedule identity and distinguishes
Terminal-derived entry/segment evidence from opaque provider claims. Segment
custody never becomes whole-entry authority by sharing a numeric bound.
Entry/stack, logical-work, and machine-state support must be evaluated separately;
the existence of one composer does not establish all entry origins or a general
WCET model.
