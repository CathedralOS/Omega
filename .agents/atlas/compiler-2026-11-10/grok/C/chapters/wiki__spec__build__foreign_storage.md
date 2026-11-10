# Foreign storage and lifetime

Source: `wiki/spec/build/foreign_storage.md` lines 1–145 (read).

## Rules

| rule id | the exact sentence, quoted | the heading it is under | what must be checked, in plain words | when it must hold | lane |
| --- | --- | --- | --- | --- | --- |
| FOREIGN_STORAGE1 | "Safe parameter/result contracts own foreign access and custody." | Foreign storage and lifetime | Foreign access and custody at boundaries must be governed by safe parameter/result contracts, not by calling plans alone | not stated | 3 |
| FOREIGN_STORAGE2 | "[Calling plans](calling_plans.md) describe representation, not permission or lifetime." | Foreign storage and lifetime | Calling plans must not be treated as granting permission or lifetime; only representation | not stated | 3 |
| FOREIGN_STORAGE3 | "[Content conservation](../resources/content_custody.md#call-conservation) and [structural access](../terminal-psi/structural_access.md) apply across native boundaries without exceptions." | Foreign storage and lifetime | Content conservation and structural access rules must apply at every native boundary with no exceptions | not stated | 2 |
| FOREIGN_STORAGE4 | "Native support is separate from these contracts; unsupported backing and lifetime routes reject." | Foreign storage and lifetime | Programs using unsupported backing or lifetime routes must be rejected | not stated | 9 |
| FOREIGN_STORAGE5 | "`addr` is numerical address data." | Inert addresses and marshaling | `addr` must be treated only as numerical address data, not as authority | not stated | 5 |
| FOREIGN_STORAGE6 | "`Ptr<T>` is a sealed, inert ABI carrier whose type parameter supplies representation/pointee shape." | Inert addresses and marshaling | `Ptr<T>` must be sealed and inert; pointee shape comes only from the type parameter | not stated | 9 |
| FOREIGN_STORAGE7 | "Neither grants authority; ordinary code cannot dereference, index, or manufacture a reference from it." | Inert addresses and marshaling | `addr` and `Ptr<T>` must not grant authority; ordinary code must be rejected if it dereferences, indexes, or manufactures a reference from them | at the operation | 2 |
| FOREIGN_STORAGE8 | "A binding materializes a pointer only from established storage claims after validating marshaling and calling policies." | Inert addresses and marshaling | Pointer bindings must be allowed only from established storage claims once marshaling and calling policies validate | not stated | 2 |
| FOREIGN_STORAGE9 | "Inbound pointers become checked views only through an authorized establishment route." | Inert addresses and marshaling | Inbound pointers must become checked views only via an authorized establishment route | at a call | 2 |
| FOREIGN_STORAGE10 | "Policies explicitly associate native pointer/count pairs or descriptor graphs with semantic extents." | Inert addresses and marshaling | Native pointer/count pairs and descriptor graphs must be explicitly associated with semantic extents in policy | not stated | 9 |
| FOREIGN_STORAGE11 | "The compiler does not guess that association." | Inert addresses and marshaling | The compiler must not infer pointer/count or descriptor-to-extent associations without explicit policy | not stated | 9 |
| FOREIGN_STORAGE12 | "A policy defining a native slice ABI derives the ordinary reference case; several separate native arguments are not equivalent to a record containing them." | Inert addresses and marshaling | Native slice ABI policy must derive the ordinary reference case; separate native arguments must not be equated to one record parameter | not stated | 9 |
| FOREIGN_STORAGE13 | "Raw data validation and encoding follow [format codecs](../layouts/codecs.md), not stronger facts invented by a cast." | Inert addresses and marshaling | Raw validation and encoding must follow format codecs only; casts must not invent stronger facts | not stated | 5 |
| FOREIGN_STORAGE14 | "`&T`, `&[T]`, `&write T`, `&write [T]`, `&mut T`, or `&mut [T]` grants only its exact access until return." | Outbound custody | Call-scoped borrow types must grant exactly their declared access only until return | at a call; at normal return | 2 |
| FOREIGN_STORAGE15 | "A linear protocol result owns stable backing, an explicitly lifetime-parameterized result retains a checked loan, or a permitted semantic snapshot uses private stable backing." | Outbound custody | Retention after return must use one of the three listed contract forms | at normal return | 2 |
| FOREIGN_STORAGE16 | "Authority moves into an already-established static/process-lifetime root." | Outbound custody | Process-lifetime disposition must move authority into an already-established static or process-lifetime root | at normal return | 2 |
| FOREIGN_STORAGE17 | "There is no general permanent-custodian spelling." | Outbound custody | Programs must not use a general permanent-custodian spelling for process-lifetime custody | not stated | 2 |
| FOREIGN_STORAGE18 | "A retained claim owns keepalive and reclamation authority, not necessarily its bytes inline." | Outbound custody | Retained claims must track keepalive and reclamation authority separately from inline bytes | not stated | 2 |
| FOREIGN_STORAGE19 | "It may lend lexical views only over rights the foreign side does not hold." | Outbound custody | Lexical views from a retained claim may cover only rights not held by the foreign side | not stated | 2 |
| FOREIGN_STORAGE20 | "Read-only foreign use may preserve semantic facts and permit Omega read views." | Outbound custody | Read-only foreign use may preserve facts and permit Omega read views under the stated conditions | at a call | 2 |
| FOREIGN_STORAGE21 | "Foreign writes invalidate facts over exactly the writable extent;" | Outbound custody | Foreign writes must invalidate semantic facts over exactly the writable extent | at the operation | 2 |
| FOREIGN_STORAGE22 | "terminal completion establishes the promised resulting facts." | Outbound custody | Terminal completion must establish the promised resulting facts after foreign writes | not stated | 2 |
| FOREIGN_STORAGE23 | "Partial release splits an exact separated subextent from the remainder still in flight." | Outbound custody | Partial release must split an exact separated subextent from the remainder still in flight | not stated | 2 |
| FOREIGN_STORAGE24 | "Published results and ordinary conservation determine whether use survives return." | Outbound custody | Whether use survives return must be determined from published results and ordinary conservation | at normal return | 2 |
| FOREIGN_STORAGE25 | "Consuming `Buffer` into `PendingWrite` can transfer its content;" | Outbound custody | Consuming `Buffer` into `PendingWrite` may transfer content when mapping is consuming | at the operation | 2 |
| FOREIGN_STORAGE26 | "`submit(&buffer) -> PendingWrite` cannot create owned retention." | Outbound custody | Non-consuming `submit(&buffer) -> PendingWrite` must not create owned retention | at normal return | 2 |
| FOREIGN_STORAGE27 | "An explicit `PendingWrite<'a>` may instead retain a compatible checked loan for `'a`." | Outbound custody | Explicit `PendingWrite<'a>` may retain a compatible checked loan for `'a` when spelled that way | not stated | 2 |
| FOREIGN_STORAGE28 | "Infer unambiguous consumed-input-to-result mappings; an ambiguous mapping needs an ordinary postcondition establishing the exact correspondence." | Outbound custody | Consumed-input-to-result mappings must be inferred only when unambiguous; ambiguous mappings require an ordinary postcondition for exact correspondence | not stated | 8 |
| FOREIGN_STORAGE29 | "Unsupported mappings reject." | Outbound custody | Input-to-result custody mappings not supported by the contracts must reject | not stated | 2 |
| FOREIGN_STORAGE30 | "Qualified content uses its owner-unique `Content<Owner, A>` projection, not a foreign-only extent algebra or annotation." | Outbound custody | Qualified content must use `Content<Owner, A>` projection, not a foreign-only extent algebra or annotation | not stated | 2 |
| FOREIGN_STORAGE31 | "Every retained native pointer slot needs exact stable-root, range, access, lifetime, and revision/lease provenance." | Outbound custody | Each retained native pointer slot must carry exact stable-root, range, access, lifetime, and revision/lease provenance | not stated | 2 |
| FOREIGN_STORAGE32 | "Unknown provenance rejects unless an admitted provider route establishes it." | Outbound custody | Unknown provenance must reject unless an admitted provider route establishes it | not stated | 9 |
| FOREIGN_STORAGE33 | "Embedded layouts have finite structural closure." | Outbound custody | Embedded layouts must have finite structural closure | not stated | 1 |
| FOREIGN_STORAGE34 | "Recursive or dynamically sized pointer graphs retain one arena/extent root covering the graph;" | Outbound custody | Recursive or dynamically sized pointer graphs must retain one arena/extent root covering the whole graph | not stated | 2 |
| FOREIGN_STORAGE35 | "the compiler does not traverse runtime pointers to discover custody." | Outbound custody | The compiler must not discover custody by traversing runtime pointers | not stated | 2 |
| FOREIGN_STORAGE36 | "Private snapshots require an explicit semantic contract permitting independent copying, with neither identity preservation nor unchecked write-back." | Outbound custody | Private snapshots must have an explicit contract permitting independent copying without identity preservation or unchecked write-back | not stated | 2 |
| FOREIGN_STORAGE37 | "They do not change the separately compiled public result type." | Outbound custody | Private snapshot semantics must not change the separately compiled public result type | not stated | 1 |
| FOREIGN_STORAGE38 | "Requirements publish unavoidable caller lifetimes/custody; realizations validate their concrete backing recipes." | Outbound custody | Requirements must publish unavoidable caller lifetimes/custody; realizations must validate concrete backing recipes | not stated | 9 |
| FOREIGN_STORAGE39 | "Concurrent native and Omega access is External placed backing." | Outbound custody | Concurrent native and Omega access must be modeled as External placed backing | not stated | 2 |
| FOREIGN_STORAGE40 | "Exclusive foreign mutation may instead move storage into the protocol and return it with preserved, invalidated, or outcome-dependent qualifications." | Outbound custody | Exclusive foreign mutation may move storage into the protocol and return it with the stated qualifications | not stated | 2 |
| FOREIGN_STORAGE41 | "Snapshot bytes count as persistent demand per live occurrence." | Outbound custody | Snapshot bytes must count as persistent demand for each live occurrence | not stated | 2 |
| FOREIGN_STORAGE42 | "Bounding aggregate demand also requires finite live-occurrence capacity: success transfers the exact capacity into the registration, rejection returns it unchanged, and successful unregister returns the same occurrence." | Outbound custody | Aggregate demand bounding must use finite live-occurrence capacity with exact transfer on success, unchanged return on rejection, and same occurrence on successful unregister | not stated | 2 |
| FOREIGN_STORAGE43 | "A consumable lifetime budget is distinct." | Outbound custody | Consumable lifetime budget must be tracked separately from live-occurrence capacity | not stated | 2 |
| FOREIGN_STORAGE44 | "Static thunk storage is bounded by distinct admitted callback identities, not live-registration count." | Outbound custody | Static thunk storage bounds must use distinct admitted callback identities, not live-registration count | not stated | 9 |
| FOREIGN_STORAGE45 | "[Callback registration](private_callbacks.md#registration-and-lifetime) separately owns the code/component lease." | Outbound custody | Callback registration must separately own the code/component lease per linked rules | not stated | 9 |
| FOREIGN_STORAGE46 | "Admission of a complete, applied safe boundary contract is the trust point for provider-established references." | Provider-owned views and completion | Provider-established references must be trusted only at admission of a complete, applied safe boundary contract | at a call | 3 |
| FOREIGN_STORAGE47 | "When the contract promises a reference result or a reference written into an output slot, its promised outcome establishes valid backing, the declared access and stability for the permitted lifetime." | Provider-owned views and completion | Promised reference results or output-slot references must establish valid backing, declared access, and stability for the permitted lifetime | at normal return | 2 |
| FOREIGN_STORAGE48 | "The referent may be provider-owned storage outside every input object." | Provider-owned views and completion | Provider-owned referents outside all inputs must be admissible when the contract promises them | not stated | 2 |
| FOREIGN_STORAGE49 | "No additional provider-root annotation or validity token is required solely because the reference crosses a foreign boundary." | Provider-owned views and completion | Crossing a foreign boundary must not by itself require extra provider-root annotation or validity token | not stated | 2 |
| FOREIGN_STORAGE50 | "The compiler retains the exact admitted provider invocation and resulting referent's provenance, access and lifetime, then checks subsequent uses, replacement and invalidation against that contract." | Provider-owned views and completion | Compiler must retain admitted invocation and referent provenance, access, and lifetime and check uses, replacement, and invalidation against the contract | not stated | 2 |
| FOREIGN_STORAGE51 | "It does not prove opaque provider compliance or infer spatial containment from an input's lifetime." | Provider-owned views and completion | Compiler must not prove opaque provider compliance or infer spatial containment from an input lifetime | not stated | 9 |
| FOREIGN_STORAGE52 | "Admission of a locator or ABI alone remains insufficient; an inert address remains inert." | Provider-owned views and completion | Locator or ABI admission alone must not establish references; inert addresses must stay inert | not stated | 9 |
| FOREIGN_STORAGE53 | "Ordinary checked code still has to establish its references." | Provider-owned views and completion | Checked code must still establish its own references beyond boundary admission | not stated | 2 |
| FOREIGN_STORAGE54 | "A view whose invalidators all require exclusive access to one receiver is an ordinary borrow from that receiver." | Provider-owned views and completion | Views with invalidators requiring exclusive access to one receiver must be classified as ordinary borrows from that receiver | not stated | 2 |
| FOREIGN_STORAGE55 | "More precise protocols use a linear validity claim that every invalidating operation consumes." | Provider-owned views and completion | Non-ordinary invalidation protocols must use a linear validity claim consumed by each invalidating operation | at the operation | 2 |
| FOREIGN_STORAGE56 | "Global, thread-local, or asynchronously invalidated storage must be copied, mediated by such a claim, or accepted under an admitted stability promise." | Provider-owned views and completion | Global, thread-local, or async-invalidated storage must be copied, mediated by a linear claim, or accepted under admitted stability promise | not stated | 2 |
| FOREIGN_STORAGE57 | "A claim cannot prevent an opaque provider from invalidating through an unmodeled route." | Provider-owned views and completion | Validity claims must not be treated as blocking opaque provider invalidation via unmodeled routes | not stated | 9 |
| FOREIGN_STORAGE58 | "Replacement ends the overwritten view's loans and installs the new view's exact provenance;" | Provider-owned views and completion | Replacement must end overwritten view loans and install the new view's exact provenance | at the operation | 2 |
| FOREIGN_STORAGE59 | "equal lifetime names do not preserve the old pointer or its input relationship." | Provider-owned views and completion | Equal lifetime names must not preserve old pointer or input relationship across replacement | not stated | 2 |
| FOREIGN_STORAGE60 | "Independently retained views keep their own loans." | Provider-owned views and completion | Independently retained views must keep separate loans | not stated | 2 |
| FOREIGN_STORAGE61 | "The admitted stability promise must cover the permitted use interval, including any relevant firmware invalidation." | Provider-owned views and completion | Admitted stability promise must cover the full permitted use interval including relevant invalidation | not stated | 2 |
| FOREIGN_STORAGE62 | "This establishes the declared view, not ownership of the executable image or authority over ranges described by fields inside the metadata." | Provider-owned views and completion | Establishment must grant the declared view only, not image ownership or range authority from metadata fields | not stated | 2 |
| FOREIGN_STORAGE63 | "Completion correlates one event with one live claim through a unique/nonreused identity, generation-checked identity, or exclusively ordered channel." | Provider-owned views and completion | Completion must correlate one event with one live claim via unique/nonreused id, generation-checked id, or exclusive channel | not stated | 3 |
| FOREIGN_STORAGE64 | "Progress releases nothing unless it returns an exact separated subclaim." | Provider-owned views and completion | Progress must not release claims unless it returns an exact separated subclaim | not stated | 2 |
| FOREIGN_STORAGE65 | "Cancellation releases nothing until terminal acknowledgement." | Provider-owned views and completion | Cancellation must not release until terminal acknowledgement | not stated | 3 |
| FOREIGN_STORAGE66 | "Tokens need generations when stale foreign copies may survive reuse." | Provider-owned views and completion | Tokens must use generations when stale foreign copies may survive reuse | not stated | 2 |
| FOREIGN_STORAGE67 | "[Boundary completion receipts](../terminal-psi/ownership.md#claim-paths) bind the exact operation, boundary, argument position, and complete consumed claim set." | Provider-owned views and completion | Boundary completion receipts must bind exact operation, boundary, argument position, and full consumed claim set | not stated | 2 |
| FOREIGN_STORAGE68 | "Interpretation and native realization join them to the admitted provider execution." | Provider-owned views and completion | Interpretation and native realization must join receipts to the admitted provider execution | not stated | 9 |
| FOREIGN_STORAGE69 | "Rejected provider completion commits no receipt or custody transfer;" | Provider-owned views and completion | Rejected provider completion must not commit receipt or custody transfer | not stated | 3 |
| FOREIGN_STORAGE70 | "that does not undo effects already performed by native code." | Provider-owned views and completion | Absence of receipt on rejected completion must not be modeled as undoing native effects already performed | not stated | 9 |
| FOREIGN_STORAGE71 | "A provider-created handle or pending operation pins the exact era whose state gives it meaning." | Provider-owned views and completion | Provider-created handles or pending operations must pin the exact era that gives them meaning | not stated | 9 |
| FOREIGN_STORAGE72 | "A replaceable provider slot names an installed route and does not itself pin an era." | Provider-owned views and completion | Replaceable provider slots must name an installed route and must not pin an era by themselves | not stated | 9 |
| FOREIGN_STORAGE73 | "Pins prevent reclamation, not teardown execution: the old era remains callable while discharging its roots, then waits for application-held claims, quiescence, and unload." | Provider-owned views and completion | Pins must prevent reclamation but not teardown execution; old era behavior must follow the stated discharge and wait sequence | not stated | 9 |
| FOREIGN_STORAGE74 | "Static custodians discharge outlives relationships at build time without runtime ledger entries." | Provider-owned views and completion | Static custodians must discharge outlives at build time with no runtime ledger entries | not stated | 9 |
| FOREIGN_STORAGE75 | "The ordinary [write-only rules](../terminal-psi/structural_access.md#write-only-authority) govern `&write T`: an existing valid referent, no observation or access widening, discardable displaced content, validity after replacement, and exact outcome write frames." | Write-only foreign access | `&write T` at foreign boundaries must satisfy the listed ordinary write-only rules | at the operation | 2 |
| FOREIGN_STORAGE76 | "Checked providers enforce these through their call closure; opaque compliance requires admitted evidence unless isolation enforces it." | Write-only foreign access | Checked providers must enforce write-only rules in call closure; opaque compliance needs admitted evidence unless isolation enforces | at a call | 9 |
| FOREIGN_STORAGE77 | "A byte-producing outcome names its exact modified prefix or other footprint;" | Write-only foreign access | Byte-producing outcomes must name exact modified prefix or footprint | at normal return | 2 |
| FOREIGN_STORAGE78 | "facts over the untouched complement survive." | Write-only foreign access | Facts over the untouched complement must survive byte-producing writes | at the operation | 2 |
| FOREIGN_STORAGE79 | "Its returned count cannot establish a value absent at entry." | Write-only foreign access | Returned byte count must not establish values absent at entry | at normal return | 2 |
| FOREIGN_STORAGE80 | "Write-only access transfers no durable custody." | Write-only foreign access | Write-only access must not transfer durable custody | at normal return | 2 |
| FOREIGN_STORAGE81 | "Identity-only retention is a stable keepalive claim lending no view." | Write-only foreign access | Identity-only retention must be a stable keepalive claim with no lent view | not stated | 2 |
| FOREIGN_STORAGE82 | "Typed construction into storage with no live `T` is a separate feature, not another interpretation of `&write`." | Write-only foreign access | Construction into storage without live `T` must not be treated as an alternate interpretation of `&write` | not stated | 2 |
| FOREIGN_STORAGE83 | "No compiler-side runtime ledger substitutes for the selected provider's actual retention and completion contract for lifetime-borrowed, moved and permitted-snapshot dispositions." | Write-only foreign access | Compiler must not use a runtime ledger in place of the provider's retention and completion contract for listed dispositions | not stated | 9 |

## Ambiguous sentences

| Sentence | Reading A | Reading B |
| --- | --- | --- |
| "Safe parameter/result contracts own foreign access and custody." | Boundary contracts exclusively determine access and custody; calling plans never participate. | Contracts own the semantic obligations; calling plans still participate in materialization while not owning permission/lifetime. |
| "Read-only foreign use may preserve semantic facts and permit Omega read views." | Read-only foreign use always preserves facts and permits Omega read views when contract allows. | Preservation and read views are optional outcomes permitted under some contracts, not guaranteed for all read-only foreign use. |
| "Consuming `Buffer` into `PendingWrite` can transfer its content;" | Any consuming mapping from `Buffer` to `PendingWrite` must transfer content. | Transfer is allowed only when the protocol's consuming path is taken; other consuming forms might differ. |
| "Infer unambiguous consumed-input-to-result mappings; an ambiguous mapping needs an ordinary postcondition establishing the exact correspondence." | The compiler must infer mappings automatically when unambiguous. | Inference is a specification convenience; whether the compiler or the author supplies the mapping when unambiguous is not stated. |
| "Unknown provenance rejects unless an admitted provider route establishes it." | Rejection is mandatory for unknown provenance with no provider route. | Unknown provenance might still compile if another mechanism not named here establishes provenance. |
| "Exclusive foreign mutation may instead move storage into the protocol and return it with preserved, invalidated, or outcome-dependent qualifications." | Exclusive foreign mutation must use move-into-protocol when concurrent External backing is insufficient. | Move-into-protocol is one permitted alternative among others not listed in this sentence. |
| "A consumable lifetime budget is distinct." | Live-occurrence capacity and consumable lifetime budget are separate checked quantities. | "Distinct" is descriptive only; whether both must be tracked or only one depends on disposition is not stated here. |
| "Admission of a complete, applied safe boundary contract is the trust point for provider-established references." | No provider reference facts exist before full contract admission. | Admission is where references become trusted for checking; other stages may carry provisional facts. |
| "It does not prove opaque provider compliance or infer spatial containment from an input's lifetime." | Compiler never proves opaque compliance or containment inference. | Compiler may defer those proofs to other evidence channels not described in this chapter. |
| "A claim cannot prevent an opaque provider from invalidating through an unmodeled route." | Programs must be rejected if they rely on claims to block unmodeled invalidation. | The sentence states a semantic limit only; whether the checker warns or rejects such programs is not stated. |
| "Progress releases nothing unless it returns an exact separated subclaim." | Partial progress never releases any part of a claim without a separated subclaim return. | "Progress" names a specific protocol event; other events might release under rules not stated here. |
| "Rejected provider completion commits no receipt or custody transfer;" | Rejection blocks both receipt emission and custody transfer. | Rejection blocks commitment to receipts; in-flight native custody might still change without a committed transfer. |
| "Pins prevent reclamation, not teardown execution:" | Pinning forbids reclaiming storage but allows teardown routines to run. | Pinning blocks only one kind of reclamation; other resource release paths may still run during teardown. |
| "opaque compliance requires admitted evidence unless isolation enforces it." | Opaque providers need admitted evidence for write-only rules when isolation does not enforce them. | Isolation is sufficient evidence by itself; whether "isolation" is a compile-time or runtime property is not stated. |
| "Identity-only retention is a stable keepalive claim lending no view." | Identity-only retention must not lend any lexical view. | Keepalive may still permit limited access through a different mechanism not counted as a "view." |

## Terms used without definition in this chapter

- safe parameter/result contracts
- foreign access and custody
- calling plans
- representation / permission / lifetime
- content conservation
- structural access
- native boundaries
- native support
- backing route / lifetime route
- `addr`
- `Ptr<T>`
- sealed / inert ABI carrier
- type parameter / pointee shape
- ordinary code
- binding (pointer materialization)
- established storage claims
- marshaling policies
- authorized establishment route
- checked views
- native pointer/count pairs
- descriptor graphs
- semantic extents
- native slice ABI
- ordinary reference case
- boundary shapes
- format codecs
- cast (stronger facts)
- call-scoped disposition
- retained after return
- process-lifetime disposition
- linear protocol result
- stable backing
- lifetime-parameterized result
- checked loan
- permitted semantic snapshot
- private stable backing
- static/process-lifetime root
- permanent-custodian spelling
- retained claim
- keepalive and reclamation authority
- lexical views
- semantic facts
- Omega read views
- writable extent
- terminal completion
- partial release
- subextent
- published results
- ordinary conservation
- `Buffer` / `PendingWrite`
- owned retention
- consumed-input-to-result mappings
- postcondition
- `Content<Owner, A>` projection
- foreign-only extent algebra
- retained native pointer slot
- stable-root
- revision/lease provenance
- admitted provider route
- embedded layouts
- finite structural closure
- arena/extent root
- pointer graphs
- private snapshots
- independent copying
- identity preservation
- unchecked write-back
- public result type
- requirements / realizations
- backing recipes
- External placed backing
- persistent demand
- live occurrence
- live-occurrence capacity
- registration / unregister
- consumable lifetime budget
- static thunk storage
- admitted callback identities
- callback registration
- code/component lease
- complete, applied safe boundary contract
- provider-established references
- reference result
- output slot
- valid backing
- declared access and stability
- permitted lifetime
- provider-owned storage
- provider-root annotation
- validity token
- admitted provider invocation
- provenance
- replacement and invalidation
- opaque provider compliance
- spatial containment
- locator
- inert address
- invalidators
- linear validity claim
- admitted stability promise
- unmodeled route
- firmware invalidation (example)
- loaded-image metadata (example)
- services table (example)
- independently retained views
- completion (protocol)
- unique/nonreused identity
- generation-checked identity
- exclusively ordered channel
- separated subclaim
- terminal acknowledgement
- tokens / generations
- stale foreign copies
- boundary completion receipts
- consumed claim set
- native realization
- provider completion
- provider-created handle
- pending operation
- era (provider)
- replaceable provider slot
- installed route
- pins
- teardown execution
- application-held claims
- quiescence
- unload
- static custodians
- outlives relationships
- runtime ledger
- write-only rules
- `&write T`
- valid referent
- access widening
- discardable displaced content
- outcome write frames
- call closure
- admitted evidence
- isolation
- byte-producing outcome
- modified prefix / footprint
- untouched complement
- returned count
- durable custody
- identity-only retention
- stable keepalive claim
- typed construction
- lifetime-borrowed dispositions
- moved dispositions
- permitted-snapshot dispositions
- component publication (linked chapter)
- private callbacks (linked chapter)

CHAPTER COMPLETE
