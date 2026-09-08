# Interrupt entry obligations

Interrupts use ordinary inbound boundary machines and
[installed external roots](external_roots.md), not a new machine species.
Target packages choose stack/nesting, acknowledgement, state, service,
suspension, and blocking contracts. Omega checks them; it does not choose
exception coverage, IST policy, interrupt masking, controller, timer handoff,
or fatal-fault policy. Table construction/publication follows
[hardware materialization](hardware_materialization.md).

## Mask and acknowledgement custody

`InterruptMaskGuard` and `InterruptAcknowledgement` are independent opaque
linear boundary data. Each has its own exact target-closed
`OpaqueRepresentation<T>` application selected before calling-policy evaluation.
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
conservative ceiling. The unfinished source selection/lineage integration is
tracked by `TOP-LEVEL-BOUNDARY-REQUIREMENTS` in [TASKS.md](../../../TASKS.md).

A deferred acknowledgement leases the installed root and controller configuration
until completion. Reconfiguration, shutdown, CPU removal, relevant power
transitions, and retirement drain outstanding acknowledgements first. Carry
policy separately controls transfer to a bottom half. There is no breakable pin
or asynchronous revocation of that debt.
