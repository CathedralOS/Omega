# Machine-state realization evidence

Source: `wiki/spec/build/machine_state_evidence.md` lines 1–67 (read).

## Rules

| rule id | the exact sentence, quoted | the heading it is under | what must be checked, in plain words | when it must hold | lane |
| --- | --- | --- | --- | --- | --- |
| MACHINE_STATE_EVIDENCE1 | "The evaluated [call and state plans](calling_plans.md) are the published boundary promise." | Machine-state realization evidence | The published boundary promise must be the evaluated call and state plans, not machine-state evidence | not stated | 3 |
| MACHINE_STATE_EVIDENCE2 | "Actual register and machine-state use is evidence about one provider artifact, not an additional caller contract." | Machine-state realization evidence | Register and machine-state use must be treated as per-artifact evidence and must not impose extra caller obligations | not stated | 8 |
| MACHINE_STATE_EVIDENCE3 | "Instruction selection and allocation must honor the state ceiling." | Final-artifact obligation | Selected instructions and register allocation must stay within the permitted state ceiling | not stated | 10 |
| MACHINE_STATE_EVIDENCE4 | "Independent validation checks the complete final artifact after inlining, specialization, relaxation, thunks/veneers, generated stubs, and admitted indirect leaves:" | Final-artifact obligation | Validation must run on the full final artifact after those lowering and linking steps | not stated | 9 |
| MACHINE_STATE_EVIDENCE5 | "actual_transitive_footprint ⊆ permitted_transitive_use" | Final-artifact obligation | Actual transitive register and machine-state footprint must be a subset of permitted transitive use | not stated | 10 |
| MACHINE_STATE_EVIDENCE6 | "actual_clobbers ∩ unsaved_interrupted_state = ∅" | Final-artifact obligation | Actual clobbers must not intersect unsaved interrupted machine state | not stated | 10 |
| MACHINE_STATE_EVIDENCE7 | "Changing allocation within those bounds revalidates the implementation without changing the published promise." | Final-artifact obligation | Allocation changes inside the footprint and clobber bounds must trigger revalidation only, not a changed published promise | not stated | 9 |
| MACHINE_STATE_EVIDENCE8 | "Contextual specialization may produce a callee under a restricted state ceiling; it is not a new generic type application." | Final-artifact obligation | Contextually specialized callees under a restricted ceiling must not be modeled as new generic type applications | not stated | 10 |
| MACHINE_STATE_EVIDENCE9 | "Final-artifact validation is mandatory." | Certificate and replay | Final-artifact validation must always run | not stated | 8 |
| MACHINE_STATE_EVIDENCE10 | "Its internal validation records are not shipped with the artifact." | Certificate and replay | Internal validation records must not ship in the delivered artifact | not stated | 9 |
| MACHINE_STATE_EVIDENCE11 | "One self-describing certificate binds exact final bytes, placements, and the complete executable-region inventory." | Certificate and replay | The certificate must bind final bytes, placements, and the full executable-region inventory | not stated | 8 |
| MACHINE_STATE_EVIDENCE12 | "Admission replays normalized instruction and region rows against closed target instruction specifications, proves exact byte coverage, and then composes the footprint." | Certificate and replay | Admission must replay rows, prove exact byte coverage, and compose the footprint | not stated | 9 |
| MACHINE_STATE_EVIDENCE13 | "A second whole-image decoder does not define a competing admission path." | Certificate and replay | Admission must not use an alternate path defined only by a whole-image decoder | not stated | 9 |
| MACHINE_STATE_EVIDENCE14 | "Checked Omega code supplies derived evidence." | Certificate and replay | Evidence from checked Omega code must be treated as derived evidence | not stated | 8 |
| MACHINE_STATE_EVIDENCE15 | "Raw/admitted leaves require explicit accepted claims with provider provenance; the trust report distinguishes them." | Certificate and replay | Raw or admitted leaves must carry explicit accepted claims with provenance and must be distinguished in the trust report | not stated | 8 |
| MACHINE_STATE_EVIDENCE16 | "Static and dynamically loaded artifacts use the same checking boundary." | Certificate and replay | Static and dynamically loaded artifacts must use identical checking rules | not stated | 9 |
| MACHINE_STATE_EVIDENCE17 | "Adding an executable-region origin requires its replay before a producer may claim complete coverage." | Certificate and replay | New executable-region origins must be replayed before claiming complete byte coverage | not stated | 9 |
| MACHINE_STATE_EVIDENCE18 | "Every byte-bearing compiler instruction with final-byte validation also needs a footprint row." | Certificate and replay | Each such instruction must have a footprint row | not stated | 10 |
| MACHINE_STATE_EVIDENCE19 | "Each nonempty retained instruction chooses exactly one replay authority: compiler target specification or checked-assembly catalog." | Certificate and replay | Nonempty retained instructions must pick exactly one replay authority among those two | not stated | 9 |
| MACHINE_STATE_EVIDENCE20 | "Zero-width scaffolding may choose neither." | Certificate and replay | Zero-width scaffolding may omit both replay authorities | not stated | 9 |
| MACHINE_STATE_EVIDENCE21 | "Unsupported shapes reject rather than disappear from the union." | Certificate and replay | Unsupported instruction shapes must cause rejection, not silent omission from the union | not stated | 9 |
| MACHINE_STATE_EVIDENCE22 | "Catalog replay includes every operand loader and the flag, stack, and control effects of fixed instruction sequences." | Certificate and replay | Checked-assembly catalog replay must cover operand loaders and flag, stack, and control effects | not stated | 10 |
| MACHINE_STATE_EVIDENCE23 | "The checker binds catalog-row counts to replayed validation counts, composes the complete instruction union, compares it with the state-plan-validated union, and binds the normalized identity into the certificate before serialization." | Certificate and replay | The checker must equalize counts, build and compare instruction unions, and bind normalized identity into the certificate before serialization | not stated | 8 |
| MACHINE_STATE_EVIDENCE24 | "Fragment order and duplication do not affect normalized set union." | Certificate and replay | Normalized set union must ignore fragment order and duplication | not stated | SHARED |
| MACHINE_STATE_EVIDENCE25 | "Object and image validation still establish that every realized fragment was supplied." | Certificate and replay | Object and image validation must prove every realized fragment was provided | not stated | 9 |
| MACHINE_STATE_EVIDENCE26 | "Generated callable and installed-entry transport retains target-decoded rows covering its complete byte span." | Certificate and replay | That transport must keep target-decoded rows spanning its full byte range | not stated | 10 |
| MACHINE_STATE_EVIDENCE27 | "These rows distinguish register reads from writes, include stack, flags and control-state effects, and bind internal control targets and the exact body-call relocation." | Certificate and replay | Rows must separate reads and writes, record stack, flags, and control effects, and bind control targets and body-call relocation | not stated | 10 |
| MACHINE_STATE_EVIDENCE28 | "Final-byte replay uses the same closed target form owners after relocation." | Certificate and replay | Final-byte replay after relocation must use the same closed target form owners | not stated | 9 |
| MACHINE_STATE_EVIDENCE29 | "The entry's use ceiling covers the union of this transport and the complete synchronous handler/callee closure;" | Certificate and replay | Entry use ceiling must include the transport union and the full synchronous handler or callee closure | at entry | 10 |
| MACHINE_STATE_EVIDENCE30 | "preservation checks use the clobbered subset." | Certificate and replay | Preservation checks must use only the clobbered subset of state | not stated | 10 |
| MACHINE_STATE_EVIDENCE31 | "An ordinary ABI does not grant an implicit allowance for stack or instruction-pointer transport." | Certificate and replay | Ordinary ABI must not be read as implicitly permitting stack or instruction-pointer transport | not stated | 3 |
| MACHINE_STATE_EVIDENCE32 | "The checked union is retained with its exact executable region and included in inventory identity." | Certificate and replay | The checked instruction union must stay tied to its executable region and appear in inventory identity | not stated | 8 |
| MACHINE_STATE_EVIDENCE33 | "Return-control evidence must match `CallPlan::entry_control`;" | Exit realization | Return-control evidence must match `CallPlan::entry_control` | at normal return | 10 |
| MACHINE_STATE_EVIDENCE34 | "the restored-state set must match `StatePlan`." | Exit realization | Restored machine state must match `StatePlan` | at normal return | 10 |
| MACHINE_STATE_EVIDENCE35 | "An opaque provider supplies the accepted claim under a root-reported trust receipt or adequate-hardware-isolation receipt." | Exit realization | Opaque providers must supply the accepted claim with one of those receipts | not stated | 9 |
| MACHINE_STATE_EVIDENCE36 | "Missing, unreported, or mismatched evidence rejects before forming provider execution." | Exit realization | Missing, unreported, or mismatched evidence must reject before provider execution is formed | not stated | 8 |

## Ambiguous sentences

| Sentence | Reading A | Reading B |
| --- | --- | --- |
| "The evaluated [call and state plans](calling_plans.md) are the published boundary promise." | Only evaluated plans may be published; evidence cannot extend the promise. | Evaluated plans are what gets published; other artifacts may still carry supplementary non-promise metadata. |
| "Actual register and machine-state use is evidence about one provider artifact, not an additional caller contract." | Callers never gain obligations from realization evidence beyond evaluated plans. | Evidence binds one artifact's implementation; whether callers see that evidence at link time or only auditors do is not stated. |
| "Instruction selection and allocation must honor the state ceiling." | Instruction selection and register allocation must each respect the same published ceiling. | "Honor" may mean only that independent validation catches violations; codegen may emit first and fail validation later. |
| "Independent validation checks the complete final artifact after inlining, specialization, relaxation, thunks/veneers, generated stubs, and admitted indirect leaves:" | Validation runs only once all listed transforms are applied to the final bytes. | "Complete" may include only compiler-emitted transforms; provider-linked leaves might be replayed separately before this check. |
| "Changing allocation within those bounds revalidates the implementation without changing the published promise." | Any in-bounds allocation change requires revalidation but leaves the plan-facing promise unchanged. | Revalidation might be optional for some allocation tweaks if footprint inequalities still hold; "without changing" applies only to successful revalidation. |
| "Contextual specialization may produce a callee under a restricted state ceiling; it is not a new generic type application." | Specialization is a restricted-ceiling codegen variant, not a separate monomorphization. | Specialization may still use generic metadata internally; "not a new generic type application" forbids only surfacing a new type application to the type checker. |
| "Final-artifact validation is mandatory." | No artifact ships without passing final-artifact validation. | Validation is mandatory for claiming complete coverage; partial builds without the claim may omit it. |
| "One self-describing certificate binds exact final bytes, placements, and the complete executable-region inventory." | One certificate aggregates all three; partial certificates are invalid. | Multiple fragments may serialize into one self-describing envelope; "one" refers to the logical certificate object. |
| "A second whole-image decoder does not define a competing admission path." | Whole-image decode cannot substitute for row replay and footprint composition. | A decoder may assist debugging but cannot admit without the primary path; whether auxiliary decode is allowed is not stated. |
| "Checked Omega code supplies derived evidence." | Compiler may infer evidence automatically for checked Omega bodies. | "Derived" may mean evidence is computed from plans plus codegen, not authored literally in source. |
| "Raw/admitted leaves require explicit accepted claims with provider provenance; the trust report distinguishes them." | Leaves without checked Omega bodies must carry explicit claims distinguishable in the trust report. | "Distinguishes" is reporting only; whether admission treats raw leaves stricter than admitted leaves is not enumerated here. |
| "Adding an executable-region origin requires its replay before a producer may claim complete coverage." | Each new region origin must replay before any complete-coverage claim. | Replay is required only for origins added after an initial certificate; incremental updates vs full re-admission is not stated. |
| "Each nonempty retained instruction chooses exactly one replay authority: compiler target specification or checked-assembly catalog." | Every retained instruction with bytes must pick exactly one authority for its lifetime. | "Retained" may exclude transient instructions; whether duplicates in the union share one authority is not stated. |
| "Zero-width scaffolding may choose neither." | Zero-width scaffolding need not bind to either replay authority. | Scaffolding without bytes might still need a footprint row elsewhere; "choose neither" applies only to replay authority. |
| "Unsupported shapes reject rather than disappear from the union." | Unsupported shapes must error; they cannot be dropped from the composed union silently. | Reject may happen at catalog binding rather than admission; "disappear" might mean only omission from reports, not from binaries. |
| "Fragment order and duplication do not affect normalized set union." | Union normalization is commutative and collapses duplicates. | Order and duplication affect only certificate serialization, not semantic footprint; other certificate fields might still be order-sensitive. |
| "The entry's use ceiling covers the union of this transport and the complete synchronous handler/callee closure;" | Use ceiling at entry must account for transport plus entire synchronous closure. | "Complete" closure might exclude asynchronous tails linked elsewhere; whether async suspension is in the union is not stated. |
| "preservation checks use the clobbered subset." | Only clobbered state participates in preservation verification. | Preservation might compare clobbered subset against full `StatePlan`; whether non-clobbered state is unchecked here is not stated. |
| "An ordinary ABI does not grant an implicit allowance for stack or instruction-pointer transport." | Stack and IP changes need explicit evidence beyond default ABI rules. | "Ordinary ABI" might still permit stack/IP changes visible in plans; implicit allowance refers only to undocumented transport beyond plans. |
| "Return-control evidence must match `CallPlan::entry_control`;" | Exit control transfer must match the entry control facet of the call plan. | "Match" may mean structural equality only, not bitwise identity of encodings. |
| "An opaque provider supplies the accepted claim under a root-reported trust receipt or adequate-hardware-isolation receipt." | Provider must attach exactly one of those receipt kinds to the claim. | Either receipt type suffices; whether both may be present or partial receipts satisfy is not stated. |
| "Missing, unreported, or mismatched evidence rejects before forming provider execution." | Provider execution formation is blocked on any evidence defect. | "Before forming" may allow staging execution graphs that are discarded on mismatch; whether partial provider graphs exist is not stated. |

## Terms used without definition in this chapter

- evaluated call and state plans
- published boundary promise
- provider artifact
- caller contract
- state ceiling
- instruction selection and allocation
- independent validation
- final artifact
- inlining / specialization / relaxation
- thunks / veneers
- generated stubs
- admitted indirect leaves
- actual transitive footprint
- permitted transitive use
- actual clobbers
- unsaved interrupted state
- published promise
- contextual specialization
- restricted state ceiling
- generic type application
- final-artifact validation
- internal validation records
- self-describing certificate
- exact final bytes
- placements
- executable-region inventory
- admission
- normalized instruction and region rows
- closed target instruction specifications
- exact byte coverage
- footprint (composed)
- whole-image decoder
- derived evidence
- checked Omega code
- raw/admitted leaves
- explicit accepted claims
- provider provenance
- trust report
- checking boundary
- executable-region origin
- complete coverage
- byte-bearing compiler instruction
- final-byte validation
- footprint row
- replay authority
- compiler target specification
- checked-assembly catalog
- zero-width scaffolding
- unsupported shapes
- operand loader
- catalog-row counts / replayed validation counts
- complete instruction union
- state-plan-validated union
- normalized identity
- fragment order / duplication
- normalized set union
- realized fragment
- generated callable and installed-entry transport
- target-decoded rows
- register reads / writes
- control-state effects
- internal control targets
- body-call relocation
- closed target form owners
- entry use ceiling
- synchronous handler / callee closure
- clobbered subset
- preservation checks
- ordinary ABI
- stack or instruction-pointer transport
- checked union
- inventory identity
- return-control evidence
- `CallPlan::entry_control`
- `StatePlan`
- opaque provider
- accepted claim
- root-reported trust receipt
- adequate-hardware-isolation receipt
- provider execution

CHAPTER COMPLETE
