# First executable realization through ownership rationale

Source: `wiki/spec/packages/topology.md` lines 578–724 (read).

## Rules

| rule id | the exact sentence, quoted | the heading it is under | what must be checked, in plain words | when it must hold | lane |
| --- | --- | --- | --- | --- | --- |
| TOPOLOGY1 | "The first cross-binary customer uses three local processes connected by installer-created private pipes, not an unspecified cloud orchestrator." | First executable realization | The reference customer must use three local processes and installer-created private pipes, not an unspecified cloud orchestrator | not stated | 9 |
| TOPOLOGY2 | "The same logical policy must work on Windows and macOS through target-owned adapters:" | First executable realization | The same logical policy must be realizable on Windows and macOS via target-owned adapters | not stated | 9 |
| TOPOLOGY3 | "anonymous pipe handles on Windows and private pipe descriptors on macOS." | First executable realization | Windows must use anonymous pipe handles; macOS must use private pipe descriptors for the adapters | not stated | 9 |
| TOPOLOGY4 | "The installer supplies only the explicitly assigned endpoints to each process and closes its unused duplicates." | First executable realization | Each process must receive only explicitly assigned endpoints; unused duplicates must be closed | not stated | 9 |
| TOPOLOGY5 | "General handle inheritance is disabled." | First executable realization | General handle inheritance must be disabled | not stated | 9 |
| TOPOLOGY6 | "Use one dedicated request/response pipe pair per binding and one outstanding request per binding." | First executable realization | Each binding must have one dedicated request/response pipe pair and at most one outstanding request | not stated | 9 |
| TOPOLOGY7 | "No broker, global endpoint lookup, DNS, open listener, connection pooling, automatic retry, or callback transfer is needed for this acceptance." | First executable realization | This acceptance profile must not depend on broker, global lookup, DNS, open listener, pooling, automatic retry, or callback transfer | not stated | 9 |
| TOPOLOGY8 | "Replies stay on the binding's response channel." | First executable realization | Replies must use only the binding's response channel | at a call | 3 |
| TOPOLOGY9 | "The selected codec uses a bounded frame and exact operation schema; invalid length, operation, payload, premature EOF, or peer failure produces an explicit protocol/transport failure and closes that binding." | First executable realization | Codec frames must be bounded with an exact operation schema; listed invalid conditions must yield explicit protocol/transport failure and close the binding | at a call | 9 |
| TOPOLOGY10 | "No reconnect to a different peer is attempted." | First executable realization | After binding failure, reconnection to a different peer must not be attempted | not stated | 9 |
| TOPOLOGY11 | "The three component programs have checked code and only these scoped transport imports." | First executable realization | Component programs must be checked and must import only the scoped transport surface allowed for this profile | not stated | SHARED |
| TOPOLOGY12 | "The trusted installation/OS contract establishes process/executable identity, private-handle mapping, and startup admission." | First executable realization | Installation/OS contract must establish process or executable identity, private-handle mapping, and startup admission | at entry | 9 |
| TOPOLOGY13 | "Target pipe primitives and any unverified loader behavior are named provider assumptions." | First executable realization | Pipe primitives and unverified loader behavior must be recorded as named provider assumptions, not silent facts | not stated | 9 |
| TOPOLOGY14 | "This profile does not claim to confine arbitrary malicious native code that can invoke host syscalls outside the checked closure; such code requires separately established OS sandboxing or rejects." | First executable realization | Malicious native code outside checked closure must not be claimed confined; such code must rely on separate OS sandboxing or be rejected | not stated | 9 |
| TOPOLOGY15 | "A broad assumption that \"the application obeys the diagram\" is not acceptable in place of the checked closure." | First executable realization | Verification must not substitute a diagram-obedience assumption for checked closure | not stated | 8 |
| TOPOLOGY16 | "The installer first validates all products and allocates the endpoints." | First executable realization | Installer must validate all products before allocating endpoints | not stated | 9 |
| TOPOLOGY17 | "Application entry is held behind the runtime's installation gate until the exact mappings and authority are admitted." | First executable realization | Application entry must remain blocked until exact mappings and authority are admitted | at entry | 9 |
| TOPOLOGY18 | "Failed setup closes resources and leaves no admitted new application activation." | First executable realization | Failed setup must close resources and must not admit new application activation | not stated | 9 |
| TOPOLOGY19 | "Tests must attempt an ungranted endpoint invocation and substituted pipe mapping, not merely draw the intended graph." | First executable realization | Acceptance tests must exercise ungranted endpoint invocation and substituted pipe mapping, not only graph structure | not stated | 9 |
| TOPOLOGY20 | "pipe/codec adapters are library/provider code; the compiler does not acquire API/Auth/Billing concepts." | First executable realization | Pipe and codec adapters must stay in library or provider code; the compiler must not acquire API, Auth, or Billing as concepts | not stated | SHARED |
| TOPOLOGY21 | "Remote encrypted transport is a different selected realization, requiring authenticated peer/binding identity and the same authority constraints." | First executable realization | Remote encrypted transport must be a separately selected realization with authenticated peer or binding identity and the same authority constraints | not stated | 9 |
| TOPOLOGY22 | "Local-pipe evidence does not establish remote-transport enforcement." | First executable realization | Local-pipe evidence must not be treated as establishing remote-transport enforcement | not stated | 8 |
| TOPOLOGY23 | "Errors identify instance, endpoint, component subject, policy, and the relevant authored binding/source span where available." | Diagnostics and implementation acceptance | Diagnostics must name instance, endpoint, component subject, policy, and binding or source span when available | not stated | SHARED |
| TOPOLOGY24 | "A bypass reports a concrete path, including cross-binary bindings." | Diagnostics and implementation acceptance | Bypass diagnostics must report a concrete path, including cross-binary bindings | not stated | 8 |
| TOPOLOGY25 | "The reference checker chooses a shortest witness with canonical-key tie breaking; it checks all policies in canonical order." | Diagnostics and implementation acceptance | The reference checker must pick a shortest witness with canonical-key tie-breaking and must check policies in canonical order | not stated | 8 |
| TOPOLOGY26 | "Unavailable/unknown confinement reports the unaccounted authority, not a fabricated path." | Diagnostics and implementation acceptance | Unavailable or unknown confinement must report unaccounted authority, not a fabricated path | not stated | 8 |
| TOPOLOGY27 | "Internal contradiction remains distinct from invalid composition or an unsupported provider/profile." | Diagnostics and implementation acceptance | Internal contradiction must be classified separately from invalid composition or unsupported provider or profile | not stated | 8 |
| TOPOLOGY28 | "This contract adds no arbitrary compiler reason codes." | Diagnostics and implementation acceptance | The contract must not introduce arbitrary new compiler reason codes | not stated | SHARED |
| TOPOLOGY29 | "Composition succeeds; installation still requires enforcement evidence." | Diagnostics and implementation acceptance | For API → Authorization → Billing with all imports accounted, composition must succeed and installation must still require enforcement evidence | not stated | SHARED |
| TOPOLOGY30 | "`only_via` rejects with the bypass path." | Diagnostics and implementation acceptance | When API → Billing bypass is added, `only_via` must reject and report the bypass path | not stated | 8 |
| TOPOLOGY31 | "Reject; every component import participates, not only request handlers." | Diagnostics and implementation acceptance | Indirect bypass via logging or helper service must be rejected with every component import participating in the check | not stated | 3 |
| TOPOLOGY32 | "Distinguish instances; do not merge by code or trait identity." | Diagnostics and implementation acceptance | Duplicate package instances must be distinguished; merging by code or trait identity must not occur | not stated | 7 |
| TOPOLOGY33 | "No edge invented; an unbound demanded import rejects." | Diagnostics and implementation acceptance | Must not invent edges between compatible unconnected endpoints; unbound demanded imports must reject | not stated | 3 |
| TOPOLOGY34 | "Reject the misleading `only_via` policy input/result." | Diagnostics and implementation acceptance | Disconnected S/T, empty selector, or overlapping via set must cause `only_via` input or result to be rejected as misleading | not stated | 8 |
| TOPOLOGY35 | "Terminating check; reject a path avoiding the required intermediary." | Diagnostics and implementation acceptance | Cycle and indirect-avoidance analysis must terminate and must reject paths that avoid a required intermediary | not stated | 4 |
| TOPOLOGY36 | "Reject without exact mediation/confinement evidence." | Diagnostics and implementation acceptance | Broad transport hidden behind an ordinary wrapper must be rejected unless exact mediation or confinement evidence is present | not stated | 8 |
| TOPOLOGY37 | "Reject; no implied delegation or reverse connection." | Diagnostics and implementation acceptance | Raw endpoint fabrication, serialized endpoint, or unlisted callback must be rejected without implied delegation or reverse connection | not stated | 3 |
| TOPOLOGY38 | "Reject stale evidence; reconstruct against the new subject." | Diagnostics and implementation acceptance | Changes after check to component, policy, binding, codec, or transport must reject stale evidence and require reconstruction against the new subject | not stated | 8 |
| TOPOLOGY39 | "Reject; only completed, verified closure facts satisfy this role." | Diagnostics and implementation acceptance | Early product schema or producer-written complete flag must be rejected; only completed verified closure facts may satisfy verified-component role | not stated | 8 |
| TOPOLOGY40 | "Installer rejects the mismatch with independently accepted owner intent." | Diagnostics and implementation acceptance | Omitted owner-required policy or trivial substitute must cause installer rejection against independently accepted owner intent | not stated | 9 |
| TOPOLOGY41 | "Reject; authenticated history is not current permission." | Diagnostics and implementation acceptance | Correctly signed old request differing from current installation authorization must be rejected | not stated | 8 |
| TOPOLOGY42 | "Reject without executing it." | Diagnostics and implementation acceptance | Plan naming an unselected policy executable must be rejected without executing that executable | not stated | 9 |
| TOPOLOGY43 | "Reject the incomplete/non-bijective installed coverage." | Diagnostics and implementation acceptance | Valid individual binding receipts with omitted or duplicated imports must reject for incomplete or non-bijective installed coverage | not stated | 3 |
| TOPOLOGY44 | "No authority from the topology dependency; scoped build execution refuses." | Diagnostics and implementation acceptance | Build helper attempts to read home files or open network must have no topology-derived authority; scoped build execution must refuse | not stated | 9 |
| TOPOLOGY45 | "Publication is not approval; the package verifier rejects invalid evidence." | Diagnostics and implementation acceptance | Forged success flag in published plan must not count as approval; package verifier must reject invalid evidence | not stated | 8 |
| TOPOLOGY46 | "Same normalized semantic graph and policy result." | Diagnostics and implementation acceptance | Changed instance or binding declaration order must yield the same normalized semantic graph and policy result | not stated | 8 |
| TOPOLOGY47 | "Reconstruct and check from admitted artifacts and owned plan." | Diagnostics and implementation acceptance | Cross-machine consumer without source trees must reconstruct and check from admitted artifacts and owned plan | not stated | 9 |
| TOPOLOGY48 | "Installation rejects even if the proposed graph passes." | Diagnostics and implementation acceptance | Wrong runtime peer, substituted native code, or insufficient confinement must cause installation rejection even when the graph check passes | not stated | 9 |
| TOPOLOGY49 | "No application activation; release or return pending custody, no false success receipt." | Diagnostics and implementation acceptance | When plan check succeeds but preparation fails before activation, must not activate application; must release or return pending custody without false success receipt | not stated | 9 |
| TOPOLOGY50 | "No successful installation receipt; retain supervision and quiesce or report explicit cleanup failure." | Diagnostics and implementation acceptance | Partial start with later activation failure must yield no successful installation receipt; must retain supervision and quiesce or report explicit cleanup failure | not stated | 9 |
| TOPOLOGY51 | "Same logical policy, separately justified enforcement/transport contracts." | Diagnostics and implementation acceptance | Co-located, isolated, and remote realizations must share logical policy but each needs separately justified enforcement and transport contracts | not stated | 9 |
| TOPOLOGY52 | "Authorization protocol rejects independently of a passing topology check." | Diagnostics and implementation acceptance | Charge amount mismatch or replayed request must be rejected by authorization protocol independent of topology check outcome | not stated | 3 |
| TOPOLOGY53 | "Not claimed safe by topology; no false confidentiality verdict." | Diagnostics and implementation acceptance | Secret-dependent reply on a permitted route must not receive a topology false-safety or confidentiality verdict | not stated | 8 |
| TOPOLOGY54 | "Test on supported developer hosts; mark remote/OS enforcement tests unavailable when their providers are absent, not passed by a graph-only test." | Diagnostics and implementation acceptance | Remote or OS enforcement tests without providers must be marked unavailable, not passed via graph-only testing | not stated | 9 |
| TOPOLOGY55 | "The first end-to-end customer is the three-component payment example, including an actual blocked bypass at the selected runtime boundary." | Diagnostics and implementation acceptance | First end-to-end acceptance must include the three-component payment example with an actual blocked bypass at the runtime boundary | not stated | 9 |
| TOPOLOGY56 | "A graph helper test alone does not close it." | Diagnostics and implementation acceptance | End-to-end acceptance must not be closed by a graph helper test alone | not stated | 8 |
| TOPOLOGY57 | "No graph, canonical-wire, substitution, policy-selection, installation or confinement row is closed until an authored Omega customer exercises the corresponding contract and invalid control." | Diagnostics and implementation acceptance | Each acceptance row for graph, wire, substitution, policy-selection, installation, or confinement must be closed only by an authored Omega customer exercising contract and invalid control | not stated | SHARED |
| TOPOLOGY58 | "Application-level authorization cases (charge-amount mismatch, request replay) live outside the topology package." | Diagnostics and implementation acceptance | Charge-amount mismatch and request replay must be handled outside topology package scope | not stated | SHARED |
| TOPOLOGY59 | "Insufficient for instance-specific enforced wiring." | Alternatives and scope settlement | Package discipline and manual reach audits alone must be treated as insufficient for instance-specific enforced wiring | not stated | 8 |
| TOPOLOGY60 | "Valid when the permitted construction itself expresses the complete restriction." | Alternatives and scope settlement | Capability construction without graph policies is acceptable only when the construction itself expresses the complete restriction | not stated | 8 |
| TOPOLOGY61 | "Graph policies earn their place when independent wiring edits must preserve a global rule." | Alternatives and scope settlement | Graph policies apply when independent wiring edits must preserve a global rule | not stated | 8 |
| TOPOLOGY62 | "Not justified: use scoped build execution's generic build outputs and package code." | Alternatives and scope settlement | Compiler-owned topology products, keywords, and graph algorithms must not be introduced; use scoped build generic outputs and package code | not stated | SHARED |
| TOPOLOGY63 | "Component evidence remains owned by existing component verification." | Alternatives and scope settlement | Component evidence must stay with existing component verification, not compiler topology hooks | not stated | 8 |
| TOPOLOGY64 | "Optional convenience for authorized schemas, not evidence of complete executable authority or installed wiring." | Alternatives and scope settlement | Reflection-driven graph discovery must not count as evidence of complete executable authority or installed wiring | not stated | 8 |
| TOPOLOGY65 | "not required to prove this bounded claim." | Alternatives and scope settlement | Precise per-entry internal dependency summaries must not be required to prove this bounded claim | not stated | 8 |
| TOPOLOGY66 | "not the first implementation." | Alternatives and scope settlement | Arbitrary program or network topology extraction must not be treated as the first implementation scope | not stated | SHARED |
| TOPOLOGY67 | "Wrong abstraction: service-set summaries have discarded paths and instance identities." | Alternatives and scope settlement | Must not rely on `reaches A through B` or Boolean service rows as the routing abstraction | not stated | 8 |
| TOPOLOGY68 | "Unsound: a correct graph algorithm can prove a false model." | Alternatives and scope settlement | Library-authored endpoint inventories or success receipts must not be trusted as evidence | not stated | 8 |
| TOPOLOGY69 | "Confuses compatibility with installed connectivity." | Alternatives and scope settlement | Same trait or schema must not be treated as an automatic installed network route | not stated | 3 |
| TOPOLOGY70 | "Violates authority, evaluation, and installation separation." | Alternatives and scope settlement | Transport credentials or mutable deployment state in compile-time constants must be rejected as violating authority, evaluation, and installation separation | not stated | 9 |
| TOPOLOGY71 | "Keep the finite graph and the two routing policies as the first package slice." | Implementation and amendments | First package slice must retain only a finite graph and the two routing policies | not stated | 8 |
| TOPOLOGY72 | "Do not expand into distributed orchestration, automatic discovery, hot replacement, ordered-query languages, or information-flow analysis to justify compiler hooks." | Implementation and amendments | Compiler hooks must not be justified by expanding into distributed orchestration, discovery, hot replacement, ordered-query languages, or information-flow analysis | not stated | SHARED |
| TOPOLOGY73 | "This contract requires independently supplied owner intent, a generic verified-component description, ordinary predicate replay, and a staged preparation/activation protocol." | Implementation and amendments | Plan checking must require independently supplied owner intent, generic verified-component description, ordinary predicate replay, and staged preparation or activation protocol | not stated | SHARED |
| TOPOLOGY74 | "Any missing completeness guarantee needs a precise contract and implementation task in that owner, not an assumed adapter." | Implementation and amendments | Missing component-fact completeness must not be bridged by an assumed adapter; must get a precise contract and task in the owning subsystem | not stated | SHARED |
| TOPOLOGY75 | "Specify the package codec field/tag tables and test corrupt/missing/duplicate/stale fields before shipping artifacts." | Implementation and amendments | Package codec field and tag tables must be specified and corrupt, missing, duplicate, and stale fields must be tested before shipping artifacts | not stated | 9 |
| TOPOLOGY76 | "Demonstrate the generic build output protocol with a code generator as well as topology." | Implementation and amendments | Generic build output protocol must be demonstrated with a code generator in addition to topology | not stated | 9 |
| TOPOLOGY77 | "Exercise an actual private-pipe installation and its failures on the available hosts." | Implementation and amendments | Acceptance must exercise an actual private-pipe installation and its failures on available hosts | not stated | 9 |
| TOPOLOGY78 | "These are implementation acceptance artifacts, not permission to claim runtime enforcement from successful graph tests." | Implementation and amendments | Listed implementation artifacts must not authorize claiming runtime enforcement from successful graph tests alone | not stated | 8 |
| TOPOLOGY79 | "Topology-specific wire choices, container selection, and graph implementation stay with the package." | Implementation and amendments | Wire choices, container selection, and graph implementation must remain package-owned, not compiler-owned | not stated | SHARED |
| TOPOLOGY80 | "Any new trusted premise or semantic amendment, whether found during implementation or later review, must be raised through [owner questions](../../../OWNER_QUESTIONS.md) before relying on it." | Implementation and amendments | New trusted premises or semantic amendments must be raised in OWNER_QUESTIONS before reliance | not stated | SHARED |
| TOPOLOGY81 | "Acceptance of this contract does not pre-authorize such changes." | Implementation and amendments | Contract acceptance must not pre-authorize undocumented semantic changes | not stated | SHARED |
| TOPOLOGY82 | "Remote transport, automatic upgrades, arbitrary policy-code loading, and typed reflection across build/product contexts remain deferred, not unspecified v1 features." | Implementation and amendments | Listed features remain explicitly deferred for v1, not silently unspecified | not stated | SHARED |
| TOPOLOGY83 | "Scoped build execution is an implementation dependency; graph-only tests do not discharge it." | Implementation and amendments | Scoped build execution dependency must not be treated as discharged by graph-only tests | not stated | 9 |
| TOPOLOGY84 | "This contract does not import capDL's kernel objects, seL4 authority model, or proof results into Omega or host processes." | Comparison and ownership rationale | capDL kernel objects, seL4 authority model, and proof results must not be imported into Omega or host processes | not stated | SHARED |
| TOPOLOGY85 | "Private pipes on a general OS need their own accounted confinement contract." | Comparison and ownership rationale | Private pipes on a general OS must have an accounted confinement contract separate from capDL | not stated | 9 |
| TOPOLOGY86 | "Compiler-owned graph analysis would duplicate ordinary finite algorithms and freeze a package policy into the language." | Comparison and ownership rationale | Compiler-owned graph analysis must not be introduced where it duplicates finite algorithms and freezes package policy in the language | not stated | SHARED |
| TOPOLOGY87 | "Conversely, a library-only claim of complete executable authority has no basis merely because its graph algorithm is correct." | Comparison and ownership rationale | A correct library graph algorithm alone must not support a claim of complete executable authority | not stated | 8 |
| TOPOLOGY88 | "The minimum split is compiler/component verification of complete facts, package computation over those facts, and provider-established actual bindings." | Comparison and ownership rationale | Responsibilities must split among compiler or component verification of complete facts, package computation, and provider-established bindings | not stated | SHARED |
| TOPOLOGY89 | "Ownership qualifications and explicit admissions carry these joins; no special `TopologySafe` keyword or runtime registry is introduced." | Comparison and ownership rationale | Joins must use ownership qualifications and explicit admissions; must not introduce `TopologySafe` keyword or runtime registry | not stated | SHARED |

## Ambiguous sentences

| Sentence | Reading A | Reading B |
| --- | --- | --- |
| "Use one dedicated request/response pipe pair per binding and one outstanding request per binding." | At most one in-flight request per binding at any time. | One outstanding request is required for acceptance but multiple sequential requests per binding remain allowed without further ordering rules stated here. |
| "No broker, global endpoint lookup, DNS, open listener, connection pooling, automatic retry, or callback transfer is needed for this acceptance." | The first acceptance profile must omit these mechanisms entirely. | These mechanisms may exist elsewhere in the system but are not required to satisfy this acceptance profile. |
| "This profile does not claim to confine arbitrary malicious native code that can invoke host syscalls outside the checked closure; such code requires separately established OS sandboxing or rejects." | Programs with unchecked syscalls must be rejected unless separate OS sandboxing is established. | Such programs may run but topology offers no confinement claim; rejection is optional if sandboxing is absent. |
| "Composition succeeds; installation still requires enforcement evidence." | Graph or policy composition may succeed at build or check time while installation must still obtain separate enforcement evidence. | "Composition succeeds" includes installation-time composition; "still requires" only adds evidence beyond graph checking. |
| "Terminating check; reject a path avoiding the required intermediary." | The checker must use an algorithm guaranteed to terminate on all inputs. | "Terminating" names a policy about required intermediaries in terminating protocols, not graph algorithm termination. |
| "Reject without exact mediation/confinement evidence." | Lack of exact evidence is sufficient for rejection. | Rejection is required only when wrapper transport is broad; narrow wrappers might pass with weaker evidence not described here. |
| "Reject stale evidence; reconstruct against the new subject." | Any post-check change forces full reconstruction before acceptance. | Stale evidence is rejected only for listed subjects (component, policy, binding, codec, transport); other changes are not covered in this row. |
| "Installer rejects the mismatch with independently accepted owner intent." | Owner intent must be accepted through a channel independent of the plan writer. | Owner intent may be carried in the plan if it was independently accepted earlier; "independently" modifies acceptance timing only. |
| "Same normalized semantic graph and policy result." | Order changes must not affect normalized graph or policy outcomes used for acceptance. | Normalization is defined only outside this range; whether declaration order affects diagnostics but not policy is not stated here. |
| "Publication is not approval; the package verifier rejects invalid evidence." | Published bytes never imply verifier approval without verifier pass. | Publication may trigger verification; forged flags fail verification but other publication paths might still gate installation separately. |
| "No application activation; release or return pending custody, no false success receipt." | Preparation failure must never emit a success receipt or activate entry. | Pending custody may still record partial progress that resembles success in non-receipt channels. |
| "Test on supported developer hosts; mark remote/OS enforcement tests unavailable when their providers are absent, not passed by a graph-only test." | Absent providers yield explicit unavailable status in test reports. | Graph-only tests may run but must not be counted as passing remote or OS enforcement cases. |
| "A graph helper test alone does not close it." | End-to-end closure requires more than graph helper tests. | Graph helper tests may close individual rows but not the full end-to-end customer named in the previous sentence. |
| "Valid when the permitted construction itself expresses the complete restriction." | Graph policies are unnecessary if capability construction is complete. | "Valid" is scope guidance for authors; the checker might still require graph policies unless construction is proven complete by another owner. |
| "Graph policies earn their place when independent wiring edits must preserve a global rule." | Graph policies are required whenever independent wiring edits can break a global rule. | Graph policies are optional convenience even when global rules exist; "earn their place" is descriptive not mandatory. |
| "Optional convenience for authorized schemas, not evidence of complete executable authority or installed wiring." | Reflection may be used but never satisfies authority or wiring evidence obligations. | Reflection may supplement evidence when combined with other facts not named in this sentence. |
| "These are implementation acceptance artifacts, not permission to claim runtime enforcement from successful graph tests." | Passing graph tests never licenses runtime enforcement claims without listed artifacts. | Artifacts are necessary but the sentence does not say whether graph tests are necessary in addition. |
| "Scoped build execution is an implementation dependency; graph-only tests do not discharge it." | Scoped build execution must be validated separately from graph testing. | Graph tests may partially satisfy preparation but cannot replace scoped build execution entirely at installation. |
| "The minimum split is compiler/component verification of complete facts, package computation over those facts, and provider-established actual bindings." | All three parts are required for a sound topology story. | "Minimum" describes ownership boundaries only; some deployments might omit package computation if no graph is used. |
| "Ownership qualifications and explicit admissions carry these joins; no special `TopologySafe` keyword or runtime registry is introduced." | Joins must use only qualifications and admissions already in the contract. | Future keywords or registries could be added through amendments despite this sentence's intent for v1. |

## Terms used without definition in this chapter

- cross-binary customer
- installer-created private pipes
- cloud orchestrator
- logical policy
- target-owned adapters
- anonymous pipe handles
- private pipe descriptors
- explicitly assigned endpoints
- unused duplicates
- general handle inheritance
- dedicated request/response pipe pair
- binding
- outstanding request
- broker
- global endpoint lookup
- connection pooling
- callback transfer
- response channel
- bounded frame
- exact operation schema
- protocol/transport failure
- scoped transport imports
- trusted installation/OS contract
- private-handle mapping
- startup admission
- provider assumptions
- checked closure
- installation gate
- pipe/codec adapters
- API/Auth/Billing concepts
- remote encrypted transport
- authenticated peer/binding identity
- authority constraints
- local-pipe evidence
- remote-transport enforcement
- component subject
- authored binding/source span
- bypass path
- cross-binary bindings
- reference checker
- shortest witness
- canonical-key tie breaking
- canonical order
- unaccounted authority
- internal contradiction
- invalid composition
- unsupported provider/profile
- compiler reason codes
- composition
- enforcement evidence
- `only_via`
- component import
- request handlers
- trait identity
- demanded import
- misleading `only_via` policy input/result
- terminating check
- required intermediary
- mediation/confinement evidence
- raw endpoint fabrication
- serialized endpoint
- unlisted callback
- implied delegation
- reverse connection
- stale evidence
- verified closure facts
- owner-required policy
- trivial substitute
- independently accepted owner intent
- installation authorization
- unselected policy executable
- binding receipt
- non-bijective installed coverage
- topology dependency
- scoped build execution
- forged success flag
- package verifier
- normalized semantic graph
- admitted artifacts
- owned plan
- runtime peer
- substituted native code
- insufficient confinement
- pending custody
- false success receipt
- installation receipt
- supervision
- quiesce
- cleanup failure
- co-located vs isolated vs remote realization
- graph helper test
- canonical-wire
- substitution
- policy-selection
- confinement row
- invalid control
- package discipline
- manual reach audits
- instance-specific enforced wiring
- capability construction
- graph policies
- global rule
- compiler-owned topology products
- generic build outputs
- reflection-driven graph discovery
- complete executable authority
- installed wiring
- per-entry internal dependency summaries
- conservative instance graphs
- arbitrary program/network topology extraction
- construction-controlled customer
- `reaches A through B`
- Boolean service rows
- service-set summaries
- library-authored endpoint inventories
- success receipts
- transport credentials
- mutable deployment state
- compile-time constants
- finite graph
- two routing policies
- verified-component description
- ordinary predicate replay
- staged preparation/activation protocol
- portable producer/verifier support
- completeness guarantee
- package codec field/tag tables
- generic build output protocol
- implementation acceptance artifacts
- semantic amendment
- arbitrary policy-code loading
- typed reflection
- build/product contexts
- capDL
- seL4 authority model
- accounted confinement contract
- complete executable authority
- provider-established actual bindings
- ownership qualifications
- explicit admissions
- `TopologySafe`
- runtime registry

CHAPTER COMPLETE
