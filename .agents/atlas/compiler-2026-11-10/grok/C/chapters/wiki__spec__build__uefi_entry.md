# UEFI entry and firmware handoff

Source: `wiki/spec/build/uefi_entry.md` lines 1–150 (read).

## Rules

| rule id | the exact sentence, quoted | the heading it is under | what must be checked, in plain words | when it must hold | lane |
| --- | --- | --- | --- | --- | --- |
| UEFI_ENTRY1 | "Freestanding is an immutable selected target/provider property, not a source machine category." | UEFI entry and firmware handoff | Freestanding must be fixed as selected target/provider property and must not be treated as a source machine category | not stated | 9 |
| UEFI_ENTRY2 | "No hosted providers are ambient." | UEFI entry and firmware handoff | No hosted provider may be ambient for the selected build | not stated | 9 |
| UEFI_ENTRY3 | "The target supplies its explicit firmware, storage, device, scheduling, entry, and exit contracts;" | UEFI entry and firmware handoff | Selected target must supply explicit firmware, storage, device, scheduling, entry, and exit contracts | not stated | 9 |
| UEFI_ENTRY4 | "`build.omg` binds the [semantic program entry](entry_roots.md) and selects provider slots." | UEFI entry and firmware handoff | `build.omg` must bind semantic program entry and select provider slots | not stated | 9 |
| UEFI_ENTRY5 | "Cathedral owns its OS policies and lifecycle, not the compiler." | UEFI entry and firmware handoff | Compiler must not own or author Cathedral OS policies and lifecycle as compiler primitives | not stated | 9 |
| UEFI_ENTRY6 | "UEFI is not an exception to source-owned platform code." | Authored firmware definitions and adapters | UEFI platform code must be source-owned like other targets; compiler must not treat UEFI as exempt | not stated | 9 |
| UEFI_ENTRY7 | "The selected target package authors firmware data declarations, layout policies, calling policies, constants, integrity checks, and bootstrap/handoff machines in ordinary Omega." | Authored firmware definitions and adapters | Listed firmware artifacts must be authored in ordinary Omega by the selected target package | not stated | 9 |
| UEFI_ENTRY8 | "System Table, Boot Services, and Loaded Image geometry comes from checked [layout plans](../layouts/plans.md), not a second firmware-field catalog built into the compiler." | Authored firmware definitions and adapters | That geometry must come from checked layout plans; compiler must not supply a separate built-in firmware-field catalog | not stated | 9 |
| UEFI_ENTRY9 | "opacity to application code does not require compiler-owned definitions." | Authored firmware definitions and adapters | Private firmware representations must not force compiler-owned definitions merely for application opacity | not stated | 9 |
| UEFI_ENTRY10 | "It binds exact selected source contracts and admitted environment premises to their generated realization." | Authored firmware definitions and adapters | Generated realization must bind to exact selected source contracts and admitted environment premises | not stated | 9 |
| UEFI_ENTRY11 | "Independent replay must check the source/plan/realization correspondence; a source digest alone does not establish it." | Authored firmware definitions and adapters | Independent replay must verify source/plan/realization correspondence; source digest alone must not establish correspondence | not stated | 8 |
| UEFI_ENTRY12 | "Firmware policies have one source definition; independent replay must not depend on a separately authored Rust copy of that policy." | Authored firmware definitions and adapters | Firmware policies must have a single source definition; independent replay must not rely on a separate Rust policy copy | not stated | 8 |
| UEFI_ENTRY13 | "Firmware invocation leaves retain explicit validity, provenance, lifecycle, and service-behavior assumptions." | Authored firmware definitions and adapters | Firmware invocation leaves must retain explicit validity, provenance, lifecycle, and service-behavior assumptions | at a call | 3 |
| UEFI_ENTRY14 | "Authored table validation alone cannot prove a pointer is valid or manufacture firmware authority." | Authored firmware definitions and adapters | Compiler must not treat authored table validation alone as proving pointer validity or manufacturing firmware authority | at the operation | 8 |
| UEFI_ENTRY15 | "Conversely, admitting those assumptions does not make ordinary parsing, allocation sequencing, retry logic, or status handling compiler primitives." | Authored firmware definitions and adapters | Admitting firmware assumptions must not turn ordinary parsing, allocation sequencing, retry logic, or status handling into compiler primitives | not stated | 9 |
| UEFI_ENTRY16 | "If a source implementation is blocked, name the missing general capability on the execution board rather than expanding a firmware-specific intrinsic." | Authored firmware definitions and adapters | Compiler must not expand firmware-specific intrinsics to bypass blocked source implementations when a general capability is missing | not stated | 9 |
| UEFI_ENTRY17 | "The UEFI x64 profile's physical `UefiPhysicalEntry::enter` accepts the platform's image handle and system-table occurrence and returns `EfiStatus` under the UEFI x64 convention and its exact selected calling policy." | Physical arrival and semantic roots | Physical entry must accept image handle and system-table occurrence and return `EfiStatus` under UEFI x64 and the exact selected calling policy | at entry | 9 |
| UEFI_ENTRY18 | "Its separate semantic `ProgramStorageEntry::enter` introduces exactly two `Extent in Granted` occurrences: image and initial storage." | Physical arrival and semantic roots | Semantic entry must introduce exactly two `Extent in Granted` occurrences (image and initial storage), no more and no fewer | at entry | 2 |
| UEFI_ENTRY19 | "Neither requirement replaces or refines the other." | Physical arrival and semantic roots | Physical and semantic entry requirements must remain distinct; neither may replace or refine the other | not stated | 9 |
| UEFI_ENTRY20 | "A generated physical shell invokes the exact source-authored target bootstrap adapter." | Physical arrival and semantic roots | Generated physical shell must invoke the exact source-authored target bootstrap adapter | at entry | 9 |
| UEFI_ENTRY21 | "The source continuation receives only the semantic schema's visible arguments." | Physical arrival and semantic roots | Source continuation must receive only arguments visible in the semantic schema | at a call | 3 |
| UEFI_ENTRY22 | "There is no optional automatically provisioned receiver or context object." | Physical arrival and semantic roots | Compiler must not optionally auto-provision a receiver or context object for the continuation | at entry | 9 |
| UEFI_ENTRY23 | "Additional state uses ordinary storage or an argument explicitly established by that installed contract." | Physical arrival and semantic roots | Extra state must use ordinary storage or an argument explicitly established by the installed contract | not stated | 2 |
| UEFI_ENTRY24 | "The continuation does not receive hidden firmware arguments or project the private system table." | Physical arrival and semantic roots | Continuation must not receive hidden firmware arguments or project the private system table | at a call | 3 |
| UEFI_ENTRY25 | "The physical arrival contract retains pointer validity, alignment and lifetime, the paging/CPU regime, entry/exit control, and firmware lifecycle assumptions." | Physical arrival and semantic roots | Physical arrival contract must retain stated assumptions (pointer validity, alignment, lifetime, paging/CPU, entry/exit control, firmware lifecycle) | at entry | 2 |
| UEFI_ENTRY26 | "An ABI-compatible pointer or register arrangement establishes none of these." | Physical arrival and semantic roots | ABI-compatible pointer or register layout must not be treated as establishing those arrival assumptions | at entry | 3 |
| UEFI_ENTRY27 | "The image handle is provenance-bearing identification, not storage authority." | Physical arrival and semantic roots | Image handle must be treated as provenance identification, not storage authority | not stated | 2 |
| UEFI_ENTRY28 | "The selected Loaded Image provider admits correspondence to the real mapped image." | Physical arrival and semantic roots | Loaded Image provider admission must be correspondence to the real mapped image | not stated | 9 |
| UEFI_ENTRY29 | "Successful page allocation admits exclusive page custody; release admits its return." | Physical arrival and semantic roots | Page allocation success must admit exclusive page custody; release must admit custody return | at the operation | 2 |
| UEFI_ENTRY30 | "Layout and arithmetic checks derive facts above these premises, not the premises themselves." | Physical arrival and semantic roots | Layout and arithmetic checks may derive facts from premises but must not substitute for the premises themselves | not stated | 8 |
| UEFI_ENTRY31 | "Retain exact selected provider postconditions, input occurrences, phase, schema, calling plan, continuation, and receipts." | Physical arrival and semantic roots | Checking must retain exact selected provider postconditions, input occurrences, phase, schema, calling plan, continuation, and receipts | not stated | 3 |
| UEFI_ENTRY32 | "Both semantic root geometry obligations must hold before either complete root is introduced." | Physical arrival and semantic roots | Both semantic root geometry obligations must be satisfied before introducing either complete root | at entry | 2 |
| UEFI_ENTRY33 | "Failure before installation calls no continuation and returns all moved bootstrap inputs." | Physical arrival and semantic roots | On failure before installation, continuation must not run and all moved bootstrap inputs must be returned | at entry | 2 |
| UEFI_ENTRY34 | "Target result mapping specifies recoverable bootstrap errors; normal Unit return maps to success." | Physical arrival and semantic roots | Target result mapping must map recoverable bootstrap errors; normal Unit return must map to success | at normal return | 9 |
| UEFI_ENTRY35 | "Crash, trap, or abort does not manufacture a returning status result." | Physical arrival and semantic roots | Crash, trap, or abort must not produce a returning status result | at normal return | 3 |
| UEFI_ENTRY36 | "Initial storage is a separately owned reserved region proved disjoint from the image root or an admitted allocation." | Physical arrival and semantic roots | Initial storage must be separately owned reserved region with disjointness proof from image root or admitted allocation | not stated | 2 |
| UEFI_ENTRY37 | "Image sections are subextents, not fresh roots." | Physical arrival and semantic roots | Image sections must be modeled as subextents, not fresh roots | not stated | 2 |
| UEFI_ENTRY38 | "Active stack, application state, or retained bootstrap state cannot hide inside an extent forwarded whole." | Physical arrival and semantic roots | Active stack, application state, or retained bootstrap state must not be hidden inside a whole forwarded extent | not stated | 2 |
| UEFI_ENTRY39 | "A shared parent allocation must partition exactly, leaving one contiguous disjoint residual when source receives one `Extent`." | Physical arrival and semantic roots | Shared parent allocation must partition exactly to one contiguous disjoint residual when source gets one `Extent` | not stated | 2 |
| UEFI_ENTRY40 | "The closed UEFI x64 profile supplies a canonical target-semantic minimum of 128 KiB available stack with 16-byte alignment." | Stack contract | Closed UEFI x64 profile must supply canonical minimum 128 KiB stack with 16-byte alignment | not stated | 9 |
| UEFI_ENTRY41 | "It remains symbolic until target closure and participates in observation/artifact identity." | Stack contract | Stack minimum remains symbolic until target closure and must participate in observation/artifact identity | not stated | 9 |
| UEFI_ENTRY42 | "Physical arrival separately admits that the actual firmware invocation meets the profile; evaluating the constant proves no runtime backing." | Stack contract | Physical arrival must separately admit firmware meets profile; evaluating the constant must not prove runtime backing | at entry | 8 |
| UEFI_ENTRY43 | "Same-stack admission includes generated shell, live adapter frames, nested program/provider WCSU, and the explicit target reserve." | Stack contract | Same-stack admission must account for generated shell, adapter frames, nested program/provider WCSU, and explicit target reserve | not stated | 9 |
| UEFI_ENTRY44 | "Each term binds its actual checked or emitted producer." | Stack contract | Each same-stack term must bind to its actual checked or emitted producer | not stated | 9 |
| UEFI_ENTRY45 | "Alternatively, a checked stub may switch to privately provisioned Omega storage while retaining firmware return state for a returning profile." | Stack contract | Checked stub path may use private Omega storage only while retaining firmware return state for returning profile | not stated | 2 |
| UEFI_ENTRY46 | "Numeric totals from a different invocation or private firmware ledger cannot replace that exact evidence." | Stack contract | Numeric totals from another invocation or private firmware ledger must not replace required exact evidence | not stated | 8 |
| UEFI_ENTRY47 | "A returning `UefiApplication` keeps Boot Services live, reclaims adapter-owned allocations, and returns through its physical result map." | Returning application versus OS handoff | Returning application profile must keep Boot Services live, reclaim adapter allocations, and return via physical result map | at normal return | 9 |
| UEFI_ENTRY48 | "A successful OS-loader handoff is non-returning and has a different lifecycle, despite sharing the physical calling convention." | Returning application versus OS handoff | Successful OS-loader handoff must be non-returning with distinct lifecycle even if calling convention matches | not stated | 9 |
| UEFI_ENTRY49 | "One cannot be inferred from the other's Unit source signature." | Returning application versus OS handoff | Returning application vs OS-loader handoff must not be inferred from the other's Unit source signature alone | not stated | 9 |
| UEFI_ENTRY50 | "An ordinary source-authored adapter implements the bounded linear `GetMemoryMap`/`ExitBootServices` cycle under the selected target/provider contract:" | Returning application versus OS handoff | OS handoff must be implemented by source-authored adapter as bounded linear `GetMemoryMap`/`ExitBootServices` cycle under selected contract | not stated | 9 |
| UEFI_ENTRY51 | "Acquire explicit map-buffer storage and retain allocations, Boot Services, surviving-stack evidence, physical invocation, and remaining attempts." | Returning application versus OS handoff | Step 1 must acquire explicit map-buffer storage and retain listed obligations through the cycle | at the operation | 2 |
| UEFI_ENTRY52 | "Read the map using its returned descriptor stride/version and retain the exact snapshot and map key." | Returning application versus OS handoff | Map read must use returned descriptor stride/version and retain exact snapshot and map key | at the operation | 2 |
| UEFI_ENTRY53 | "Every projection fits the returned byte range." | Returning application versus OS handoff | Every map projection must fit the returned byte range | at the operation | 6 |
| UEFI_ENTRY54 | "Attempt exit. A stale-key outcome retires that snapshot and returns the unchanged live custody for retry, decreasing the measure only under `1 <= remaining`." | Returning application versus OS handoff | Stale-key exit must retire snapshot, restore unchanged live custody for retry, and decrease remaining measure only when `1 <= remaining` | at the operation | 2 |
| UEFI_ENTRY55 | "Exhaustion returns the target-authored EFI error." | Returning application versus OS handoff | When attempts exhaust, must return target-authored EFI error | at normal return | 9 |
| UEFI_ENTRY56 | "Success consumes Boot Services and the final map into the exit receipt, transitions allocation custody, and retains the final-map obligation." | Returning application versus OS handoff | Successful exit must consume Boot Services and final map into exit receipt, transition custody, and retain final-map obligation | at the operation | 2 |
| UEFI_ENTRY57 | "No destructor conceals that fallible protocol." | Returning application versus OS handoff | Destructors must not hide the fallible `GetMemoryMap`/`ExitBootServices` protocol | not stated | 2 |
| UEFI_ENTRY58 | "Boot-scoped providers end;" | Returning application versus OS handoff | Boot-scoped providers must end on successful handoff | not stated | 9 |
| UEFI_ENTRY59 | "already allocated storage preserves its occurrence lineage while custody passes to the program." | Returning application versus OS handoff | Allocated storage must preserve occurrence lineage when custody passes to program | not stated | 2 |
| UEFI_ENTRY60 | "New physical-memory claims require final-map, successful exit, and target-policy evidence." | Returning application versus OS handoff | New physical-memory claims must require final-map, successful exit, and target-policy evidence | not stated | 2 |
| UEFI_ENTRY61 | "Reserved, runtime, ACPI, device, active bootstrap, and otherwise excluded ranges are not implicitly claimable." | Returning application versus OS handoff | Listed excluded ranges must not be implicitly claimable as physical memory | not stated | 2 |
| UEFI_ENTRY62 | "Runtime Services survive only under their separate post-exit contract." | Returning application versus OS handoff | Runtime Services may survive only under separate post-exit contract | not stated | 9 |
| UEFI_ENTRY63 | "A bodyless handoff declaration backed by a compiler-specific retry-loop realization is not the completed source implementation." | Returning application versus OS handoff | Bodyless handoff plus compiler-only retry-loop realization must not count as completed source implementation | not stated | 9 |
| UEFI_ENTRY64 | "Generated entry and stack-transition mechanics remain compiler responsibilities; the protocol body uses ordinary machine calls, control flow, and custody checking." | Returning application versus OS handoff | Entry and stack-transition mechanics are compiler-owned; protocol body must use ordinary machines with custody checking | not stated | SHARED |
| UEFI_ENTRY65 | "The incoming firmware stack is not assumed to survive exit." | Returning application versus OS handoff | Compiler must not assume incoming firmware stack survives exit | not stated | 2 |
| UEFI_ENTRY66 | "Switch to an accounted handoff stack before the final attempt or establish the incoming stack's required lifetime." | Returning application versus OS handoff | Before final exit attempt must switch to accounted handoff stack or establish required lifetime for incoming stack | at the operation | 2 |
| UEFI_ENTRY67 | "Retain its claim through success and expose only disjoint residual storage to source." | Returning application versus OS handoff | Handoff stack claim must be retained through success; source may see only disjoint residual storage | at normal return | 2 |
| UEFI_ENTRY68 | "Use ordinary checked [materialization](hardware_materialization.md) and [interrupt obligations](interrupt_obligations.md), not an IDT language or compiler-owned OS tables." | Hardware preparation and reporting | Hardware prep must use checked materialization and interrupt obligations; must not use IDT language or compiler-owned OS tables | not stated | 9 |
| UEFI_ENTRY69 | "Stack/preemption selection drives both concrete gate IST placement and WCSU; these cannot be independently authored facts." | Hardware preparation and reporting | IST placement and WCSU must follow stack/preemption selection; must not be independently authored facts | not stated | 10 |
| UEFI_ENTRY70 | "Split IDT addresses normally materialize after rebasing, whereas loader-consumed fields must fit native relocation forms." | Hardware preparation and reporting | Split IDT addresses materialize after rebasing; loader-consumed fields must fit native relocation forms | not stated | 9 |
| UEFI_ENTRY71 | "Where practical, reserve and validate final table/stack placements before exit." | Hardware preparation and reporting | Final table/stack placements should be reserved and validated before exit when practical | before exit | 9 |
| UEFI_ENTRY72 | "The post-exit critical sequence then performs only required mapping/stack transition, visibility, prepared-root publication, checked table installation, and finalization." | Hardware preparation and reporting | Post-exit critical sequence must be limited to listed operations only | not stated | 9 |
| UEFI_ENTRY73 | "External interrupts remain disabled until the complete exception floor exists." | Hardware preparation and reporting | External interrupts must stay disabled until complete exception floor exists | on every edge | 10 |
| UEFI_ENTRY74 | "Software-fault-free claims retain explicit platform assumptions; they do not prove NMI, machine-check, or physical failure absent." | Hardware preparation and reporting | Software-fault-free claims must retain explicit platform assumptions and must not claim absence of NMI, machine-check, or physical failure | not stated | 8 |
| UEFI_ENTRY75 | "Reports bind selected requirements/providers, calling/state plans, admitted environment facts, extent scopes, artifact/materialization/visibility identities, external roots with reach/stack/nesting/version pins, instruction footprints, and remaining authority debts." | Hardware preparation and reporting | Reports must bind all listed requirement, plan, fact, identity, root, footprint, and debt fields | not stated | SHARED |
| UEFI_ENTRY76 | "Installed-ledger snapshots are not guessed build tables." | Hardware preparation and reporting | Installed-ledger snapshots must not be inferred or guessed from build tables alone | not stated | 9 |
| UEFI_ENTRY77 | "[Executable installation](executable_installation.md) owns boot authentication, AP artifacts, visibility, and quiescence independently." | Hardware preparation and reporting | Boot authentication, AP artifacts, visibility, and quiescence belong to executable installation contract, not this chapter's inline rules | not stated | 9 |
| UEFI_ENTRY78 | "The adapter validates arrival, installs lifecycle-scoped providers, obtains root geometry and correspondence evidence, then crosses the semantic installation edge." | Physical arrival and semantic roots | Bootstrap adapter must validate arrival, install lifecycle-scoped providers, obtain geometry and evidence, then cross semantic installation edge | at entry | 9 |
| UEFI_ENTRY79 | "Source packages may keep those representations private;" | Authored firmware definitions and adapters | Source packages may keep firmware representations private without violating checking rules | not stated | 9 |
| UEFI_ENTRY80 | "The compiler supplies generic checking, evaluation, lowering, and independent evidence replay, plus the necessary target entry shell and instruction/ABI primitives." | Authored firmware definitions and adapters | Compiler role is generic checking, evaluation, lowering, evidence replay, entry shell, and instruction/ABI primitives—not source-owned policy bodies | not stated | SHARED |

## Ambiguous sentences

| Sentence | Reading A | Reading B |
| --- | --- | --- |
| "Source packages may keep those representations private;" | Private representations are permitted; the compiler checks them through layout plans without requiring public ABI exposure. | "May" allows but does not require privacy; packages could still choose to expose representations publicly. |
| "If a source implementation is blocked, name the missing general capability on the execution board rather than expanding a firmware-specific intrinsic." | This is process guidance for humans; the compiler has no obligation to enforce board naming. | The compiler must reject or block builds that add firmware-specific intrinsics instead of using general capabilities. |
| "Where practical, reserve and validate final table/stack placements before exit." | "Where practical" is a soft requirement: compiler should enforce reservation/validation when the source provides enough information. | "Where practical" excuses skipping reservation/validation entirely whenever inconvenient. |
| "Alternatively, a checked stub may switch to privately provisioned Omega storage while retaining firmware return state for a returning profile." | The stub path is optional among valid implementations that meet stack evidence rules. | Any returning profile may use private storage without meeting same-stack admission evidence if a stub is checked. |
| "`1 <= remaining`" (in stale-key retry step) | Remaining attempts decrease only when the remaining count is at least one before the attempt. | The retry measure uses an invariant written `1 <= remaining` that must hold whenever a stale-key retry is permitted. |
| "Software-fault-free claims retain explicit platform assumptions; they do not prove NMI, machine-check, or physical failure absent." | Claims are qualified: compiler must surface assumptions and must not treat claims as proving absence of those fault classes. | "Retain explicit platform assumptions" might mean assumptions are preserved in reports only, not checked at compile time. |
| "Installed-ledger snapshots are not guessed build tables." | Ledger content must come from checked installation evidence, not inferred from static build metadata. | Build tables may still inform ledgers if combined with non-guess installation hooks. |

## Terms used without definition

- Freestanding
- selected target/provider property
- source machine category
- hosted providers
- ambient
- firmware, storage, device, scheduling, entry, and exit contracts
- provider slots
- Cathedral
- layout policies
- calling policies
- integrity checks
- bootstrap/handoff machines
- System Table
- Boot Services
- Loaded Image
- layout plans
- firmware-field catalog
- independent evidence replay
- target entry shell
- admitted environment premises
- generated realization
- source digest
- firmware invocation leaves
- firmware authority
- firmware-specific intrinsic
- execution board
- UEFI x64 profile
- `UefiPhysicalEntry::enter`
- image handle
- system-table occurrence
- `EfiStatus`
- UEFI x64 convention
- `ProgramStorageEntry::enter`
- `Extent in Granted`
- physical shell
- target bootstrap adapter
- semantic installation edge
- semantic schema
- installed contract
- private system table
- paging/CPU regime
- entry/exit control
- firmware lifecycle assumptions
- Loaded Image provider
- page custody
- input occurrences
- phase
- receipts
- semantic root geometry obligations
- complete root
- bootstrap inputs
- Target result mapping
- subextents
- shared parent allocation
- contiguous disjoint residual
- closed UEFI x64 profile
- target closure
- observation/artifact identity
- same-stack admission
- WCSU
- target reserve
- checked stub
- privately provisioned Omega storage
- firmware return state
- returning profile
- private firmware ledger
- `UefiApplication`
- physical result map
- OS-loader handoff
- Unit source signature
- `GetMemoryMap`/`ExitBootServices` cycle
- map-buffer storage
- surviving-stack evidence
- physical invocation
- remaining attempts
- descriptor stride/version
- map key
- stale-key outcome
- live custody
- exit receipt
- final-map obligation
- Boot-scoped providers
- occurrence lineage
- physical-memory claims
- target-policy evidence
- post-exit contract
- bodyless handoff declaration
- compiler-specific retry-loop realization
- stack-transition mechanics
- accounted handoff stack
- materialization
- interrupt obligations
- IDT language
- compiler-owned OS tables
- Stack/preemption selection
- IST placement
- Split IDT addresses
- rebasing
- loader-consumed fields
- native relocation forms
- post-exit critical sequence
- prepared-root publication
- checked table installation
- exception floor
- Software-fault-free claims
- calling/state plans
- extent scopes
- artifact/materialization/visibility identities
- external roots
- reach/stack/nesting/version pins
- instruction footprints
- authority debts
- Installed-ledger snapshots
- guessed build tables
- Executable installation
- AP artifacts
- quiescence

CHAPTER COMPLETE
