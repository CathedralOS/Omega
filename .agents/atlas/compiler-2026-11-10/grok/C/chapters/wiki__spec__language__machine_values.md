# Machines as runtime values

Source: `wiki/spec/language/machine_values.md` lines 1–152 (read).

## Rules

| rule id | the exact sentence, quoted | the heading it is under | what must be checked, in plain words | when it must hold | lane |
| --- | --- | --- | --- | --- | --- |
| MACHINE_VALUES1 | "A callable field publishes the complete contract its caller may rely on." | Machines as runtime values | Callable fields must expose the full callable contract to callers | not stated | 10 |
| MACHINE_VALUES2 | "All `dyn` source forms, including captured `dyn Trait<_>`, are retired." | Machines as runtime values | Programs using any `dyn` source form, including captured `dyn Trait<_>`, must be rejected | not stated | 10 |
| MACHINE_VALUES3 | "Static traits, named conformances, proof laws and `machine F` generic binders remain." | Machines as runtime values | Static traits, named conformances, proof laws, and `machine F` generic binders must remain available | not stated | 7 |
| MACHINE_VALUES4 | "A runtime callable type uses the structural signature and clauses already used by a `requires machine` requirement, without a declaration name:" | Callable types and construction | Runtime callable types must match `requires machine` structural signature and clauses and must not carry a declaration name | not stated | 10 |
| MACHINE_VALUES5 | "The same type form is usable for locals, parameters, results, fields, arrays and other ordinary containers." | Callable types and construction | The runtime callable type form must be accepted in locals, parameters, results, fields, arrays, and ordinary containers | not stated | 1 |
| MACHINE_VALUES6 | "Parameter names in its contract bind that callable's formal arguments; `result` denotes its result." | Callable types and construction | Contract parameter names must bind the callable's formals and `result` must denote its return | not stated | 10 |
| MACHINE_VALUES7 | "Lifetime binders and ordinary `requires`, `ensures`, reach, invocation, suspension, blocking, crash, progress, resource and custody contracts retain their meanings." | Callable types and construction | Callable contracts must interpret those clauses with their existing meanings | not stated | 10 |
| MACHINE_VALUES8 | "A runtime callable has fixed finite service and crash ceilings that the type must state:" | Callable types and construction | Runtime callable types must state fixed finite service and crash ceilings | not stated | 10 |
| MACHINE_VALUES9 | "`reaches none` and `crashes none` mean empty," | Callable types and construction | `reaches none` and `crashes none` on callable types must mean empty ceilings | not stated | 10 |
| MACHINE_VALUES10 | "and a callable type that omits either clause is an [open callable contract](effects.md#open-callable-contracts), which a runtime position rejects." | Callable types and construction | Open callable contracts (omitting reach or crash clause) must be rejected at runtime positions | not stated | 10 |
| MACHINE_VALUES11 | "Other axes retain their existing omission rules." | Callable types and construction | Non-reach/non-crash contract axes must follow existing omission rules from elsewhere | not stated | 10 |
| MACHINE_VALUES12 | "In a callable-valued context, a named machine reference selects an exact closed declaration application." | Callable types and construction | Named machine references in callable-valued contexts must resolve to an exact closed declaration application | not stated | 10 |
| MACHINE_VALUES13 | "A qualified named implementation member is eligible; the conformance must be explicitly named, and the exact overload and all static arguments resolved by ordinary selection." | Callable types and construction | Qualified implementation members require explicitly named conformance and resolved overload with all static arguments by ordinary selection | not stated | 7 |
| MACHINE_VALUES14 | "No search for a unique visible implementation or implicit whole-trait conversion is introduced." | Callable types and construction | Selection must not search for a unique visible implementation or perform implicit whole-trait conversion | not stated | 7 |
| MACHINE_VALUES15 | "Unrestricted type-, machine- or evidence-polymorphic runtime callables are outside V1; close their static arguments before constructing a value." | Callable types and construction | Unrestricted polymorphic runtime callables must be rejected or static arguments must be closed before value construction | not stated | 7 |
| MACHINE_VALUES16 | "Existing explicitly finite value families retain their declared coverage, as described in [Terminal dispatch](../terminal-psi/dynamic_dispatch.md#finite-generic-method-families)." | Callable types and construction | Explicitly finite value families must keep their declared coverage per Terminal dispatch | not stated | 10 |
| MACHINE_VALUES17 | "Construction proves ordinary [contract refinement](machines.md#substitution): the selected body must accept every input permitted by the callable requirement, provide its guarantees, and stay within its operational/resource/custody bounds." | Callable types and construction | Construction must prove refinement: body accepts requirement inputs, meets guarantees, and stays within operational/resource/custody bounds | not stated | 8 |
| MACHINE_VALUES18 | "Substitute any static callback or conformance-member reach dependencies before checking the closed selection against the destination callable's finite ceiling." | Callable types and construction | Reach dependencies from static callbacks or conformance members must be substituted before comparing closed selection to destination finite ceiling | not stated | 3 |
| MACHINE_VALUES19 | "It does not run the body." | Callable types and construction | Callable construction checking must not execute the machine body | not stated | 10 |
| MACHINE_VALUES20 | "Machine values contain code selection and retained contract evidence, not an implicit state capture, receiver, allocation, raw address or new source of authority." | Callable types and construction | Machine values must hold only code selection and contract evidence; must not imply implicit capture, receiver, allocation, raw address, or new authority | not stated | 10 |
| MACHINE_VALUES21 | "Ordinary initialization must establish a callable; no null/default callable or unchecked integer-to-callable cast exists." | Callable types and construction | Callables must be established by ordinary initialization only; null, default, and unchecked integer-to-callable casts must be rejected | not stated | 10 |
| MACHINE_VALUES22 | "Calling `f(args)` or `(record.operation)(args)` uses the value's current callable contract and exact argument/result mapping." | Invocation, storage and evidence | Calls must use the value's current contract and exact argument/result mapping | at the operation | 3 |
| MACHINE_VALUES23 | "It proves current preconditions and access obligations, retains permitted effects and progress, and supplies checked postconditions and returned-borrow origins." | Invocation, storage and evidence | Calls must prove preconditions and access obligations, retain permitted effects and progress, and establish postconditions and returned-borrow origins | at the operation | 3 |
| MACHINE_VALUES24 | "Calls can take zero, one or multiple state arguments." | Invocation, storage and evidence | Invocations must allow zero, one, or multiple state arguments | at the operation | 3 |
| MACHINE_VALUES25 | "They are not method calls on the record containing the field." | Invocation, storage and evidence | Callable-field invocation must not be treated as a method call on the containing record | at the operation | 3 |
| MACHINE_VALUES26 | "Forwarding, reassignment, joins, fields and containers preserve the selected code and its contract evidence." | Invocation, storage and evidence | Forwarding, reassignment, joins, fields, and containers must preserve selected code and contract evidence | not stated | 2 |
| MACHINE_VALUES27 | "Known selection may narrow the operational envelope only by [retained checked evidence](effects.md#known-dynamic-selections)." | Invocation, storage and evidence | Narrowing operational envelope for known selection requires retained checked evidence only | not stated | 8 |
| MACHINE_VALUES28 | "Unknown selections use the declared ceiling." | Invocation, storage and evidence | Unknown selections must use the declared operational ceiling | not stated | 10 |
| MACHINE_VALUES29 | "Possible demands union at joins; guarantees intersect." | Invocation, storage and evidence | At joins, possible demands must union and guarantees must intersect | on every edge | 3 |
| MACHINE_VALUES30 | "A reusable body cannot adopt one caller's stronger facts or an optimizer's target prediction as an unconditional promise." | Invocation, storage and evidence | Reusable bodies must not treat caller facts or optimizer predictions as unconditional promises | not stated | 8 |
| MACHINE_VALUES31 | "A code-only value is copyable where its complete qualifications and custody allow." | Invocation, storage and evidence | Copyability follows complete qualifications and custody rules for code-only values | not stated | 2 |
| MACHINE_VALUES32 | "Copying it does not borrow or extend the lifetime of any state argument." | Invocation, storage and evidence | Copying code-only callables must not borrow or extend state-argument lifetimes | not stated | 2 |
| MACHINE_VALUES33 | "Code lifetime and replacement dependencies still apply." | Invocation, storage and evidence | Code lifetime and replacement dependencies must still be enforced on copy | not stated | 2 |
| MACHINE_VALUES34 | "A record containing mutable state references is governed by those references' ordinary multiplicity, not by the copyability of its callable fields." | Invocation, storage and evidence | Record multiplicity follows mutable state references, not callable-field copyability | not stated | 2 |
| MACHINE_VALUES35 | "Operations records are ordinary records." | Invocation, storage and evidence | Operations records must be treated as ordinary records | not stated | 1 |
| MACHINE_VALUES36 | "Their fields are checked independently;" | Invocation, storage and evidence | Operations-record fields must be checked independently | not stated | 1 |
| MACHINE_VALUES37 | "mixing compatible machines from different named implementations is permitted unless a published record predicate or explicit evidence relates them." | Invocation, storage and evidence | Mixing compatible machines across implementations is allowed unless a record predicate or explicit evidence forbids or relates them | not stated | 7 |
| MACHINE_VALUES38 | "Field completeness is not a proof of cross-operation laws, matching allocators, or a shared implementation identity." | Invocation, storage and evidence | Compiler must not infer cross-operation laws, allocator matching, or shared implementation identity from field completeness alone | not stated | 8 |
| MACHINE_VALUES39 | "Such relationships must be stated and proved." | Invocation, storage and evidence | Cross-operation relationships must be explicitly stated and proved | not stated | 8 |
| MACHINE_VALUES40 | "Static trait laws do not appear on a record that does not carry them;" | Invocation, storage and evidence | Records without trait laws must not surface static trait laws | not stated | 7 |
| MACHINE_VALUES41 | "a trait's [schema](conformances.md#implementations-are-operation-records) carries its laws as fields, and construction proves them." | Invocation, storage and evidence | Trait schemas must carry laws as fields and construction must prove them | not stated | 8 |
| MACHINE_VALUES42 | "Callable addresses do not automatically supply executable equality, ordering, hashing, or proof identity." | Invocation, storage and evidence | Callable addresses must not implicitly provide equality, ordering, hashing, or proof identity | not stated | 10 |
| MACHINE_VALUES43 | "Ordinary conformance synthesis needs its published derivation; it cannot compare descriptors or hidden witness tags as a shortcut." | Invocation, storage and evidence | Conformance synthesis must use published derivation; descriptor or hidden witness tag comparison shortcuts are forbidden | not stated | 8 |
| MACHINE_VALUES44 | "In proposition contexts, a call must independently satisfy the ordinary proof admission rules." | Invocation, storage and evidence | Calls in proposition contexts must meet ordinary proof admission rules independently | at the operation | 8 |
| MACHINE_VALUES45 | "A stored callable's signature or absence of effects does not establish totality, termination, mathematical identity or a rewrite law." | Invocation, storage and evidence | Stored callable signature or lack of effects must not prove totality, termination, mathematical identity, or rewrite laws | not stated | 8 |
| MACHINE_VALUES46 | "A static callable selection is an exact machine declaration with its supplied static arguments, its complete contract, any remaining generic parameters, and any binding to a named requirement." | Static callable selections | Static callable selections must bind exact declaration, static arguments, complete contract, remaining generics, and named requirement binding | not stated | 10 |
| MACHINE_VALUES47 | "It may be a field of an ordinary static record, a static argument, or the result of compile-time evaluation." | Static callable selections | Static selections must be allowed as static record fields, static arguments, or compile-time evaluation results | not stated | 1 |
| MACHINE_VALUES48 | "It is a canonical static atom whose identity is the declaration and static application, never an address, generated code, a body comparison or behavioral equivalence." | Static callable selections | Static selection identity must be declaration plus static application only; identity must not use address, generated code, body comparison, or behavioral equivalence | not stated | 10 |
| MACHINE_VALUES49 | "It has no runtime representation:" | Static callable selections | Static callable selections must have no runtime representation | not stated | 10 |
| MACHINE_VALUES50 | "a consumer taking the record as a static parameter calls the selected declaration under its exact published contract, with reach, suspension, blocking, crash, termination, carry and result relations projected per selected field." | Static callable selections | Static-parameter consumers must call the selected declaration under its published contract with per-field projected relations | at a call | 3 |
| MACHINE_VALUES51 | "The same record schema may be materialized at runtime, where each field is the callable descriptor described above." | Static callable selections | Runtime materialization of the same schema must use callable descriptors per field | not stated | 10 |
| MACHINE_VALUES52 | "Materialization requires every selection's static arguments closed and a finite published envelope;" | Static callable selections | Runtime materialization must require closed static arguments and a finite published envelope for every selection | not stated | 10 |
| MACHINE_VALUES53 | "an open generic family has no descriptor." | Static callable selections | Open generic families must not receive a runtime callable descriptor | not stated | 7 |
| MACHINE_VALUES54 | "`machine F` in a generic list selects a declaration statically under its required contract." | Static selection remains distinct | `machine F` generic binders must select a declaration statically under required contract | not stated | 7 |
| MACHINE_VALUES55 | "It remains eligible for specialization and direct calls." | Static selection remains distinct | `machine F` selections must remain eligible for specialization and direct calls | not stated | 7 |
| MACHINE_VALUES56 | "It is not a runtime parameter and is not removed by this feature." | Static selection remains distinct | `machine F` must not be treated as a runtime parameter | not stated | 7 |
| MACHINE_VALUES57 | "Conversely, a runtime callable does not become a static generic argument merely because the optimizer predicts its target." | Static selection remains distinct | Optimizer target prediction must not turn a runtime callable into a static generic argument | not stated | 7 |
| MACHINE_VALUES58 | "Explicit generic instantiations can produce callable values once their static arguments close." | Static selection remains distinct | Callable values may be produced from explicit generic instantiations after static arguments close | not stated | 7 |
| MACHINE_VALUES59 | "Their callable contracts use the checked application's substituted [operational dependencies](effects.md#static-callback-operational-dependencies);" | Static selection remains distinct | Callable contracts from generic instantiation must use substituted operational dependencies from the checked application | not stated | 3 |
| MACHINE_VALUES60 | "construction still proves refinement against the destination callable type." | Static selection remains distinct | Construction must still prove refinement against the destination callable type | not stated | 8 |
| MACHINE_VALUES61 | "Neither spelling promises inlining, exact body counts or a specific indirect-call cost." | Static selection remains distinct | Language must not treat either spelling as promising inlining, body counts, or indirect-call cost | not stated | 9 |
| MACHINE_VALUES62 | "The baseline is an indirect callable descriptor with a fixed contract and calling shape." | Representation and component boundaries | Runtime callables baseline representation is indirect descriptor with fixed contract and calling shape | not stated | 10 |
| MACHINE_VALUES63 | "It has no semantic global roster of every assigned machine." | Representation and component boundaries | Type system must not require a semantic global roster of all assigned machines | not stated | 10 |
| MACHINE_VALUES64 | "Adding an admissible target does not change the callable type or every holder's layout." | Representation and component boundaries | Adding an admissible call target must not change callable type or layout of all holders | not stated | 9 |
| MACHINE_VALUES65 | "Native descriptor encoding is target-owned; it is not promised to be a raw pointer, a particular word count, or immune to indirect-call mitigations." | Representation and component boundaries | Native encoding is target-owned; must not promise raw pointer, fixed word count, or immunity to indirect-call mitigations | not stated | 9 |
| MACHINE_VALUES66 | "Closed-world reachability, resource checking and devirtualization still apply." | Representation and component boundaries | Closed-world reachability, resource checking, and devirtualization rules must still apply | not stated | 3 |
| MACHINE_VALUES67 | "An optimizer may prove a target set and use direct calls or finite dispatch, but that set is optimization evidence, not part of source type identity." | Representation and component boundaries | Proved target sets are optimization evidence only and must not become part of source type identity | not stated | 9 |
| MACHINE_VALUES68 | "Retain the same contract, argument correspondence, costs and code dependencies." | Representation and component boundaries | Optimized call paths must retain contract, argument correspondence, costs, and code dependencies | not stated | 3 |
| MACHINE_VALUES69 | "Local callable descriptors use within-artifact calling conventions." | Representation and component boundaries | Local callable descriptors must use within-artifact calling conventions | not stated | 9 |
| MACHINE_VALUES70 | "They do not cross an independently replaceable component boundary as unchecked pointers." | Representation and component boundaries | Local descriptors must not cross replaceable component boundaries as unchecked pointers | not stated | 9 |
| MACHINE_VALUES71 | "Use boundary requirements with their `CallPlan`/`StatePlan`, and a local proxy machine when a local callable interface is wanted." | Representation and component boundaries | Cross-boundary callable interfaces must use boundary requirements with `CallPlan`/`StatePlan` and local proxy machine | not stated | 9 |
| MACHINE_VALUES72 | "Code retention, outstanding borrows and era custody follow [component publication](../build/component_publication.md)." | Representation and component boundaries | Code retention, outstanding borrows, and era custody must follow component publication rules | not stated | 2 |
| MACHINE_VALUES73 | "An indirect representation removes roster-layout coupling; it does not make old code or state safe to unload while values still depend on it." | Representation and component boundaries | Unloading code or state must remain unsafe while callable values still depend on it | not stated | 2 |
| MACHINE_VALUES74 | "state remains ordinary explicit arguments." | Machines as runtime values | Machine state must be passed and checked as ordinary explicit arguments | not stated | 10 |
| MACHINE_VALUES75 | "[Implementations](conformances.md#implementations-are-operation-records) are records of these selections." | Static callable selections | Implementations must be operation records whose fields are static callable selections | not stated | 7 |

## Ambiguous sentences

| Sentence | Reading A | Reading B |
| --- | --- | --- |
| "Named machines can be stored in ordinary data and selected at runtime." | Any named machine declaration may appear in ordinary storage with runtime dispatch. | Only machines that meet subsequent callable-type and refinement rules qualify as storable values. |
| "Operations records replace runtime trait selectors;" | Source must use operations records instead of trait-object dispatch forms. | Runtime trait selectors are retired for dispatch only; other trait uses may remain elsewhere. |
| "Static traits, named conformances, proof laws and `machine F` generic binders remain." | Compiler must continue to accept and check these constructs unchanged by this feature. | They remain in the language specification but may interact differently with new callable values. |
| "Anonymous bodies and capture syntax remain a separate proposal." | No compiler obligation for anonymous bodies or capture in V1. | This chapter neither adopts nor rejects them; another document may bind them later. |
| "Other axes retain their existing omission rules." | Reach/crash omission triggers open-contract rejection; other contract axes follow rules in linked effects chapter only. | Omission on any axis might still reject at runtime positions if another chapter says so. |
| "In a callable-valued context, a named machine reference selects an exact closed declaration application." | Reference must resolve to one fully instantiated declaration before forming a value. | "Closed" might mean no remaining generic parameters on the declaration only, not on arguments. |
| "A qualified named implementation member is eligible;" | Any qualified conformance member is admissible if conformance is named and selection resolves. | Eligibility is limited to members listed in the operations record schema or trait schema. |
| "Unrestricted type-, machine- or evidence-polymorphic runtime callables are outside V1;" | Such callables are rejected outright in V1. | They may exist only after static arguments are closed, not as fully polymorphic values. |
| "Construction proves ordinary contract refinement" | Refinement is static proof at value construction sites only. | Refinement might be re-checked at assignments or coercions to callable types. |
| "Substitute any static callback or conformance-member reach dependencies before checking the closed selection against the destination callable's finite ceiling." | Substitution is mandatory in the stated order before ceiling comparison. | Some reach dependencies might be checked incrementally if substitution is partial. |
| "Machine values contain code selection and retained contract evidence, not an implicit state capture, receiver, allocation, raw address or new source of authority." | Values never carry hidden environment or authority beyond what refinement proves. | "Not implicit" allows explicit evidence-backed capture if another rule permits it. |
| "Known selection may narrow the operational envelope only by retained checked evidence" | Narrowing requires evidence artifact retained with the value or proof context. | "Known" might mean syntactically known at compile time versus proven equal at runtime. |
| "Possible demands union at joins; guarantees intersect." | Applies to all control-flow joins involving callable evidence. | Applies only to joins of callable values or contracts, not arbitrary control merge. |
| "mixing compatible machines from different named implementations is permitted unless a published record predicate or explicit evidence relates them." | Mixing is default-allowed with independent per-field checking. | "Compatible" requires structural callable-type compatibility, not merely different implementation names. |
| "Field completeness is not a proof of cross-operation laws, matching allocators, or a shared implementation identity." | Absence of stated cross-field laws does not block compilation of complete records. | Complete records compile but proofs cannot assume cross-field laws without extra evidence. |
| "Callable addresses do not automatically supply executable equality, ordering, hashing, or proof identity." | No builtin instances for callable addresses unless derived elsewhere. | Addresses might participate in proof identity without executable comparison operators. |
| "A stored callable's signature or absence of effects does not establish totality, termination, mathematical identity or a rewrite law." | Proposition goals need separate proofs beyond the stored type. | Absence of effects might still contribute to some proof goals defined in other chapters. |
| "It has no runtime representation:" | Static selections never lower to runtime storage or calls except via separate materialization. | No distinct runtime tag; materialized runtime descriptors are a different form. |
| "an open generic family has no descriptor." | Open families cannot be materialized to runtime callable values. | Open families might exist at compile time only without blocking other fields' materialization. |
| "a runtime callable does not become a static generic argument merely because the optimizer predicts its target." | Optimizer evidence cannot change the static/runtime classification of a parameter. | Prediction might specialize call sites without reclassifying the parameter kind. |
| "Neither spelling promises inlining, exact body counts or a specific indirect-call cost." | Language semantics impose no performance contract for either static or runtime spellings. | ABI or backend documents might still specify default costs without being language promises. |
| "Adding an admissible target does not change the callable type or every holder's layout." | Type and layout are stable across the admissible target set at the type level. | "Admissible" might mean link-time only, not source-visible additions. |
| "An optimizer may prove a target set and use direct calls or finite dispatch, but that set is optimization evidence, not part of source type identity." | Devirtualization cannot change type checking conclusions. | Optimizer-proved sets might still affect checked reach if another chapter links them. |
| "They do not cross an independently replaceable component boundary as unchecked pointers." | Cross-component call requires boundary plans and proxy, never raw descriptor bits. | Within a loaded component, descriptors might still be raw pointers at the ABI. |
| "An indirect representation removes roster-layout coupling; it does not make old code or state safe to unload while values still depend on it." | Dependency on callable values blocks unload regardless of representation. | Unload might be safe for code not referenced by live descriptors even if roster-like tables existed historically. |

## Terms used without definition in this chapter

- ordinary data
- callable field
- complete contract
- operations records
- runtime trait selectors
- static traits
- named conformances
- proof laws
- `machine F` generic binders
- anonymous bodies
- capture syntax
- structural signature
- `requires machine` requirement
- declaration name
- runtime callable type
- locals, parameters, results, fields, arrays
- ordinary containers
- formal arguments
- `result`
- lifetime binders
- `requires`, `ensures`
- reach, invocation, suspension, blocking, crash, progress
- resource and custody contracts
- fixed finite service and crash ceilings
- `reaches none`, `crashes none`
- open callable contract
- runtime position
- callable-valued context
- named machine reference
- closed declaration application
- qualified named implementation member
- conformance
- ordinary selection
- unique visible implementation
- implicit whole-trait conversion
- type-, machine- or evidence-polymorphic runtime callables
- V1
- explicitly finite value families
- declared coverage
- contract refinement
- operational/resource/custody bounds
- static callback
- conformance-member reach dependencies
- finite ceiling
- code selection
- retained contract evidence
- implicit state capture
- receiver
- new source of authority
- null/default callable
- unchecked integer-to-callable cast
- exact argument/result mapping
- preconditions and access obligations
- permitted effects and progress
- checked postconditions
- returned-borrow origins
- state arguments
- forwarding, reassignment, joins
- contract evidence
- known selection
- operational envelope
- retained checked evidence
- declared ceiling
- possible demands / guarantees
- reusable body
- optimizer's target prediction
- code-only value
- complete qualifications and custody
- code lifetime and replacement dependencies
- mutable state references
- ordinary multiplicity
- operations records (predicate)
- published record predicate
- explicit evidence
- cross-operation laws
- matching allocators
- shared implementation identity
- static trait laws
- trait schema
- callable addresses
- executable equality, ordering, hashing
- proof identity
- conformance synthesis
- published derivation
- descriptors
- hidden witness tags
- proposition contexts
- proof admission rules
- totality, termination, mathematical identity, rewrite law
- static callable selection
- static arguments
- remaining generic parameters
- binding to a named requirement
- ordinary static record
- compile-time evaluation
- canonical static atom
- behavioral equivalence
- runtime representation
- static parameter
- carry and result relations
- materialization
- callable descriptor
- finite published envelope
- open generic family
- specialization
- direct calls
- runtime parameter
- explicit generic instantiations
- operational dependencies
- inlining
- exact body counts
- indirect-call cost
- indirect callable descriptor
- fixed contract and calling shape
- semantic global roster
- admissible target
- holder's layout
- native descriptor encoding
- raw pointer
- indirect-call mitigations
- closed-world reachability
- resource checking
- devirtualization
- optimization evidence
- source type identity
- argument correspondence
- costs and code dependencies
- within-artifact calling conventions
- independently replaceable component boundary
- unchecked pointers
- boundary requirements
- `CallPlan`/`StatePlan`
- local proxy machine
- code retention
- outstanding borrows
- era custody
- component publication (linked)
- roster-layout coupling

CHAPTER COMPLETE
