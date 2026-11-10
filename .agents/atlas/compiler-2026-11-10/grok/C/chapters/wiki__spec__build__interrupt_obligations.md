# Interrupt entry obligations

Source: `wiki/spec/build/interrupt_obligations.md` lines 1–71 (read).

## Rules

rule id | the exact sentence, quoted | the heading it is under | what must be checked, in plain words | when it must hold | lane
--- | --- | --- | --- | --- | ---
INTERRUPT_OBLIGATIONS1 | "Interrupts use ordinary inbound boundary machines and [installed external roots](external_roots.md), not a new machine species." | Interrupt entry obligations | Interrupt handling must use ordinary inbound boundary machines with installed external roots, not a separate machine species | not stated | 10
INTERRUPT_OBLIGATIONS2 | "Target packages choose stack/nesting, acknowledgement, state, service, suspension, and blocking contracts." | Interrupt entry obligations | Stack/nesting, acknowledgement, state, service, suspension, and blocking contracts must come from target packages, not be invented by the compiler | not stated | 9
INTERRUPT_OBLIGATIONS3 | "Omega checks them;" | Interrupt entry obligations | The compiler must check the target-chosen stack/nesting, acknowledgement, state, service, suspension, and blocking contracts | not stated | SHARED
INTERRUPT_OBLIGATIONS4 | "it does not choose exception coverage, IST policy, interrupt masking, controller, timer handoff, or fatal-fault policy." | Interrupt entry obligations | The compiler must not choose exception coverage, IST policy, interrupt masking, controller, timer handoff, or fatal-fault policy | not stated | 9
INTERRUPT_OBLIGATIONS5 | "Table construction/publication follows [hardware materialization](hardware_materialization.md)." | Interrupt entry obligations | Interrupt table construction and publication must follow hardware materialization rules | not stated | 9
INTERRUPT_OBLIGATIONS6 | "An installed fatal body checked to have no normal completion does not resume the interrupted activation." | Interrupt entry obligations | A fatal installed body with no normal completion must be checked so it does not resume the interrupted activation | at entry | 10
INTERRUPT_OBLIGATIONS7 | "Its state plan may therefore make no interrupted continuation preservation commitment: interrupted, saved and restored sets are empty, while its permitted-use ceiling still covers the complete handler closure." | Interrupt entry obligations | For such a fatal body, the state plan may omit interrupted continuation preservation (empty interrupted, saved, and restored sets) while permitted-use ceiling still covers the full handler closure | not stated | 10
INTERRUPT_OBLIGATIONS8 | "Generated arrival code retains its independent validation obligations;" | Interrupt entry obligations | Generated arrival code must keep its own validation obligations separate from the handler | not stated | 3
INTERRUPT_OBLIGATIONS9 | "the handler's footprint does not establish the adapter's effects." | Interrupt entry obligations | Handler footprint must not be taken as establishing the adapter's effects | not stated | 3
INTERRUPT_OBLIGATIONS10 | "The admitted fatal protocol must stop if the body unexpectedly returns and must never resume an opaque hardware frame." | Interrupt entry obligations | The admitted fatal protocol must halt on unexpected return from the body and must never resume an opaque hardware frame | at normal return | 10
INTERRUPT_OBLIGATIONS11 | "This disposition belongs to the exact admitted body and installation; a calling policy or frame shape alone cannot establish it." | Interrupt entry obligations | Fatal disposition must be established only by the exact admitted body and installation, not by calling policy or frame shape alone | not stated | 3
INTERRUPT_OBLIGATIONS12 | "Returning interrupt entries retain their full save, restore and acknowledgement obligations." | Interrupt entry obligations | Returning interrupt entries must retain complete save, restore, and acknowledgement obligations | not stated | 10
INTERRUPT_OBLIGATIONS13 | "`InterruptMaskGuard` and `InterruptAcknowledgement` are independent opaque linear boundary data." | Mask and acknowledgement custody | `InterruptMaskGuard` and `InterruptAcknowledgement` must be treated as independent opaque linear boundary data | not stated | 2
INTERRUPT_OBLIGATIONS14 | "Each has its own exact target-closed `OpaqueRepresentation<T, Representation>` application selected before calling-policy evaluation." | Mask and acknowledgement custody | Each must use its own exact target-closed `OpaqueRepresentation` application chosen before calling-policy evaluation | not stated | 9
INTERRUPT_OBLIGATIONS15 | "Representation selection changes neither linearity nor the named discharge," | Mask and acknowledgement custody | Representation choice must not alter linearity or the named discharge obligation | not stated | 2
INTERRUPT_OBLIGATIONS16 | "and package code cannot inspect or reconstruct either private representation." | Mask and acknowledgement custody | Package code must not inspect or reconstruct the private representation of either type | at the operation | 2
INTERRUPT_OBLIGATIONS17 | "Saved interrupt mask | Routed `Active`; consuming `restore` restores the exact prior state." | Mask and acknowledgement custody | Saved interrupt mask must be routed `Active` and consuming `restore` must restore the exact prior mask state | at the operation | 10
INTERRUPT_OBLIGATIONS18 | "Interrupt acknowledgement | Routed `Pending`; consuming `complete` settles the exact policy debt." | Mask and acknowledgement custody | Interrupt acknowledgement must be routed `Pending` and consuming `complete` must settle the exact policy debt | at the operation | 10
INTERRUPT_OBLIGATIONS19 | "Provider occurrence evidence binds the exact root, installed code, provider execution, invocation, control/guard/prior/masked state or acknowledgement policy/token lineage." | Mask and acknowledgement custody | Provider occurrence evidence must bind the listed exact root, code, execution, invocation, states, and policy or token lineage | not stated | 8
INTERRUPT_OBLIGATIONS20 | "Equal compact identities cannot substitute another root's occurrence." | Mask and acknowledgement custody | Equal compact identities must not substitute for another root's occurrence | not stated | 8
INTERRUPT_OBLIGATIONS21 | "Replayed invocation/acknowledgement evidence rejects." | Mask and acknowledgement custody | Replayed invocation or acknowledgement evidence must be rejected | not stated | 8
INTERRUPT_OBLIGATIONS22 | "Nested mask guards restore only the newest saved state." | Mask and acknowledgement custody | Nested mask guards must restore only the newest saved mask state | at the operation | 10
INTERRUPT_OBLIGATIONS23 | "Deriver-owned exit requires the entry mask state and the exact completed acknowledgement." | Mask and acknowledgement custody | Deriver-owned exit must require entry mask state and the exact completed acknowledgement | at normal return | 10
INTERRUPT_OBLIGATIONS24 | "Forgotten settlement and double completion violate linearity;" | Mask and acknowledgement custody | Forgetting settlement or double completion must be treated as linearity violations | at the operation | 2
INTERRUPT_OBLIGATIONS25 | "active entries pin root retirement." | Mask and acknowledgement custody | Active entries must pin root retirement | not stated | 10
INTERRUPT_OBLIGATIONS26 | "`Pending` authorizes one stable core-owned semantic acknowledgement-entry requirement." | Mask and acknowledgement custody | `Pending` must authorize exactly one stable core-owned semantic acknowledgement-entry requirement | not stated | 8
INTERRUPT_OBLIGATIONS27 | "Target roots inherit it and refine the physical plan/ABI and bounded installation reach." | Mask and acknowledgement custody | Target roots must inherit that requirement and refine physical plan, ABI, and bounded installation reach | not stated | 9
INTERRUPT_OBLIGATIONS28 | "Installation introduces the exact qualified parameter;" | Mask and acknowledgement custody | Installation must introduce the exact qualified parameter | not stated | 9
INTERRUPT_OBLIGATIONS29 | "ordinary calls require that qualification as a precondition." | Mask and acknowledgement custody | Ordinary calls must require that qualification as a precondition | at a call | 3
INTERRUPT_OBLIGATIONS30 | "The joined evidence binds requirement, semantic and ABI positions, domain, carry, plan, and admitted occurrence; no authored entry marker or parameter selector substitutes for it." | Mask and acknowledgement custody | Joined evidence must bind requirement, semantic and ABI positions, domain, carry, plan, and admitted occurrence; entry markers or parameter selectors must not substitute | not stated | 8
INTERRUPT_OBLIGATIONS31 | "Mask restoration reaches `MachineControl`." | Completion reach and lifetime | Mask restoration must reach `MachineControl` | at the operation | 10
INTERRUPT_OBLIGATIONS32 | "Acknowledgement has a provider-neutral bounded abstract row beneath `MachineControl + PortIo`: PIC may select `PortIo`, LAPIC/x2APIC `MachineControl`, and another admitted mechanism both or empty when its actual contract warrants it." | Completion reach and lifetime | Acknowledgement completion reach must stay within the stated provider-neutral bounded abstract row and provider-specific selections | not stated | 10
INTERRUPT_OBLIGATIONS33 | "This upper bound grants no authority or Boolean choice algebra." | Completion reach and lifetime | The completion-reach upper bound must not be read as granting extra authority or a Boolean choice algebra | not stated | 10
INTERRUPT_OBLIGATIONS34 | "Entry and completion remain different operation rows." | Completion reach and lifetime | Entry and completion must remain distinct operation rows | not stated | 10
INTERRUPT_OBLIGATIONS35 | "Equal rows do not identify a provider or authorize cross-provider settlement." | Completion reach and lifetime | Equal operation rows must not identify a provider or authorize cross-provider settlement | not stated | 9
INTERRUPT_OBLIGATIONS36 | "The installed route binds exact entry, completion operation, provider execution, policy, and token lineage, and replays the provider-selected completion reach when settling debt." | Completion reach and lifetime | The installed route must bind exact entry, completion operation, provider execution, policy, and token lineage, and replay provider-selected completion reach when settling debt | at the operation | 9
INTERRUPT_OBLIGATIONS37 | "The installation closure substitutes every bounded row and rejects unresolved rows;" | Completion reach and lifetime | Installation closure must substitute every bounded row and reject unresolved rows | not stated | 9
INTERRUPT_OBLIGATIONS38 | "ordinary exported callable contracts bind providers first or publish a fixed conservative ceiling." | Completion reach and lifetime | Ordinary exported callable contracts must bind providers first or publish a fixed conservative ceiling | not stated | 9
INTERRUPT_OBLIGATIONS39 | "A deferred acknowledgement leases the installed root and controller configuration until completion." | Completion reach and lifetime | A deferred acknowledgement must lease the installed root and controller configuration until completion | across a pause | 10
INTERRUPT_OBLIGATIONS40 | "Reconfiguration, shutdown, CPU removal, relevant power transitions, and retirement drain outstanding acknowledgements first." | Completion reach and lifetime | Reconfiguration, shutdown, CPU removal, relevant power transitions, and retirement must drain outstanding acknowledgements before proceeding | not stated | 10
INTERRUPT_OBLIGATIONS41 | "Carry policy separately controls transfer to a bottom half." | Completion reach and lifetime | Transfer to a bottom half must be controlled only by carry policy, separately from acknowledgement rules | not stated | 3
INTERRUPT_OBLIGATIONS42 | "There is no breakable pin or asynchronous revocation of that debt." | Completion reach and lifetime | Deferred acknowledgement debt must not have a breakable pin or asynchronous revocation | across a pause | 2

## Ambiguous sentences

Sentence | Reading A | Reading B
--- | --- | ---
"Its state plan may therefore make no interrupted continuation preservation commitment: interrupted, saved and restored sets are empty, while its permitted-use ceiling still covers the complete handler closure." | For a fatal body with no normal completion, the state plan is allowed to omit preservation commitments and leave those sets empty. | The state plan must omit those commitments whenever the body has no normal completion; emptiness is mandatory, not optional. |
"Representation selection changes neither linearity nor the named discharge," | Any target-closed `OpaqueRepresentation` choice keeps the same linear discharge obligations (`restore` / `complete`). | Linearity and discharge are fixed by the types; representation selection is constrained so it cannot alter them, but how selection is validated is not stated here. |
"active entries pin root retirement." | While an entry is active, root retirement must be blocked or deferred. | "Pin" names a specific retirement mechanism defined elsewhere; this sentence only states a relationship without saying how retirement is enforced. |
"Target roots inherit it and refine the physical plan/ABI and bounded installation reach." | Target roots must carry the core requirement and may refine plan, ABI, and reach within bounds. | Inheritance is mandatory; "refine" might mean optional customization or mandatory target-specific detail—the chapter does not say which. |
"PIC may select `PortIo`, LAPIC/x2APIC `MachineControl`, and another admitted mechanism both or empty when its actual contract warrants it." | Each provider may select among the listed completion-reach options when its contract warrants. | "Another admitted mechanism both or empty" could mean selecting two mechanisms, selecting none, or a single mechanism with alternate readings; the phrase does not fix the grammar of "both or empty." |
"ordinary exported callable contracts bind providers first or publish a fixed conservative ceiling." | Each ordinary exported callable contract must either bind providers explicitly or publish a fixed conservative ceiling. | Contracts might bind some providers and use a ceiling for others; whether the choice is per-contract or must be uniform is not stated. |
"Carry policy separately controls transfer to a bottom half." | Bottom-half transfer is governed only by carry policy, not by mask or acknowledgement rules in this chapter. | Carry policy is an additional gate; whether mask/acknowledgement must also be satisfied before transfer is not stated. |

## Terms used without definition in this chapter

- ordinary inbound boundary machines
- installed external roots
- machine species
- target packages
- stack/nesting (contracts)
- acknowledgement (contracts)
- state (contracts)
- service (contracts)
- suspension (contracts)
- blocking (contracts)
- exception coverage
- IST policy
- interrupt masking
- controller (interrupt)
- timer handoff
- fatal-fault policy
- table construction/publication
- hardware materialization
- installed fatal body
- normal completion
- interrupted activation
- state plan
- interrupted continuation preservation commitment
- interrupted set / saved set / restored set
- permitted-use ceiling
- handler closure
- generated arrival code
- validation obligations
- handler's footprint
- adapter's effects
- admitted fatal protocol
- opaque hardware frame
- admitted body
- installation
- calling policy
- frame shape
- returning interrupt entries
- `InterruptMaskGuard`
- `InterruptAcknowledgement`
- opaque linear boundary data
- target-closed
- `OpaqueRepresentation<T, Representation>`
- calling-policy evaluation
- linearity
- named discharge
- private representation
- package code
- Routed `Active` / Routed `Pending`
- `restore` / `complete`
- Saved interrupt mask
- policy debt
- Provider occurrence evidence
- provider execution
- invocation
- control/guard/prior/masked state
- acknowledgement policy/token lineage
- compact identities
- replayed evidence
- nested mask guards
- Deriver-owned exit
- entry mask state
- root retirement
- `Pending` (routing state)
- core-owned semantic acknowledgement-entry requirement
- target roots
- physical plan/ABI
- bounded installation reach
- exact qualified parameter
- joined evidence
- semantic and ABI positions
- domain
- carry
- plan
- admitted occurrence
- authored entry marker
- parameter selector
- `MachineControl`
- `PortIo`
- provider-neutral bounded abstract row
- PIC / LAPIC / x2APIC
- admitted mechanism
- Boolean choice algebra
- operation rows
- cross-provider settlement
- installed route
- completion operation
- token lineage
- installation closure
- bounded row
- unresolved rows
- ordinary exported callable contracts
- conservative ceiling
- deferred acknowledgement
- installed root
- controller configuration
- reconfiguration / shutdown / CPU removal / power transitions / retirement
- outstanding acknowledgements
- carry policy
- bottom half
- breakable pin
- asynchronous revocation
- debt (acknowledgement)

CHAPTER COMPLETE
