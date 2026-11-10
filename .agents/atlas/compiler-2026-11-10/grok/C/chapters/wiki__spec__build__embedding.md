# Embedding and interpreted components

Source: `wiki/spec/build/embedding.md` lines 1–453 (read).

## Rules

| rule id | the exact sentence, quoted | the heading it is under | what must be checked, in plain words | when it must hold | lane |
| --- | --- | --- | --- | --- | --- |
| EMBEDDING1 | "Loading Psi does not run `build.omg`, fetch packages, compile source, or obtain filesystem or installation authority." | Products and composition | Loading Psi must not trigger build, package fetch, source compile, or filesystem or installation authority acquisition | not stated | 9 |
| EMBEDDING2 | "Build permissions do not become runtime bindings." | Products and composition | Build-granted permissions must not be treated as automatic runtime bindings | not stated | 9 |
| EMBEDDING3 | "An independently replaceable component needs the checked closure and stable slots of component publication." | Products and composition | Independent replaceability requires checked closure and stable publication slots | not stated | 9 |
| EMBEDDING4 | "Building a selected component uses its required compilation context;" | Products and composition | Building a selected component must use that component's required compilation context only | not stated | 9 |
| EMBEDDING5 | "A native plugin product still needs its selected ABI/container; a Psi artifact is not a DLL." | Products and composition | Native plugins must have selected ABI or container; Psi artifacts must not be treated as DLLs | not stated | 9 |
| EMBEDDING6 | "Inspection checks canonical form and structural validity under shared loading limits." | Loading and preparation | Inspection must verify canonical form and structural validity within shared loading limits | not stated | SHARED |
| EMBEDDING7 | "It requires no execution trust and performs no execution-only descriptor or entailment analysis." | Loading and preparation | Inspection must not require execution trust and must skip execution-only descriptor or entailment analysis | not stated | SHARED |
| EMBEDDING8 | "Execution admission requires the host's authorization for the exact artifact and producer." | Loading and preparation | Execution admission must require host authorization for the exact artifact and producer | at entry | 9 |
| EMBEDDING9 | "Preparation resolves required execution metadata and installation relationships before creating an executable instance." | Loading and preparation | Preparation must resolve execution metadata and installation relationships before instance creation | at entry | 9 |
| EMBEDDING10 | "Untrusted execution is unavailable until the interpreter, adapters and teardown meet the containment acceptance in the implementation plan." | Loading and preparation | Untrusted execution must remain unavailable until containment acceptance is met | at entry | 9 |
| EMBEDDING11 | "This is a separation of codec and preparation responsibilities, not an option to skip structural checks." | Loading and preparation | Structural checks must not be skippable despite codec and preparation separation | not stated | SHARED |
| EMBEDDING12 | "Public byte-loading and module-start APIs enforce it as well as CLI entry points." | Loading and preparation | Byte-loading, module-start, and CLI entry points must enforce inspection and admission separation | not stated | SHARED |
| EMBEDDING13 | "A decoded object or an artifact-supplied producer name cannot manufacture that authorization." | Loading and preparation | Decoded objects and artifact producer names must not create execution authorization | not stated | 9 |
| EMBEDDING14 | "Native realization requires trusted input." | Loading and preparation | Native realization must reject or refuse untrusted input | not stated | 9 |
| EMBEDDING15 | "Shared external loaders enforce the 64 MiB input cap, bounded counts before allocation, nesting and string limits, and an overall work budget." | Loading and preparation | External loaders must enforce size cap, bounded counts, nesting and string limits, and total work budget | not stated | 6 |
| EMBEDDING16 | "Per-query limits do not replace the total budget." | Loading and preparation | Per-query limits must not excuse exceeding the overall work budget | not stated | SHARED |
| EMBEDDING17 | "Exhaustion reports resource refusal separately from malformed input or a failed semantic check." | Loading and preparation | Budget exhaustion must be reported as resource refusal distinct from malformation or semantic failure | not stated | SHARED |
| EMBEDDING18 | "Changed context requires rebuilding affected plans." | Loading and preparation | Context changes must trigger rebuild of affected preparation plans | not stated | 9 |
| EMBEDDING19 | "Bind plans to the artifact, operation, type, path, domains and qualifications, plus any selected layout or installation context they depend on." | Loading and preparation | Plans must be bound to artifact, operation, type, path, domains, qualifications, and dependent layout or installation context | not stated | 9 |
| EMBEDDING20 | "Repeated operation dispatch performs no proof search and no linear scans over whole declaration tables." | Loading and preparation | Operation dispatch must not perform proof search or full-table linear scans | at the operation | 8 |
| EMBEDDING21 | "Closed checks against an actual stored value remain on their defined paths, preserving the interpreter's independent execution oracle." | Loading and preparation | Closed checks on stored values must stay on their defined paths | at the operation | SHARED |
| EMBEDDING22 | "Unsupported or exhausted checks refuse or fault under their defined outcome; they never trigger broader proof search." | Loading and preparation | Unsupported or exhausted checks must refuse or fault without triggering broader proof search | at the operation | 8 |
| EMBEDDING23 | "One interpreter applies these operation checks to every admitted execution." | Loading and preparation | Every admitted execution must receive the same operation checks | at the operation | SHARED |
| EMBEDDING24 | "Producer trust changes admission and permission to optimize or lower; it does not select a dispatch mode that skips concrete safety checks." | Loading and preparation | Producer trust must not enable dispatch that skips concrete safety checks | at the operation | SHARED |
| EMBEDDING25 | "An interpreter may execute Linux-targeted code when its admitted host environment supplies the required Linux contracts and supported instruction semantics." | Target environments and execution engines | Linux-targeted interpreted execution requires admitted host environment with required Linux contracts and supported instruction semantics | at entry | 9 |
| EMBEDDING26 | "An exact Linux syscall binding must preserve its selected Linux contract, representation and foreign identity." | Target environments and execution engines | Exact Linux syscall bindings must preserve selected Linux contract, representation, and foreign identity | not stated | 9 |
| EMBEDDING27 | "A convenient host operation is not a silent substitute." | Target environments and execution engines | Host convenience operations must not silently substitute for selected bindings | not stated | 9 |
| EMBEDDING28 | "Native/interpreter comparison retains those exact selections rather than replacing them with mocks." | Target environments and execution engines | Native versus interpreter comparison must keep exact selections, not mocks | not stated | 9 |
| EMBEDDING29 | "Ordinary entry execution establishes every demanded provider slot in the selected executable closure before execution, including possible dynamic calls." | Target environments and execution engines | Before entry, every demanded provider slot in the executable closure must be established, including for possible dynamic calls | at entry | 9 |
| EMBEDDING30 | "A branch merely expected not to run cannot excuse missing supply;" | Target environments and execution engines | Expected-unreachable branches must not excuse missing provider supply | at entry | 9 |
| EMBEDDING31 | "Missing adapter or target-environment support prevents that interpretation and names the missing requirement and its demand." | Target environments and execution engines | Missing adapter or environment support must prevent interpretation and report missing requirement and demand | at entry | 9 |
| EMBEDDING32 | "Do not run native code as an implicit fallback, replace selected services, or increase authority." | Target environments and execution engines | Interpretation path must not implicitly run native code, replace selected services, or increase authority | at entry | 9 |
| EMBEDDING33 | "Calling the pending slot rejects;" | Target environments and execution engines | Calls to a pending provider-readiness slot must reject | at a call | 3 |
| EMBEDDING34 | "ordinary work-budget exhaustion does not authorize installation," | Target environments and execution engines | Work-budget exhaustion must not authorize component installation | not stated | 9 |
| EMBEDDING35 | "installation cannot replace an existing binding." | Target environments and execution engines | Provider installation must not replace an existing binding | not stated | 9 |
| EMBEDDING36 | "The component's whole callable closure is admitted before it executes." | Target environments and execution engines | Whole callable closure of a component must be admitted before execution | at entry | 3 |
| EMBEDDING37 | "Reservation is not supply or a host-adapter permission." | Target environments and execution engines | Provider slot reservation must not count as supply or host-adapter permission | not stated | 9 |
| EMBEDDING38 | "`--both` remains a comparison request, not an engine name, and is mutually exclusive with an explicit `--engine` selection." | Standalone execution selection | `--both` must be mutually exclusive with explicit `--engine` and must not be treated as an engine name | not stated | 9 |
| EMBEDDING39 | "Engine selection does not select an OS target." | Standalone execution selection | Engine selection must not change or select the OS target | not stated | 9 |
| EMBEDDING40 | "Normal package, Build and trust admission remain required." | Standalone execution selection | Psi runs must still require normal package, Build, and trust admission | at entry | 9 |
| EMBEDDING41 | "Missing provider or execution support reports unavailability before guest effects; no native fallback or replacement provider is inferred." | Standalone execution selection | Missing provider or execution support must report unavailability before guest effects without inferring native fallback or replacement provider | at entry | 9 |
| EMBEDDING42 | "locator data selects a foreign leaf but does not replace provider admission or runtime authority checks." | Imports, exports, and provider slots | Foreign locator selection must not substitute for provider admission or runtime authority checks | not stated | 9 |
| EMBEDDING43 | "Every supplied argument must have actual backing and established qualifications." | Imports, exports, and provider slots | Every supplied boundary argument must have backing and established qualifications | at a call | 2 |
| EMBEDDING44 | "A machine name or mutable parameter creates no hidden receiver, allocation or default context." | Imports, exports, and provider slots | Machine names and mutable parameters must not imply hidden receiver, allocation, or default context | not stated | 1 |
| EMBEDDING45 | "Requirement and implementation signatures retain the same explicit parameter correspondence." | Imports, exports, and provider slots | Requirement and implementation signatures must keep explicit parameter correspondence | not stated | 7 |
| EMBEDDING46 | "An admitted adapter may supply declared arguments, not synthesize a context by inspecting the selected function's signature." | Imports, exports, and provider slots | Adapters may supply only declared arguments and must not synthesize context from callee signature inspection | at a call | 3 |
| EMBEDDING47 | "Neither `public` nor `satisfies` alone exports a callable ABI." | Imports, exports, and provider slots | `public` or `satisfies` alone must not constitute exported callable ABI | not stated | 9 |
| EMBEDDING48 | "No `main`, `run`, or `Run` name is privileged by embedding." | Imports, exports, and provider slots | Embedding must not privilege `main`, `run`, or `Run` names as entry | not stated | 9 |
| EMBEDDING49 | "Multiple static roles use distinct applications or wrapper requirements, not same-typed source fields." | Imports, exports, and provider slots | Multiple static roles must use distinct applications or wrappers, not same-typed fields | not stated | 9 |
| EMBEDDING50 | "Establish every demanded import before running guest code, except an explicitly reserved component under the readiness lifecycle above;" | Imports, exports, and provider slots | All demanded imports must be established before guest execution except explicitly reserved readiness-lifecycle slots | at entry | 9 |
| EMBEDDING51 | "missing, duplicate, wrong-instance, incompatible, ambiguous, or unauthorized supply rejects without an ambient fallback." | Imports, exports, and provider slots | Bad import supply must reject without ambient fallback | at entry | 9 |
| EMBEDDING52 | "Provider supply does not transfer authority;" | Imports, exports, and provider slots | Provider installation must not transfer authority to guest | not stated | 8 |
| EMBEDDING53 | "operation arguments and admitted receiver policy carry their own obligations." | Imports, exports, and provider slots | Arguments and receiver policy must carry their own authority obligations | at a call | 8 |
| EMBEDDING54 | "admission rejoins exact declarations, applications, contracts, representations, and selected adapters." | Imports, exports, and provider slots | Untrusted adapter generation admission must rejoin exact declarations, applications, contracts, representations, and adapters | at entry | 9 |
| EMBEDDING55 | "Display-name lookup resolves to that exact entry;" | Imports, exports, and provider slots | Display-name lookup must resolve to the exact retained entry | at a call | 9 |
| EMBEDDING56 | "a string, ordinal, or address alone establishes neither interface compatibility nor call authority." | Imports, exports, and provider slots | String, ordinal, or address alone must not establish interface compatibility or call authority | at a call | 3 |
| EMBEDDING57 | "A guest boundary call dispatches through the exact installed slot and its host adapter/context." | Imports, exports, and provider slots | Guest boundary calls must dispatch only through exact installed slot and adapter or context | at a call | 3 |
| EMBEDDING58 | "Runtime-discovered native signatures need a supported checked ABI adapter, not an arbitrary function-pointer cast." | Imports, exports, and provider slots | Runtime-discovered native signatures require supported checked ABI adapter; arbitrary function-pointer casts must not suffice | at a call | 3 |
| EMBEDDING59 | "Host-to-guest calls satisfy the complete entry contract too." | Imports, exports, and provider slots | Host-to-guest calls must satisfy the complete entry contract | at a call | 3 |
| EMBEDDING60 | "Well-shaped scalars do not establish preconditions, domain membership, ownership, or grants." | Imports, exports, and provider slots | Well-shaped scalars at boundaries must not be treated as establishing preconditions, domains, ownership, or grants | at a call | 0 |
| EMBEDDING61 | "Static proofs, admitted evidence, or checked entry operations establish those obligations;" | Imports, exports, and provider slots | Those obligations must be established only by static proofs, admitted evidence, or checked entry operations | at a call | 8 |
| EMBEDDING62 | "arbitrary mathematical contracts are not automatically executable validators." | Imports, exports, and provider slots | Mathematical contracts must not run as automatic executable validators at boundaries | at a call | 8 |
| EMBEDDING63 | "Native provider behavior remains an explicit trusted premise unless independently checked." | Imports, exports, and provider slots | Native provider behavior must be treated as trusted premise unless independently checked | not stated | 8 |
| EMBEDDING64 | "Proof-only entities with no executable realization reject instead of receiving fake entries or zero." | Address observations | Proof-only entities without executable realization must reject rather than receive fake entries or zero | not stated | 8 |
| EMBEDDING65 | "Address demand retains an otherwise uncalled executable body." | Address observations | Address demand must retain otherwise uncalled executable bodies | not stated | 1 |
| EMBEDDING66 | "The query creates no host ABI adapter, indirect-call permission, byte-addressed memory model, or reflective edit operation." | Address observations | Address queries must not create host ABI adapter, indirect-call permission, byte-addressed memory model, or reflective edit capability | at the operation | 9 |
| EMBEDDING67 | "The identities do not make native address arithmetic or physical memory accesses meaningful by themselves." | Address observations | Address identities must not by themselves legitimize native address arithmetic or physical memory access | not stated | 5 |
| EMBEDDING68 | "`code_layout<Machine>()` cannot describe a purely interpreted body's native regions: those do not exist." | Address observations | `code_layout` must not describe native regions for purely interpreted bodies | not stated | 9 |
| EMBEDDING69 | "Data `size_of<T>()` and `align_of<T>()` instead observe the selected target layout, not the interpreter's host storage." | Address observations | `size_of` and `align_of` for data must use selected target layout, not host interpreter storage | not stated | 9 |
| EMBEDDING70 | "Runtime-scoped instance and entry handles cannot become valid for an unrelated instance after storage reuse." | Ownership and lifetime | Runtime-scoped handles must not become valid for unrelated instances after storage reuse | not stated | 2 |
| EMBEDDING71 | "neither arrangement grants unrestricted access while guest loans exclude it." | Ownership and lifetime | Host ownership or borrow must not grant unrestricted access while guest loans exclude it | not stated | 2 |
| EMBEDDING72 | "Scalars are checked against their exact carriers." | Ownership and lifetime | Boundary crossings must check scalars against exact carriers | on every edge | 0 |
| EMBEDDING73 | "Records, sums, and arrays use checked representations, not reinterpretation of arbitrary host structs." | Ownership and lifetime | Records, sums, and arrays must use checked representations, not host struct reinterpretation | on every edge | 1 |
| EMBEDDING74 | "Borrowed views retain the backing, access mode, and loan for their complete lifetime, including a permitted suspension." | Ownership and lifetime | Borrowed views must retain backing, access mode, and loan for full lifetime including permitted suspension | across a pause | 2 |
| EMBEDDING75 | "Host resources retain exact owner, lifetime, and authority." | Ownership and lifetime | Host resources at boundaries must retain exact owner, lifetime, and authority | not stated | 2 |
| EMBEDDING76 | "A numeric identifier supplied by a script cannot fabricate them." | Ownership and lifetime | Script-supplied numeric identifiers must not fabricate host resource owner, lifetime, or authority | not stated | 2 |
| EMBEDDING77 | "Guest values retained by the host keep their necessary instance/component custody alive." | Ownership and lifetime | Host-retained guest values must keep required instance or component custody alive | not stated | 2 |
| EMBEDDING78 | "Erasure does not license reconstructing proofs, grants, linear resources, or qualifiers from metadata or bytes." | Ownership and lifetime | Erased data must not be used to reconstruct proofs, grants, linear resources, or qualifiers | not stated | 8 |
| EMBEDDING79 | "A work-budget pause is an interpreter-safe stop, not source `suspends`, a call return, cancellation, or an invariant-restoring observation point." | Execution and interference | Work-budget pause must not be treated as source suspends, call return, cancellation, or invariant-restoring point | across a pause | 10 |
| EMBEDDING80 | "Live loans, facts, partially moved storage, invariant windows, and component pins remain attached to the activation." | Execution and interference | On work-budget pause, loans, facts, partial moves, invariant windows, and pins stay on the activation | across a pause | 2 |
| EMBEDDING81 | "An external operation cannot inspect or mutate affected live values while those obligations exclude it." | Execution and interference | External operations must not inspect or mutate live values when obligations exclude access | at the operation | 2 |
| EMBEDDING82 | "Even a write preserving the type's invariant can invalidate an activation's stronger flow facts." | Execution and interference | Type-invariant-preserving writes must still be checked against activation flow facts | at the operation | 8 |
| EMBEDDING83 | "Metadata is not a borrow permit." | Execution and interference | Metadata must not authorize borrows by itself | not stated | 2 |
| EMBEDDING84 | "A queued edit acquires valid access and establishes its contract when applied, not merely when queued." | Execution and interference | Queued edits obtain access and contract only when applied | at the operation | 2 |
| EMBEDDING85 | "Re-entry obeys the existing callback contract and must preserve all retained access and custody constraints." | Execution and interference | Re-entry must obey callback contract and preserve access and custody constraints | at a call | 3 |
| EMBEDDING86 | "Invocation arguments and binding identities cannot be substituted by passing a different handler on resume." | Execution and interference | Resume must not substitute invocation arguments or binding identities via a different handler | across a pause | 3 |
| EMBEDDING87 | "For independent slots, each entry pins its selected implementation until leave;" | Execution and interference | Independent slot entries must pin selected implementation until leave | at entry | 9 |
| EMBEDDING88 | "Pending host operations complete against their exact request identity once." | Execution and interference | Pending host operations must complete once against exact request identity | at a call | 3 |
| EMBEDDING89 | "Completed effects are not repeated on resumption." | Execution and interference | Completed host effects must not repeat on resumption | across a pause | 3 |
| EMBEDDING90 | "Rejection after a possible host effect neither rolls it back nor permits automatic retry." | Execution and interference | Post-effect rejection must neither roll back nor auto-retry | at a call | 3 |
| EMBEDDING91 | "Dropping an invocation handle does not discard its unfinished activation or release loans;" | Execution and interference | Dropping invocation handle must not discard unfinished activation or release loans | not stated | 2 |
| EMBEDDING92 | "Queuing an event reports queue acceptance, not that the event's requested action occurred." | Messages and callbacks | Event queue APIs must report acceptance only, not completion of requested action | at a call | 3 |
| EMBEDDING93 | "An idle guest executes only when started/resumed by its selected execution mechanism;" | Messages and callbacks | Idle guests must run only when started or resumed by selected execution mechanism | not stated | 10 |
| EMBEDDING94 | "Dynamic lookup chooses an already retained entry; it cannot instantiate arbitrary new generic code or acquire private access." | Reflection and debugging | Dynamic lookup must target retained entries only and must not instantiate arbitrary generics or private access | at a call | 7 |
| EMBEDDING95 | "There is no safe embedding operation for arbitrary writes to live fields, locals, or instructions." | Reflection and debugging | Embedding must not offer safe arbitrary writes to live fields, locals, or instructions | not stated | 2 |
| EMBEDDING96 | "Read and update live objects through exposed checked operations at valid access points." | Reflection and debugging | Live object reads and updates must go through exposed checked operations at valid access points | at the operation | 2 |
| EMBEDDING97 | "A detached editable document is not yet an instance of its described type." | Reflection and debugging | Detached editable documents must not be treated as typed instances | not stated | 1 |
| EMBEDDING98 | "Constructors and update operations establish invariants and resource custody before returning established values." | Reflection and debugging | Constructors and updates must establish invariants and custody before returning established values | at normal return | 2 |
| EMBEDDING99 | "They must distinguish unavailable, erased, uninitialized, and currently unestablished data." | Reflection and debugging | Debugger descriptions must distinguish unavailable, erased, uninitialized, and unestablished data | not stated | SHARED |
| EMBEDDING100 | "Such diagnostic data grants no typed value, callable handle, or resource authority and executes no getter merely to display it." | Reflection and debugging | Diagnostic data must grant no value, handle, or authority and must not run getters for display alone | not stated | 8 |
| EMBEDDING101 | "A debugger write-and-resume escape hatch is not part of this safe contract." | Reflection and debugging | Debugger write-and-resume must not be part of safe embedding contract | not stated | 2 |
| EMBEDDING102 | "Prevent guest resumption and new guest-originated host calls." | Closing and failure | Guest discard must prevent resumption and new guest-originated host calls | not stated | 3 |
| EMBEDDING103 | "Reclaim interpreter-owned storage whose loans and cleanup obligations permit it." | Closing and failure | Reclamation must respect loans and cleanup obligations on interpreter-owned storage | not stated | 2 |
| EMBEDDING104 | "while withholding resumption of the discarded guest." | Closing and failure | Late completions must not resume discarded guest | not stated | 10 |
| EMBEDDING105 | "A cancellation request alone releases nothing." | Closing and failure | Cancellation request alone must not release resources | not stated | 2 |
| EMBEDDING106 | "Completed effects remain completed; no rollback or immediate reclamation is promised." | Closing and failure | Completed effects must remain; rollback and immediate reclamation must not be assumed | not stated | 3 |
| EMBEDDING107 | "Rejection or Busy preserves custody." | Closing and failure | Rejection or Busy close outcomes must preserve custody | not stated | 2 |
| EMBEDDING108 | "Shutdown releases host state only after its outstanding obligations settle;" | Closing and failure | Shutdown must release host state only after obligations settle | not stated | 2 |
| EMBEDDING109 | "If the selected embedding cannot safely contain abandonment, reject that mode before granting such resources or require an appropriate wider custodian." | Closing and failure | Unsafe containment modes must reject before granting risky resources or require wider custodian | not stated | 9 |
| EMBEDDING110 | "Memory leakage alone does not settle external protocol debt." | Closing and failure | Memory leakage must not be treated as settling external protocol debt | not stated | 2 |
| EMBEDDING111 | "Neither a paused call nor a host-language destructor establishes safe cancellation." | Closing and failure | Paused calls and host destructors must not imply safe cancellation | not stated | 2 |
| EMBEDDING112 | "It does not invent another component manager or weaker authority model." | Replacement and native equivalence | Interpreted component providers must not invent alternate manager or weaker authority | not stated | 9 |
| EMBEDDING113 | "only the designated update authority may publish." | Replacement and native equivalence | Publication must be performed only by designated update authority | not stated | 9 |
| EMBEDDING114 | "An external editor submits a candidate through that authority rather than overwriting interpreter state." | Replacement and native equivalence | External editors must submit candidates through update authority, not overwrite interpreter state | not stated | 9 |
| EMBEDDING115 | "Authorization for the old bytes does not cover the replacement." | Replacement and native equivalence | Old-byte authorization must not cover replacement artifact execution | not stated | 9 |
| EMBEDDING116 | "Preparation binds to the new artifact and context." | Replacement and native equivalence | Preparation after replacement must bind to new artifact and context | not stated | 9 |
| EMBEDDING117 | "Automatic migration of active continuations into edited code and retroactive discovery of safe replacement cuts are not supplied by reflection or recompilation." | Replacement and native equivalence | Reflection or recompilation must not be assumed to migrate active continuations or discover safe replacement cuts | not stated | 10 |
| EMBEDDING118 | "There is no ambient filesystem, network, process, or loader capability." | Authority, evidence, and ownership of implementation | Embedding must not assume ambient filesystem, network, process, or loader capability | not stated | 9 |
| EMBEDDING119 | "Descendants may attenuate supply, not turn virtual backing into genuine host access." | Authority, evidence, and ownership of implementation | Descendants may attenuate supply but must not convert virtual backing into genuine host access | not stated | 9 |
| EMBEDDING120 | "A binding's type does not constrain malicious native code by itself; containment and provider trust remain explicit." | Authority, evidence, and ownership of implementation | Type of binding must not alone constrain malicious native code; containment and trust stay explicit | not stated | 8 |
| EMBEDDING121 | "Ordinary structure, schema, input, authority, and execution checks always apply." | Authority, evidence, and ownership of implementation | Structure, schema, input, authority, and execution checks must always apply in embedding | not stated | SHARED |
| EMBEDDING122 | "Interpreted components execute supported checked instruction semantics, not arbitrary machine-code bytes." | Checked assembly and target environments | Interpreted execution must run supported checked instruction semantics only, not arbitrary machine-code bytes | at the operation | 10 |
| EMBEDDING123 | "Block-local scratch registers/flags do not communicate implicitly with another block or surrounding generated code." | Checked assembly and target environments | Block-local scratch must not implicitly communicate across blocks or surrounding generated code | on every edge | 10 |
| EMBEDDING124 | "Live access requires actual host authority." | Checked assembly and target environments | Live access to modeled devices requires actual host authority | at the operation | 9 |
| EMBEDDING125 | "Instruction coverage and environment compatibility are checked before execution and at component installation." | Checked assembly and target environments | Instruction coverage and environment compatibility must be checked before execution and at installation | at entry | 10 |
| EMBEDDING126 | "Unknown/unsupported effects reject without native fallback." | Checked assembly and target environments | Unknown or unsupported assembly effects must reject without native fallback | not stated | 10 |
| EMBEDDING127 | "Halt and external entry obey the target model, not work-budget-pause behavior;" | Checked assembly and target environments | Halt and external entry must follow target model, not work-budget-pause semantics | at the operation | 10 |
| EMBEDDING128 | "the existing invocation/resource lifecycle retains pending effects and prevents re-execution on resume." | Checked assembly and target environments | Invocation lifecycle must retain pending effects and prevent re-execution on resume | across a pause | 10 |
| EMBEDDING129 | "they must not implement Cathedral's boot, scheduler, migration, or update policy as a substitute for running that code." | Interpreted Cathedral acceptance | Host providers must not substitute for running Cathedral boot, scheduler, migration, or update policy in guest code | not stated | 9 |
| EMBEDDING130 | "Preserve exact contract/custody and reject incompatible or unauthorized replacement." | Interpreted Cathedral acceptance | Replacement demonstrations must preserve contract and custody and reject incompatible or unauthorized replacement | not stated | 9 |
| EMBEDDING131 | "Do not replace the exercised OS sequence with a host routine, demand assembly removal, or report support for unimplemented instructions." | Interpreted Cathedral acceptance | Cathedral acceptance work must not replace OS sequence with host routine, demand assembly removal, or claim unimplemented instructions as supported | not stated | 10 |
| EMBEDDING132 | "Guest source calls its requirements directly; it carries no opaque routing value." | Imports, exports, and provider slots | Guest source must call requirements directly without opaque routing values | at a call | 3 |
| EMBEDDING133 | "An instance's import roster identifies each exact slot/occurrence, closed requirement application, and full contract." | Imports, exports, and provider slots | Import roster must identify each slot, occurrence, closed requirement application, and full contract | at entry | 9 |
| EMBEDDING134 | "Ordinary host callbacks receive no unrestricted mutable runtime." | Execution and interference | Ordinary host callbacks must not receive unrestricted mutable runtime access | at a call | 2 |
| EMBEDDING135 | "Management operations work through scoped authority;" | Execution and interference | Component-management operations must use scoped authority only | at a call | 9 |
| EMBEDDING136 | "Direct Psi transformation is another producing step whose exact output needs host trust before execution or native lowering." | Replacement and native equivalence | Direct Psi transformation output requires host trust before execution or native lowering | not stated | 9 |
| EMBEDDING137 | "Separate instances are not automatically OS sandboxes." | Authority, evidence, and ownership of implementation | Separate interpreter instances must not be treated as automatic OS sandboxes | not stated | 9 |
| EMBEDDING138 | "Explicit values, authorized memory and contracted target state do communicate;" | Checked assembly and target environments | Explicit values, authorized memory, and contracted target state must communicate across blocks | on every edge | 10 |
| EMBEDDING139 | "those effects persist across blocks and pauses." | Checked assembly and target environments | Communicated assembly effects must persist across blocks and pauses | across a pause | 10 |
| EMBEDDING140 | "A modeled device, interrupt mask or processor is not the host's device or processor." | Checked assembly and target environments | Modeled device, interrupt mask, or processor must not be identified with host hardware | not stated | 9 |
| EMBEDDING141 | "Receiving limits do not require rediscovering the producer's source proofs." | Loading and preparation | Enforcing receiving limits must not require rediscovering producer source proofs | not stated | 8 |
| EMBEDDING142 | "The retained plan states what to check, not that a future value passes." | Loading and preparation | Retained dispatch plan must specify checks only, not pre-approve future values | at the operation | SHARED |
| EMBEDDING143 | "Actual bounds, liveness, permissions and supported value predicates are evaluated under accounted resource limits." | Loading and preparation | Runtime bounds, liveness, permissions, and predicates must run under accounted resource limits | at the operation | SHARED |
| EMBEDDING144 | "Ordinary work over runtime data is unaffected by the declaration-lookup rule." | Loading and preparation | Declaration-lookup rule must not block ordinary runtime-data work | at the operation | SHARED |
| EMBEDDING145 | "Stronger state continuity requires the selected coordination/migration contract." | Replacement and native equivalence | Stronger continuity across replacement requires selected coordination or migration contract | not stated | 9 |
| EMBEDDING146 | "with authority, persistent state and once-only effects preserved across work pauses." | Interpreted Cathedral acceptance | Cathedral assembly demonstrations must preserve authority, persistent state, and once-only effects across work pauses | across a pause | 10 |

## Ambiguous sentences

| Sentence | Reading A | Reading B |
| --- | --- | --- |
| "Loading Psi does not run `build.omg`, fetch packages, compile source, or obtain filesystem or installation authority." | The loader API must never invoke those build steps when given Psi bytes. | A host may still run builds separately; this sentence only forbids implicit build side effects on load, not explicit host-initiated builds. |
| "Build permissions do not become runtime bindings." | Build-time granted capabilities are not automatically installed as runtime provider bindings. | Build permissions may inform admission but do not transfer as ambient authority at guest entry. |
| "no whole-program rebuild or incremental-compilation speedup is implied." | Component-scoped build must not assume whole-program or incremental speedups. | Building a component may still reuse incremental infrastructure; the contract does not promise any particular performance. |
| "Untrusted execution is unavailable until the interpreter, adapters and teardown meet the containment acceptance in the implementation plan." | Admission must fail closed until containment criteria in the linked plan are met. | Containment acceptance is an implementation milestone; this chapter does not state whether partial admission with restrictions is allowed before full acceptance. |
| "Trusted immutable stage outputs may carry their existing authorization." | Stage outputs inherit authorization without re-proving producer identity at load. | "Existing authorization" may mean only authorization attached in the artifact format; whether hosts may widen trust is not stated. |
| "Per-query limits do not replace the total budget." | Individual query caps must still count toward and cannot exceed the overall work budget. | Per-query limits are additional; whether they can exhaust before total budget is not stated. |
| "Producer trust changes admission and permission to optimize or lower; it does not select a dispatch mode that skips concrete safety checks." | Trusted producers may get faster paths but never skip runtime safety checks. | Trust may enable more optimization at prepare time only; whether lowering differs by trust is not stated here. |
| "Interpretation is not another OS target." | Engine selection must not be confused with OS target environment selection. | Interpreted builds may still require target metadata; "not another OS target" addresses product taxonomy only. |
| "Generic loading and execution require neither those optional adapters nor source recompilation." | Psi load and run must work without optional host adapters or recompile. | Optional adapters may still be needed for specific provider closures even if generic load succeeds. |
| "A branch merely expected not to run cannot excuse missing supply;" | Static analysis must demand all closure providers regardless of reachability heuristics. | Only providers in the selected executable closure count; "expected not to run" refers to host speculation, not compiler unreachable-code removal. |
| "This is explicit scheduling, never speculation that an unbound branch will not run." | Readiness protocol rejects speculative deferral of provider installation. | Explicit scheduling applies only to readiness lifecycle slots, not all dynamic calls. |
| "Omission selects `native`;" | CLI without `--engine` runs native emission and launch. | Default applies only to `omega run`; other entry points may differ and are not stated here. |
| "It uses the shared Psi program/instance lifecycle, not a second interpreter or source evaluator." | Psi runs must use the standard program or instance lifecycle APIs. | "Not a second interpreter" forbids duplicate evaluators; whether source-level interpretation exists elsewhere is not stated. |
| "Generation derives from checked interfaces and may be untrusted:" | Adapter generation may run without trusting the generator; admission revalidates. | Untrusted generation must still emit only forms admission can verify; whether all generator outputs are re-checked is not detailed here. |
| "A compiled adapter can make an ordinary native host call; the interpreter does not need to generate machine code per call." | Boundary dispatch may use precompiled adapters without per-call codegen. | Interpreter may still JIT or specialize elsewhere; requirement is per-call codegen is not mandatory. |
| "Native provider behavior remains an explicit trusted premise unless independently checked." | Compiler must not treat native provider semantics as proven without separate checking. | "Independently checked" may mean runtime tests or proof; which mechanisms count is not stated. |
| "Existing interpreter identities may supply these values;" | Address queries may use interpreter-supplied identities without a second registry. | Host may supply identities; whether interpreter or host is authoritative when both exist is not stated. |
| "Owned transfer and compatible borrowing need not serialize or copy every value." | Crossings may transfer ownership or borrow without full serialization when compatible. | Compatibility rules are ordinary value rules; which types skip copy is not enumerated here. |
| "A conservative wrapper may keep a runtime exclusively borrowed across an unfinished call, but this is not the general execution model:" | Exclusive runtime borrow across calls is optional wrapper behavior, not default. | Long-running guests must allow management operations; how wrappers interact with that requirement is not fully specified. |
| "a long-running guest must be able to invoke authorized component-management operations and permit independently admissible work without returning from its top-level machine." | Guest need not return from top-level machine for admissible management work. | "Independently admissible work" scope is library-defined; which operations qualify is not listed here. |
| "Holding an outer guest invocation does not freeze every component's provider for that outer invocation's lifetime." | Outer invocation does not pin all component providers until it completes. | Pins apply per independent slot entry; extent of "every component" vs subset is contextual. |
| "Delivery, capacity, backpressure, ordering, and completion are explicit library contracts, not language-wide event semantics." | Language does not fix event semantics; libraries must declare them. | Compiler has no obligation to verify queue contracts unless expressed in checked interfaces. |
| "Ordinary internal Omega mutation still uses machines and borrows; it need not introduce a boundary trait for every setter." | Internal mutation follows ordinary machines and borrows without per-setter boundary traits. | Embedding reflection APIs are separate; whether every external update needs boundary traits is not restated. |
| "Consume late completions for settlement and cleanup," | Late async completions must be consumed for settlement without guest resume. | Whether consumption implies running guest code or host-only handlers is not stated. |
| "Continuity-free publication may precede old-version drain;" | New version may publish before old activations drain when continuity-free contract selected. | What "continuity-free" permits for in-flight calls is defined only by coordination contract named elsewhere. |
| "This does not require physically copying every unchanged byte." | Replacement preparation may reuse unchanged logical content without byte-identical copy. | Artifact identity and authorization still bind to new artifact object; reuse mechanism is engineering. |
| "Semantic constant evaluation retains its separate admission regime." | Const eval admission is outside embedding rules in this chapter. | Embedding loaders must not conflate semantic const eval trust with Psi execution admission; boundaries are not fully specified here. |
| "Keep device mechanisms in environment providers and OS policy in the interpreted application, not the compiler." | Compiler must not embed device or OS policy; those live in providers and guest code. | Compiler still checks instruction coverage; division of who models policy vs who checks syntax is architectural, not a rejection rule. |
| "Simulation establishes behavior under its stated model, not native timing or actual hardware correctness." | Acceptance tests under simulation prove model behavior only. | Compiler or harness must not claim hardware correctness from simulation; whether simulation affects admission is not stated. |

## Terms used without definition in this chapter

- Embedding
- interpreter library
- Psi (as consumed artifact)
- shared interface packages
- source compiler (as separate dependency)
- `build.omg`
- lock agreement
- attenuated authority
- nested build files
- runtime bindings
- fused dependencies
- checked closure
- stable slots
- component publication
- interpreted realization
- Psi artifact
- compilation context
- rebuild/replacement cohort
- artifact acquisition
- virtual filesystem
- module resolver
- container formats
- physical representations
- provider implementations
- ABI/container
- Psi admission contract
- inspecting bytes vs authorizing execution
- execution trust
- execution-only descriptor analysis
- entailment analysis
- execution metadata
- installation relationships
- executable instance
- containment acceptance
- implementation plan (psi claim admission)
- codec and preparation responsibilities
- byte-loading APIs
- module-start APIs
- trusted immutable stage outputs
- producer name
- native realization
- 64 MiB input cap
- bounded counts
- nesting and string limits
- overall work budget
- canonical re-encoding
- descriptor analysis
- bounded entailment
- resource refusal
- declaration-dependent restrictions
- descriptor plans
- indexed lookups
- installation context
- declaration-lookup rule
- independent execution oracle
- supported value predicates
- producer trust
- target environment
- execution engine
- Cathedral (as environment name)
- Linux contracts
- instruction semantics
- provider bodies
- adapters (host)
- portable filesystem binding
- virtual implementation
- Linux syscall binding
- foreign identity
- virtual-service tests
- `hosted-interpreter-abi-library-dependency`
- host bindings
- target-aware host adapters
- host integration
- locator libraries
- mechanism libraries
- executable closure
- dynamic calls
- Build provider-readiness protocol
- static component slot
- readiness crossing
- activation
- host-adapter permission
- explicit-execution-engine
- `--both`
- program/provider configuration
- Psi program/instance lifecycle
- source evaluator
- JIT / hot-code compilation
- boundary-trait application
- import/provider slot
- provider-slot realization
- opaque routing value
- `ForeignBinding`
- foreign leaf
- shared interface declarations
- entry roots
- established qualifications
- installed-call contracts
- import roster
- closed requirement application
- wrapper requirements
- ambient fallback
- import/export adapters
- display-name lookup
- host adapter/context
- runtime-discovered native signatures
- checked ABI adapter
- complete entry contract
- grants
- admitted evidence
- trusted premise
- address queries / address exposure
- entry identity
- location identity
- address registry
- proof-only entities
- executable realization
- host pointers
- native trampolines
- indirect-call permission
- byte-addressed memory model
- reflective edit operation
- `code_layout<Machine>()`
- purely interpreted body
- Program / Runtime / Invocation (library roles)
- program owner
- runtime-scoped handles
- guest loans
- checked representations
- host structs
- permitted suspension
- host resources
- instance/component custody
- erasure
- linear resources
- qualifiers
- work-budget pause
- source `suspends`
- invariant windows
- component pins
- flow facts
- queued edit
- component-management operations
- scoped authority
- callback contract (private callbacks)
- independent slots
- pending request identity
- queue acceptance
- command/event queues
- admitted asynchronous completion
- semantic metadata
- runtime descriptions and access views
- dynamic invocation
- detached editable document
- debugger descriptions
- write-and-resume escape hatch
- retention costs
- universal object header
- guest resumption
- guest-originated host calls
- provider operations
- interpreter-owned storage
- provider-held storage
- late completions
- containment audit
- Busy (close outcome)
- disposition/containment contract
- DMA loan
- registered callback
- wider custodian
- interpreted component provider
- component manager
- update authority
- continuity-free publication
- old-version drain
- coordination/migration contract
- Direct Psi transformation
- ambient filesystem/network/process/loader capability
- virtual backing
- malicious native code
- semantic constant evaluation
- separate admission regime
- checked instruction semantics
- block-local scratch registers/flags
- modeled device
- interrupt mask
- environment providers
- instruction coverage
- environment compatibility
- halt / external entry
- realization evidence
- Cathedral algorithms and state machines
- boot / scheduler / migration / update policy (as forbidden host substitutes)
- device-interaction sequence
- processor idle/external-entry sequence
- once-only effects
- OS sequence
- Cathedral instruction coverage

CHAPTER COMPLETE
