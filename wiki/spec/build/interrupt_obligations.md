# Interrupt entry obligations

Interrupts use ordinary inbound boundary machines and
[installed external roots](external_roots.md), not a new machine species.
Target packages choose stack/nesting, acknowledgement, state, service,
suspension, and blocking contracts. Omega checks them; it does not choose
exception coverage, IST policy, interrupt masking, controller, timer handoff,
or fatal-fault policy. Table construction/publication follows
[hardware materialization](hardware_materialization.md).

An installed fatal body checked to have no normal completion does not resume
the interrupted activation. Its state plan may therefore make no interrupted
continuation preservation commitment: interrupted, saved and restored sets are
empty, while its permitted-use ceiling still covers the complete handler
closure. Generated arrival code retains its independent validation obligations;
the handler's footprint does not establish the adapter's effects. The admitted
fatal protocol must stop if the body unexpectedly returns and must never resume
an opaque hardware frame. This
disposition belongs to the exact admitted body and installation; a calling
policy or frame shape alone cannot establish it. Returning interrupt entries
retain their full save, restore and acknowledgement obligations.

## Mask providers and initial authority

Named decision: `sealed-runtime-providers`, settled 2026-11-12. Interrupt-mask
save/restore providers are ordinary platform packages using contracted target
`asm` or a legitimate admitted native operation. Exact compiler primitives and
entry/exit bridges remain narrowly identified where required; an authored
`CompilerIntrinsic` locator does not supply their implementation or custody.

`InterruptMaskControl::save_and_mask` takes an explicit controller-authority
value or borrow with a protected qualification authorizing the particular mask
scope and execution context. Establish that permission before changing the
state. The returned `InterruptMaskGuard in Active` carries the obligation and
authority to restore the matching previous state; it does not supply the
missing permission for the initial mask. Restricting issuance of Active alone
does not protect a public zero-argument masking operation. Apply
[initial-call authority](../resources/authority.md#initial-call-authority)
at every lower-level callable route as well as the public wrapper.

The provider must actually perform the checked/admitted mask transition and
establish that the issued guard belongs to it. Merely constructing a guard or
asserting Active proves neither a hardware action nor its obligation. Save and
restore retain one compatible representation, provider provenance, scope and
CPU/activation-context relationship. Restoration consumes the matching guard
and restores the exact prior state, including initially masked and nested use;
it must not unconditionally enable interrupts or restore unrelated state after
migration. Carry and context-preservation rules remain independently checked.

Package/default selection grants no machine-control authority. A configuration
without the demanded realization rejects explicitly, never as a successful
no-op. Model checks can establish bookkeeping, but only execution under a
suitable privileged target or emulator demonstrates that the actual mask
operation is emitted and behaves as specified. The
[delivery controls](../../drafts/designs/runtime_provider_authority.md#acceptance)
keep those evidence claims separate.

## Mask and acknowledgement custody

`InterruptMaskGuard` and `InterruptAcknowledgement` are independent opaque
linear boundary data. Each has its own exact target-closed
`OpaqueRepresentation<T, Representation>` application selected before calling-policy evaluation.
Representation selection changes neither linearity nor the named discharge,
and package code cannot inspect or reconstruct either private representation.

| Obligation | Required qualification and terminal operation |
| --- | --- |
| Saved interrupt mask | Routed `Active`; consuming `restore` restores the exact prior state. |
| Interrupt acknowledgement | Routed `Pending`; consuming `complete` settles the exact policy debt. |

Provider occurrence evidence binds the exact root, installed code, provider
execution, invocation, control/guard/prior/masked state or acknowledgement
policy/token lineage. Equal compact identities cannot substitute another root's
occurrence. Replayed invocation/acknowledgement evidence rejects. Nested mask
guards restore only the newest saved state. Deriver-owned exit requires the
entry mask state and the exact completed acknowledgement. Forgotten settlement
and double completion violate linearity; active entries pin root retirement.

`Pending` authorizes one stable core-owned semantic acknowledgement-entry
requirement. Target roots inherit it and refine the physical plan/ABI and bounded
installation reach. Installation introduces the exact qualified parameter;
ordinary calls require that qualification as a precondition. The joined evidence
binds requirement, semantic and ABI positions, domain, carry, plan, and admitted
occurrence; no authored entry marker or parameter selector substitutes for it.

## Completion reach and lifetime

Mask restoration reaches `MachineControl`. Acknowledgement has a provider-neutral
bounded abstract row beneath `MachineControl + PortIo`: PIC may select `PortIo`,
LAPIC/x2APIC `MachineControl`, and another admitted mechanism both or empty when
its actual contract warrants it. This upper bound grants no authority or Boolean
choice algebra.

Entry and completion remain different operation rows. Equal rows do not identify
a provider or authorize cross-provider settlement. The installed route binds
exact entry, completion operation, provider execution, policy, and token lineage,
and replays the provider-selected completion reach when settling debt. The
installation closure substitutes every bounded row and rejects unresolved rows;
ordinary exported callable contracts bind providers first or publish a fixed
conservative ceiling.

A deferred acknowledgement leases the installed root and controller configuration
until completion. Reconfiguration, shutdown, CPU removal, relevant power
transitions, and retirement drain outstanding acknowledgements first. Carry
policy separately controls transfer to a bottom half. There is no breakable pin
or asynchronous revocation of that debt.
