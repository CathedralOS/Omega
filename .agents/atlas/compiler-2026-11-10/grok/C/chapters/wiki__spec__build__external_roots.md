# Installed external roots

Source: `wiki/spec/build/external_roots.md` lines 1–141 (read).

## Rules

| rule id | the exact sentence, quoted | the heading it is under | what must be checked, in plain words | when it must hold | lane |
| --- | --- | --- | --- | --- | --- |
| EXTERNAL_ROOTS1 | "An external root admits entry without an Omega caller." | Installed external roots | Installed external roots must allow entry with no Omega caller on the stack | not stated | 9 |
| EXTERNAL_ROOTS2 | "Program entry, interrupts, and foreign callbacks use [target slots](entry_roots.md) and the same distinction between selected plans, installed authority, and runtime liveness." | Installed external roots | Program entry, interrupts, and foreign callbacks must use target slots and distinguish selected plans, installed authority, and runtime liveness | not stated | 9 |
| EXTERNAL_ROOTS3 | "A reclaimable inbound stub needs a registration owning the foreign-held edge and any required code/component lease." | Admission and removal | A reclaimable inbound stub must have registration that owns the foreign-held edge and any required code or component lease | not stated | 9 |
| EXTERNAL_ROOTS4 | "exact entry, boundary plan, selected provider/effect/receipt, and artifact;" | Admission and removal | The installed root ledger must retain exact entry, boundary plan, selected provider/effect/receipt, and artifact | not stated | 9 |
| EXTERNAL_ROOTS5 | "normalized service reach and provider exit assurance;" | Admission and removal | The installed root ledger must retain normalized service reach and provider exit assurance | not stated | 3 |
| EXTERNAL_ROOTS6 | "stack domains, nesting/preemption and acknowledgement relationships;" | Admission and removal | The installed root ledger must retain stack domains and nesting, preemption, and acknowledgement relationships | not stated | 10 |
| EXTERNAL_ROOTS7 | "independent resource ceilings, realized facts, and validation evidence; and" | Admission and removal | The installed root ledger must retain independent resource ceilings, realized facts, and validation evidence | not stated | 9 |
| EXTERNAL_ROOTS8 | "liveness, installed-code occurrence, and component-version pins." | Admission and removal | The installed root ledger must retain liveness, installed-code occurrence, and component-version pins | not stated | 9 |
| EXTERNAL_ROOTS9 | "Installation consumes independently supplied publication authority." | Admission and removal | Installation must consume independently supplied publication authority | at the operation | 9 |
| EXTERNAL_ROOTS10 | "Its linear root handle borrows installed-code custody." | Admission and removal | The linear root handle must borrow installed-code custody | not stated | 2 |
| EXTERNAL_ROOTS11 | "Removal returns authority only after exact unreachability and required quiescence." | Admission and removal | Removal must return authority only after exact unreachability and required quiescence | at the operation | 9 |
| EXTERNAL_ROOTS12 | "A process-lifetime static callback needs the same build-time plan and report, but no live replacement ledger when neither its code nor state can be reclaimed." | Admission and removal | Process-lifetime static callbacks must have the same build-time plan and report and must have no live replacement ledger when code and state are not reclaimable | not stated | 9 |
| EXTERNAL_ROOTS13 | "The validated selected provider plan survives in the root's normalized identity and provider execution." | Admission and removal | The validated selected provider plan must survive in the root's normalized identity and in provider execution | not stated | 9 |
| EXTERNAL_ROOTS14 | "Execution cannot accept a second independently supplied plan." | Admission and removal | Execution must reject a second independently supplied plan | at the operation | 9 |
| EXTERNAL_ROOTS15 | "Drift in entry, reach, provider, resource realization, installed occurrence, or boundary commitment rejects." | Admission and removal | Drift in entry, reach, provider, resource realization, installed occurrence, or boundary commitment must reject installation or execution | not stated | 9 |
| EXTERNAL_ROOTS16 | "Compact fingerprints are report coordinates;" | Admission and removal | Compact fingerprints must serve only as report coordinates, not as sole authority | not stated | SHARED |
| EXTERNAL_ROOTS17 | "the exact facts and strong identities remain available for replay." | Admission and removal | Exact facts and strong identities must remain available for replay | not stated | 8 |
| EXTERNAL_ROOTS18 | "The compiler compiles entry code and checks its calling, installation, stack, state, and lifetime contracts." | Secondary-processor startup | The compiler must compile entry code and check calling, installation, stack, state, and lifetime contracts | not stated | SHARED |
| EXTERNAL_ROOTS19 | "Hardware behavior that code cannot establish remains an explicit admitted boundary assumption." | Secondary-processor startup | Hardware behavior the code cannot establish must remain only as an explicit admitted boundary assumption, not as compiler-proven fact | not stated | 9 |
| EXTERNAL_ROOTS20 | "Bind that contract to the exact installed entry, processor, resources, and invocation." | Secondary-processor startup | The selected provider's startup contract must bind to the exact installed entry, processor, resources, and invocation | not stated | 9 |
| EXTERNAL_ROOTS21 | "Caller-supplied profile numbers establish no hardware facts." | Secondary-processor startup | Caller-supplied profile numbers must not establish hardware facts in the compiler model | not stated | 5 |
| EXTERNAL_ROOTS22 | "Return the account to pending custody so a fresh invocation can retry; no later arrival through the refused attempt may remain possible." | Secondary-processor startup | When dispatch is definitely not established, the account must return to pending custody and no later arrival through the refused attempt may remain possible | not stated | 9 |
| EXTERNAL_ROOTS23 | "The account stays invoked and held — neither withdrawable nor reissuable — and the outstanding carrier remains answerable by a later definitive receipt, which may still resolve the attempt to started." | Secondary-processor startup | When possibly dispatched with unconfirmed arrival, the account must stay invoked and held, not withdrawable or reissuable, with the carrier answerable to a later definitive receipt | not stated | 9 |
| EXTERNAL_ROOTS24 | "Timeout or lost acknowledgement releases nothing and erases no outstanding attempt." | Secondary-processor startup | Timeout or lost acknowledgement must not release custody and must not erase an outstanding attempt | not stated | 9 |
| EXTERNAL_ROOTS25 | "The processor is marked started and its accounted stack and state stay held until retirement." | Secondary-processor startup | On confirmed arrival at the agreed entry, the processor must be marked started and accounted stack and state must stay held until retirement | not stated | 10 |
| EXTERNAL_ROOTS26 | "Confirmation binds the exact invocation and installed entry." | Secondary-processor startup | Confirmation must bind the exact invocation and installed entry | not stated | 9 |
| EXTERNAL_ROOTS27 | "Sending a request or constructing an ordinary success record is not confirmation." | Secondary-processor startup | Sending a request or constructing an ordinary success record must not count as confirmation | not stated | 9 |
| EXTERNAL_ROOTS28 | "a settlement receipt must name the exact outstanding carrier and attest both that the carrier can no longer dispatch — no later arrival through this attempt — and that nothing executes on the account's dedicated stack or private state." | Secondary-processor startup | Settlement of an unconfirmed attempt requires a settlement receipt naming the exact outstanding carrier and attesting no further dispatch and no execution on the dedicated stack or private state | not stated | 9 |
| EXTERNAL_ROOTS29 | "Only then does the account leave the ledger and return its complete custody." | Secondary-processor startup | The account may leave the ledger and return complete custody only after a valid settlement receipt with both attestations | not stated | 2 |
| EXTERNAL_ROOTS30 | "A receipt missing either premise leaves the carrier outstanding, still answerable by a later definitive startup receipt or a later settlement answer." | Secondary-processor startup | A settlement receipt missing either premise must leave the carrier outstanding and answerable to a later definitive startup or settlement receipt | not stated | 9 |
| EXTERNAL_ROOTS31 | "It cannot require a fabricated successful-start record to reach cancellation." | Secondary-processor startup | Cancellation must not require fabricating a successful-start record | not stated | 9 |
| EXTERNAL_ROOTS32 | "Retirement of a started processor needs a provider quiescence receipt naming the started record exactly;" | Secondary-processor startup | Retiring a started processor must require a provider quiescence receipt that names the started record exactly | not stated | 9 |
| EXTERNAL_ROOTS33 | "an incomplete drain keeps the account held and hands the started evidence back." | Secondary-processor startup | An incomplete drain must keep the account held and return the started evidence | not stated | 9 |
| EXTERNAL_ROOTS34 | "An unresolved attempt cannot become a fresh independent attempt by forgetting its identity:" | Secondary-processor startup | An unresolved attempt must not become a fresh independent attempt by identity erasure | not stated | 9 |
| EXTERNAL_ROOTS35 | "completion and settlement both bind the record's outstanding invocation, so delayed or replayed receipts cannot resolve another attempt or release its resources." | Secondary-processor startup | Completion and settlement must bind the record's outstanding invocation so delayed or replayed receipts cannot resolve another attempt or release its resources | not stated | 9 |
| EXTERNAL_ROOTS36 | "Each processor's startup resources are accounted separately — a dedicated stack class and a private state extent — so no two admitted processors can share one backing." | Secondary-processor startup | Each processor must have separately accounted dedicated stack class and private state extent; no two admitted processors may share one backing | not stated | 10 |
| EXTERNAL_ROOTS37 | "The startup ledger borrows the installed entry for its whole lifetime:" | Secondary-processor startup | The startup ledger must borrow the installed entry for its whole lifetime | not stated | 2 |
| EXTERNAL_ROOTS38 | "An admitted account that has never been invoked may withdraw with its resources returned intact;" | Secondary-processor startup | An admitted account never invoked may withdraw with resources returned intact | not stated | 2 |
| EXTERNAL_ROOTS39 | "once an invocation is outstanding the account can no longer withdraw, since the attempt may still arrive, and a started account stays held until retirement." | Secondary-processor startup | With an outstanding invocation the account must not withdraw; a started account must stay held until retirement | not stated | 2 |
| EXTERNAL_ROOTS40 | "no completion, settlement, or retirement edge releases that installation while an attempt under it could still exist." | Secondary-processor startup | No completion, settlement, or retirement edge may release the installed-entry installation while an attempt under it could still exist | on every edge | 9 |
| EXTERNAL_ROOTS41 | "These columns are independent; a calling-plan digest substitutes for none of them." | Resource columns | Stack, logical work, and machine-state columns must stay independent; a calling-plan digest must not substitute for any column | not stated | 9 |
| EXTERNAL_ROOTS42 | "Root admission composes the artifact-wide stack/nesting relation, not only each body's demand." | Resource columns | Root admission must compose the artifact-wide stack and nesting relation, not only each body's demand | not stated | 10 |
| EXTERNAL_ROOTS43 | "Logical-work evidence distinguishes reconstructed Terminal entry/segment facts from admitted opaque-provider claims, and joins each to its actual installed occurrence." | Resource columns | Logical-work evidence must distinguish reconstructed Terminal entry/segment facts from admitted opaque-provider claims and join each to actual installed occurrence | not stated | 8 |
| EXTERNAL_ROOTS44 | "A segment claim is not a whole-entry certificate." | Resource columns | A segment claim must not be treated as a whole-entry certificate | not stated | 8 |
| EXTERNAL_ROOTS45 | "Reports retain ceilings, realized facts, and validation receipts without exposing private proof data or numeric entry addresses." | Resource columns | Reports must retain ceilings, realized facts, and validation receipts and must not expose private proof data or numeric entry addresses | not stated | 8 |
| EXTERNAL_ROOTS46 | "Maximum logical work is not WCET;" | Resource columns | Maximum logical work must not be interpreted as WCET | not stated | 9 |
| EXTERNAL_ROOTS47 | "it may support analysis but reserves no register, changes no ABI, and authorizes no sponsor, transfer, or resume operation." | Resource columns | Maximum logical work may support analysis but must not reserve registers, change ABI, or authorize sponsor, transfer, or resume operations | not stated | 9 |
| EXTERNAL_ROOTS48 | "`StatePlan` is part of the public boundary contract." | Resource columns | `StatePlan` must be part of the public boundary contract | not stated | 9 |
| EXTERNAL_ROOTS49 | "Stack and work realization facts normally belong to the candidate and admission provision, not public API identity." | Resource columns | Stack and work realization facts must normally belong to candidate and admission provision, not public API identity | not stated | 9 |
| EXTERNAL_ROOTS50 | "A replacement contract promising no reprovisioning must explicitly publish the resource ceiling that promise requires." | Resource columns | A replacement contract promising no reprovisioning must explicitly publish the resource ceiling the promise requires | not stated | 9 |
| EXTERNAL_ROOTS51 | "Equal public contracts do not excuse replaying stale candidate-specific resource evidence." | Resource columns | Equal public contracts must not excuse replaying stale candidate-specific resource evidence | not stated | 8 |
| EXTERNAL_ROOTS52 | "An installation-bound requirement may declare one bounded abstract row `reaches <= Bound`." | Installation-bound reach | An installation-bound requirement may declare at most one bounded abstract row `reaches <= Bound` | not stated | 3 |
| EXTERNAL_ROOTS53 | "The root manifest retains its exact requirement-path identity, normalized bound, and all internal dependencies before selection." | Installation-bound reach | The root manifest must retain exact requirement-path identity, normalized bound, and all internal dependencies before selection | not stated | 9 |
| EXTERNAL_ROOTS54 | "Ordinary callable package/component contracts cannot carry the unresolved row." | Installation-bound reach | Ordinary callable package or component contracts must not carry the unresolved installation-bound reach row | not stated | 9 |
| EXTERNAL_ROOTS55 | "Selection supplies the exact operation row." | Installation-bound reach | Selection must supply the exact operation row | not stated | 9 |
| EXTERNAL_ROOTS56 | "Installation proves it is within the bound, substitutes it through the complete root closure, and rejects any remaining unresolved row." | Installation-bound reach | Installation must prove the selected operation row is within the bound, substitute it through the complete root closure, and reject any remaining unresolved row | not stated | 3 |
| EXTERNAL_ROOTS57 | "Equal service sets do not establish entry/completion or multi-operation protocol coherence; that requires exact provider execution and lineage." | Installation-bound reach | Equal service sets must not establish entry, completion, or multi-operation protocol coherence without exact provider execution and lineage | not stated | 3 |
| EXTERNAL_ROOTS58 | "Their selected applications must likewise resolve to finite rows in the installed root closure." | Installation-bound reach | Selected applications of generic APIs with deferred static callback or member reach must resolve to finite rows in the installed root closure | not stated | 3 |
| EXTERNAL_ROOTS59 | "Such dependencies do not create installation slots, select providers, or grant authority." | Installation-bound reach | Static reach dependencies must not create installation slots, select providers, or grant authority | not stated | 9 |
| EXTERNAL_ROOTS60 | "Composed WCSU and entry-context derivation." | Resource columns | Stack realization evidence must be composed WCSU and entry-context derivation | not stated | 10 |
| EXTERNAL_ROOTS61 | "Maximum logical work and IR proof/admitted-provider evidence." | Resource columns | Logical-work realization evidence must include maximum logical work and IR proof or admitted-provider evidence | not stated | 8 |
| EXTERNAL_ROOTS62 | "Actual footprint and final-code evidence." | Resource columns | Machine-state realization evidence must be actual footprint and final-code evidence | not stated | 8 |
| EXTERNAL_ROOTS63 | "Installation is the source of evidence for `Atomic::interruption_fence`." | Interruption ordering | Installation must be the source of evidence for `Atomic::interruption_fence` | at the operation | 10 |
| EXTERNAL_ROOTS64 | "The root route identifies handler, asynchronous source, execution context, and interrupted-code relationship sufficiently to derive same-context entry." | Interruption ordering | The root route must identify handler, asynchronous source, execution context, and interrupted-code relationship enough to derive same-context entry | not stated | 10 |
| EXTERNAL_ROOTS65 | "The operation cannot assert that relationship itself." | Interruption ordering | `Atomic::interruption_fence` must not assert the same-context relationship by itself | at the operation | 10 |
| EXTERNAL_ROOTS66 | "Missing installed-root or provider evidence rejects." | Interruption ordering | Missing installed-root or provider evidence must reject use of the interruption ordering relation | not stated | 8 |
| EXTERNAL_ROOTS67 | "emitting a stub alone grants no installed root." | Interruption ordering | Emitting a stub alone must not grant an installed root | not stated | 9 |
| EXTERNAL_ROOTS68 | "The [component-publication contract](component_publication.md) owns live eras and publication transactions;" | Interruption ordering | Live eras and publication transactions must be owned by the component-publication contract, not by external-root installation alone | not stated | 9 |

## Ambiguous sentences (two readings)

| quoted sentence | reading A | reading B |
| --- | --- | --- |
| "OS table schemas, publication policy, and device protocols belong to consumer packages, not compiler-owned lifecycle types." | The compiler must not model those artifacts as its own lifecycle types. | The compiler may still validate stubs that reference consumer-defined types without owning publication policy semantics. |
| "Cathedral or another consumer package owns processor discovery and startup sequencing, including requests, acknowledgements, retries, and cancellation." | The compiler must not implement discovery or sequencing; it only checks compiled entry contracts. | Consumer packages run sequencing at runtime; the compiler might still emit ledger checks for consumer protocol records. |
| "A checked consumer-authored handshake or an explicitly admitted provider guarantee may establish it; the compiler does not mandate a separate acknowledgement protocol." | Any confirmation path the compiler accepts must be either a checked consumer handshake or an admitted provider guarantee. | The compiler may accept confirmation without checking handshake code if a provider guarantee is admitted elsewhere. |
| "Each processor's startup resources are accounted separately — a dedicated stack class and a private state extent —" | Accounting is compiler-enforced at admission with distinct stack class and state extent per processor. | Separate accounting is a ledger obligation that consumer code maintains; the compiler only types the extent fields. |
| "The startup ledger borrows the installed entry for its whole lifetime:" | The linear borrow is enforced like other ownership borrows through installation edges. | "Borrows" describes custody semantics in reports, not necessarily a Rust-style loan in user code. |
| "Stack \| Admitted domain provision \| Composed WCSU and entry-context derivation." | Admission must reject stack realization without composed WCSU and entry-context derivation evidence. | WCSU derivation may be deferred to link time if domain provision is already admitted at compile time. |
| "Logical work \| Logical-work provision \| Maximum logical work and IR proof/admitted-provider evidence." | Both maximum logical work and IR or provider evidence are mandatory for installation. | Maximum logical work suffices when IR proof is replaced entirely by admitted-provider evidence per linked logical-work rules. |
| "Their selected applications must likewise resolve to finite rows in the installed root closure." | Every monomorphized application must resolve reach before root installation completes. | Deferred reach may resolve at a later build stage if still within the same installed root closure artifact. |
| "The relation orders compiler-visible coherent memory;" | Missing root evidence rejects the fence operation entirely. | The relation supplies ordering facts only where installed-root evidence exists; other memory ordering stays unspecified. |

## Terms used without definition in this chapter

- external root
- Omega caller
- target slots
- selected plans / installed authority / runtime liveness
- reclaimable inbound stub
- foreign-held edge
- code/component lease
- installed root ledger
- boundary plan
- selected provider/effect/receipt
- normalized service reach
- provider exit assurance
- stack domains
- nesting/preemption and acknowledgement relationships
- resource ceilings / realized facts / validation evidence
- publication authority
- linear root handle
- installed-code custody
- exact unreachability / required quiescence
- process-lifetime static callback
- live replacement ledger
- normalized identity
- compact fingerprints / strong identities / replay
- secondary processor / AP
- consumer package / Cathedral
- boundary providers
- admitted boundary assumption
- arrival regime / entry representation / placement constraints / completion guarantees
- caller-supplied profile numbers
- low-memory trampoline / address-derived startup vector
- pending custody / invoked and held / outstanding carrier
- definitive receipt / definitive startup receipt
- settlement receipt / fabricated successful-start record
- provider quiescence receipt
- dedicated stack class / private state extent
- startup ledger
- WCSU
- entry-context derivation
- logical-work provision / maximum logical work
- Terminal entry/segment facts
- opaque-provider claims
- segment claim / whole-entry certificate
- numeric entry addresses
- WCET
- sponsor, transfer, or resume operation
- `StatePlan`
- candidate and admission provision
- public API identity
- replacement contract / reprovisioning
- installation-bound requirement
- bounded abstract row `reaches <= Bound`
- requirement-path identity
- root manifest / root closure
- unresolved row / operation row
- service sets / provider execution and lineage
- static reach dependencies / installation slots
- `Atomic::interruption_fence`
- root route / same-context entry
- compiler-visible coherent memory
- component-publication contract / live eras / publication transactions

CHAPTER COMPLETE
