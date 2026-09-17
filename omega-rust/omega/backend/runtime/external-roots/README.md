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

[interrupt_table.rs](src/interrupt_table.rs) accounts one descriptor table's
complete declared member set — fatal exception entries on their own dedicated
critical stack classes plus acknowledged interrupts such as the timer — over
the installed-root ledger. Admission replays the ledger's retained root records
and retains each member's linear handle so entries cannot retire while the
table holds them. Publication is a separate edge: the consumer-established
table value must name exactly the admitted set, and the checked-instruction
provider answers with a receipt for the exact issued carrier. Refusal returns
the established value for retry; publication and receipt identities cannot be
replayed. The checked edge itself is the sole receipt-minting boundary: on
x86-64 it replays the exercised consumer authority's bound identity, its
installed-realization scope, and both required scope legs (processor table
control plus table publication), then replays the declared 10-byte
pseudo-descriptor operand — accounted read site in the table's address space,
non-aliasing, and `{base, limit}` naming the exact established destination —
before minting. The answer records the operand read, the contract's fixed
`r10` scratch clobber, and the descriptor-table register state a published
answer installs; a declined attempt installs nothing.

A published table is also the dispatch boundary for hardware arrivals:
`begin_published_interrupt_entry` resolves the reported vector through the
member set sealed at publication and enters the armed member's retained root
through the ledger's ordinary admission edge, so a provider receipt mints
entry obligations only for an arrival whose gate actually reached hardware.
Arrivals before publication, on unarmed vectors, or against a foreign
installed realization reject with the receipt returned.

Byte materialization keeps the same writer/consumer split: the ledger derives
the checked post-handoff writer program whose fragments resolve each member's
sealed entry target into the produced gate's offset fields, while the
consumer-declared descriptor constants — selector, gate kind, privilege, IST
slot, and the reserved-zero bytes — are staged table content the writer
preserves. `validate_written_descriptor_table` is the consumer's semantic
edge: it proves the produced image came from exactly this table's derived
writer over this exact installed realization, replays every declared
descriptor's constant fields plus the zero fill across undeclared slots,
joins each member's declared IST slot through the installed TSS to its
declared critical stack class, and only then mints the established value
naming the exact written destination. Publication still consumes only that
established value plus separately supplied authority.
