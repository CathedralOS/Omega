# Assumed binding contracts

Source: `wiki/spec/build/assumed_binding_contracts.md` lines 1–253 (read).

## Rules

rule id | the exact sentence, quoted | the heading it is under | what must be checked, in plain words | when it must hold | lane
--- | --- | --- | --- | --- | ---
ASSUMED_BINDING_CONTRACT1 | "**Ordinary Omega programs should reject assumed bindings outright.**" | Assumed binding contracts | Programs classified as ordinary must not be accepted when they use assumed bindings | not stated | 9
ASSUMED_BINDING_CONTRACT2 | "They are not a workaround for borrow-checker errors, missing proofs in ordinary code, incorrect contracts, or compiler limitations." | Assumed binding contracts | Assumed bindings must not be treated as permitting compilation when ordinary borrow, proof, contract, or compiler-gap failures would otherwise apply | not stated | SHARED
ASSUMED_BINDING_CONTRACT3 | "**Do not introduce an assumed binding merely to make rejected code compile.**" | Assumed binding contracts | An assumed binding introduced only to compile otherwise-rejected code must not be accepted | not stated | 9
ASSUMED_BINDING_CONTRACT4 | "Libraries must not pass these problems downstream by asking consumers to approve an assumed binding." | Assumed binding contracts | Library builds must not rely on downstream consumers approving an assumed binding to resolve upstream errors | not stated | 9
ASSUMED_BINDING_CONTRACT5 | "Root authorization remains mandatory, together with exact receiving/project acceptance where package acceptance applies." | Assumed binding contracts | Root authorization and applicable receiving/project acceptance must be present before the relationship is admitted | not stated | 9
ASSUMED_BINDING_CONTRACT6 | "This mechanism does not add a general bytes-to-code installation route or waive the independent structural and operational checks below." | Assumed binding contracts | Bytes-to-code installation must not bypass the structural and operational checks described in this chapter | not stated | 9
ASSUMED_BINDING_CONTRACT7 | "The first ordinary argument is the mutable Build context; namespace qualification supplies no implicit receiver." | Assumed binding contracts | `assume_binding_contract` must use a mutable Build context as the first ordinary argument without an implicit receiver from namespace qualification | at a call | 9
ASSUMED_BINDING_CONTRACT8 | "The receiver-style spelling is retired." | Assumed binding contracts | Receiver-style spelling of the operation must be rejected | at a call | 9
ASSUMED_BINDING_CONTRACT9 | "This is a typed Build operation, not a language keyword, unchecked body, or cast between `Binding` values." | Assumed binding contracts | The operation must be classified and checked only as the typed Build operation, not as keyword, unchecked body, or `Binding` cast | at a call | 9
ASSUMED_BINDING_CONTRACT10 | "It records a runtime installation rule." | Assumed binding contracts | The operation must record a runtime installation rule rather than only a compile-time fact | at a call | 9
ASSUMED_BINDING_CONTRACT11 | "Both requirements retain their own identities and contracts; the actual implementation remains checked against the supplied requirement." | Assumed binding contracts | Required and supplied requirements keep separate identities and contracts; the implementation body must still be checked against the supplied requirement | not stated | 3
ASSUMED_BINDING_CONTRACT12 | "The assumption belongs to the relationship at this crossing, not to that implementation on every call." | Assumed binding contracts | The semantic assumption must be attached only to this installed crossing, not globally to every invocation of the implementation | not stated | 3
ASSUMED_BINDING_CONTRACT13 | "The domain must authorize the exact claim requirement as an issuer." | Caller, implementation, and root | The domain named in the example pattern must authorize the exact claim requirement as issuer before the claim is valid | not stated | 8
ASSUMED_BINDING_CONTRACT14 | "The admitted caller-facing crossing establishes the qualification on the exact returned subject." | Caller, implementation, and root | Through the admitted caller-facing link, the returned subject must receive the established qualification | at normal return | 2
ASSUMED_BINDING_CONTRACT15 | "Calling the supplier directly does not." | Caller, implementation, and root | A direct call to the supplier must not establish that qualification on the returned subject | at a call | 3
ASSUMED_BINDING_CONTRACT16 | "Ordinary `satisfies WritableMemory::claim` would still require proof of the complete contract and is not synthesized by this link." | Caller, implementation, and root | A `satisfies` edge for the required operation must not be synthesized from the assumed link without full contract proof | not stated | 8
ASSUMED_BINDING_CONTRACT17 | "Memory instructions and ordinary code retain their complete checks relative to the admitted premises; no `unsafe` block, raw-instruction exception, or `admit_implementation` facility is introduced." | Caller, implementation, and root | Checks in memory instructions and ordinary code must remain complete relative to admitted premises; must not introduce `unsafe`, raw-instruction exception, or `admit_implementation` | not stated | SHARED
ASSUMED_BINDING_CONTRACT18 | "Resolve both static operands to exact requirements/applications under ordinary visibility and package rules." | Compatibility and direction | Both type arguments must resolve to exact requirements/applications under visibility and package rules | at a call | 9
ASSUMED_BINDING_CONTRACT19 | "The supplied side must have one selected complete provider and executable realization." | Compatibility and direction | The supplied requirement must have exactly one selected complete provider with executable realization | at a call | 9
ASSUMED_BINDING_CONTRACT20 | "Equal spelling, a runtime address, or a signature alone cannot select it." | Compatibility and direction | Provider selection must not rely on spelling, runtime address, or signature alone | at a call | 9
ASSUMED_BINDING_CONTRACT21 | "Static type/const/conformance arguments and lifetime correspondence remain explicit parts of the checked relationship." | Compatibility and direction | Type, const, conformance arguments and lifetime correspondence must be explicit in the checked relationship | not stated | 7
ASSUMED_BINDING_CONTRACT22 | "Same count, order, and underlying runtime carriers; parameter names may differ." | Compatibility and direction | Parameters and results must match in count, order, and underlying runtime carriers | not stated | 3
ASSUMED_BINDING_CONTRACT23 | "Preserve owned values, shared, mutable and write-only borrows, and their explicit subject correspondence." | Compatibility and direction | Passing modes must preserve owned values, borrows, and explicit subject correspondence | not stated | 2
ASSUMED_BINDING_CONTRACT24 | "Same validated runtime layout and calling plan; equal byte size alone is insufficient." | Compatibility and direction | Representation must match validated runtime layout and calling plan, not merely byte size | not stated | 3
ASSUMED_BINDING_CONTRACT25 | "Preserve transfers, borrow relationships, multiplicity and cleanup; no silent copy, drop or lifetime extension." | Compatibility and direction | Ownership and lifetimes must preserve transfers, borrows, multiplicity, cleanup; must reject silent copy, drop, or lifetime extension | not stated | 2
ASSUMED_BINDING_CONTRACT26 | "Differences in propositions, erased qualifications and authorized provenance need checked evidence or an exact root assumption." | Compatibility and direction | Semantic contract differences must have checked evidence or an exact recorded root assumption | not stated | 8
ASSUMED_BINDING_CONTRACT27 | "Check reach, suspension, blocking, crashes, invocation and termination compatibility; retain actual transitive implementation dependencies." | Compatibility and direction | Operational contracts must be checked for reach, suspension, blocking, crashes, invocation, termination, and transitive implementation dependencies | not stated | 3
ASSUMED_BINDING_CONTRACT28 | "A representation-changing qualification cannot be ignored as erased evidence." | Compatibility and direction | Representation-changing qualifications must not be treated as ignorable erased evidence | not stated | 8
ASSUMED_BINDING_CONTRACT29 | "Different runtime shapes require an ordinary checked adapter with a matching entry; the build operation does not reorder arguments, recast scalar bits, or use an ABI trampoline to disguise incompatible contracts." | Compatibility and direction | Incompatible runtime shapes must use a checked adapter; the build operation must not reorder args, recast bits, or use ABI trampolines to disguise incompatibility | not stated | 3
ASSUMED_BINDING_CONTRACT30 | "Each parameter/result is mapped exactly once." | Compatibility and direction | Each parameter and result must map exactly once in the relationship | not stated | 3
ASSUMED_BINDING_CONTRACT31 | "entry: caller preconditions + admitted entry premises establish implementation preconditions" | Compatibility and direction | At entry, caller preconditions plus admitted entry premises must establish implementation preconditions | at a call | 8
ASSUMED_BINDING_CONTRACT32 | "exit: implementation postconditions + admitted exit premises establish caller postconditions" | Compatibility and direction | At exit, implementation postconditions plus admitted exit premises must establish caller postconditions | at normal return | 8
ASSUMED_BINDING_CONTRACT33 | "Admission identifies each exact subject and missing fact; it is not an ambient assertion that every value of that carrier is valid." | Compatibility and direction | Admission must name exact subjects and missing facts and must not assert carrier-wide validity | not stated | 8
ASSUMED_BINDING_CONTRACT34 | "Routed authority still needs its domain-owner-authorized establishment route and exact entry/result receipt." | Compatibility and direction | Routed authority must use domain-owner-authorized establishment and exact entry/result receipt | at a call; at normal return | 8
ASSUMED_BINDING_CONTRACT35 | "Fresh claims are explicit admitted issuance, not unrecorded duplication of a transferred claim." | Compatibility and direction | Fresh claims must be explicit admitted issuances, not silent duplication of transferred claims | not stated | 2
ASSUMED_BINDING_CONTRACT36 | "It must not be used to repair a failed proof or conformance in ordinary code." | Known contradictions at the trust crossing | Assumed binding must not repair failed proof or conformance in ordinary code | not stated | 8
ASSUMED_BINDING_CONTRACT37 | "An explicitly authorized and accepted `assume_binding_contract` relationship may assume its semantic connection even when the checker can refute that connection (`assumed-binding-refutable-claim`)." | Known contradictions at the trust crossing | With authorization and acceptance, the semantic connection may be recorded even when the checker refutes it | not stated | 8
ASSUMED_BINDING_CONTRACT38 | "The caller checks against the required contract; the implementation checks against the supplied contract." | Known contradictions at the trust crossing | Caller-side checking must use the required contract; implementation-side checking must use the supplied contract | at a call | 3
ASSUMED_BINDING_CONTRACT39 | "The root assumes the connection between them, not correctness of an unchecked body." | Known contradictions at the trust crossing | The recorded assumption is the cross-contract connection, not unchecked-body correctness | not stated | 8
ASSUMED_BINDING_CONTRACT40 | "Neither the assumption nor conclusions derived from it can waive runtime-shape, passing-mode, ABI, ownership/lifetime mapping, operational compatibility, either body's own checking, or the build-time prohibition." | Known contradictions at the trust crossing | Assumption and derived conclusions must not waive listed structural, operational, body-check, or build-time rules | not stated | SHARED
ASSUMED_BINDING_CONTRACT41 | "Those checks remain independent of the assumed semantic connection." | Known contradictions at the trust crossing | Listed checks must run independently of the assumed semantic link | not stated | SHARED
ASSUMED_BINDING_CONTRACT42 | "Record a detected contradiction with the precise conflicting statements, subjects and facts establishing the refutation." | Known contradictions at the trust crossing | When a contradiction is detected, record precise conflicting statements, subjects, and refutation facts | not stated | 8
ASSUMED_BINDING_CONTRACT43 | "Report it through the existing extremely-high-risk review; the operation needs no separate contradiction flag or new escape form." | Known contradictions at the trust crossing | Contradictions must go through extremely-high-risk review without a separate contradiction flag or escape form | not stated | 9
ASSUMED_BINDING_CONTRACT44 | "Failure to prove a relationship is not a refutation, and failure to find a contradiction establishes neither correctness nor consistency." | Known contradictions at the trust crossing | Absence of proof must not be treated as refutation; absence of detected contradiction must not be treated as correctness or consistency | not stated | 8
ASSUMED_BINDING_CONTRACT45 | "Checked callers and optimizers may rely on the caller-facing contract." | Known contradictions at the trust crossing | Optimizers and checked callers may use the caller-facing contract for reasoning | not stated | 3
ASSUMED_BINDING_CONTRACT46 | "admission does not promise that a caller observes `0`, traps, or detects the lie." | Known contradictions at the trust crossing | Admission must not be modeled as promising particular runtime observations when the assumption is false | not stated | 3
ASSUMED_BINDING_CONTRACT47 | "Artifact guarantees remain conditional on the recorded assumption." | Known contradictions at the trust crossing | Artifact guarantees must be conditioned on the recorded assumption | not stated | 9
ASSUMED_BINDING_CONTRACT48 | "Its falsity invalidates those guarantees; no unconditional theorem that `0 == 1` or checked conformance is created." | Known contradictions at the trust crossing | False assumptions must invalidate conditional guarantees and must not create unconditional false theorems or checked conformance | not stated | 8
ASSUMED_BINDING_CONTRACT49 | "Evidence checking verifies derivations and assumption scope, not the truth of the root's assertion." | Known contradictions at the trust crossing | Evidence checking must verify derivations and assumption scope only, not truth of the root assertion | not stated | 8
ASSUMED_BINDING_CONTRACT50 | "The link supplies one caller-facing operation, not an inferred whole-trait conformance." | Selection and retained evidence | The link must cover one caller-facing operation, not infer whole-trait conformance | not stated | 3
ASSUMED_BINDING_CONTRACT51 | "Every required member still needs exactly one supply." | Selection and retained evidence | Each required trait member must have exactly one supply | not stated | 9
ASSUMED_BINDING_CONTRACT52 | "Explicit root selection outranks target defaults under ordinary provider precedence." | Selection and retained evidence | Explicit root selection must take precedence over target defaults per ordinary provider precedence | not stated | 9
ASSUMED_BINDING_CONTRACT53 | "Competing explicit checked and assumed supplies for the same operation reject rather than silently preferring the assumption." | Selection and retained evidence | Competing checked and assumed supplies for one operation must reject compilation | not stated | 9
ASSUMED_BINDING_CONTRACT54 | "Follow any supplied-side links to an actual selected implementation, retaining all assumptions; cyclic binding aliases reject." | Selection and retained evidence | Supplied-side links must resolve to a selected implementation with assumptions retained; cycles must reject | not stated | 9
ASSUMED_BINDING_CONTRACT55 | "Retain both contracts and their exact applications, positional subject mapping, proved versus assumed obligations, domain establishment authority, selected provider/body and its semantic dependencies, calling/layout identity, root grant, and runtime-only classification." | Selection and retained evidence | The listed contract, mapping, obligation, authority, provider, layout, grant, and classification facts must be retained | not stated | SHARED
ASSUMED_BINDING_CONTRACT56 | "Binding identity cannot hide the actual code being invoked." | Selection and retained evidence | Binding identity must expose the actual invoked code | not stated | 3
ASSUMED_BINDING_CONTRACT57 | "Detected semantic contradictions and their supporting facts remain attached to the assumed relationship." | Selection and retained evidence | Contradictions and supporting facts must stay attached to the assumed relationship | not stated | 8
ASSUMED_BINDING_CONTRACT58 | "Inlining, wrappers, normalization, and erasure cannot turn the relationship into a checked conformance or remove its transitive trust record." | Selection and retained evidence | Transforms must not convert the link to checked conformance or drop the transitive trust record | not stated | 3
ASSUMED_BINDING_CONTRACT59 | "Reuse ordinary call and result custody; do not add a parallel compiler pipeline, opaque instruction path, or package-owned verifier." | Selection and retained evidence | Call/result custody must use ordinary paths without a parallel pipeline, opaque instruction path, or package-owned verifier | not stated | 3
ASSUMED_BINDING_CONTRACT60 | "Checking establishes the structural obligations and records the assumption under the root selection; it does not certify the assumption's truth." | Selection and retained evidence | Checking must record structural obligations and assumption under root selection without certifying assumption truth | not stated | 8
ASSUMED_BINDING_CONTRACT61 | "Only the consuming runtime root may authorize the relationship." | Runtime only | Authorization must come only from the consuming runtime root | not stated | 9
ASSUMED_BINDING_CONTRACT62 | "Dependencies cannot authorize themselves, and this operation cannot widen delegated build services." | Runtime only | Dependencies must not self-authorize; the operation must not widen delegated build services | not stated | 9
ASSUMED_BINDING_CONTRACT63 | "Root authorization and [project acceptance](../packages/acceptance.md#assumed-binding-contract-acceptance) are both required where package acceptance applies." | Runtime only | Root authorization and project acceptance must both be present when package acceptance applies | not stated | 9
ASSUMED_BINDING_CONTRACT64 | "`build.omg` may register the runtime relationship, but receives no callable binding or proposition evidence from registration." | Runtime only | Registration in `build.omg` must not create a callable binding or proposition evidence | not stated | 9
ASSUMED_BINDING_CONTRACT65 | "Execution through that link, or reliance on its assumptions to justify build-time computation, rejects in build entries, imported helpers, generators, constants, proof/layout evaluation, and automatic build-time tests." | Runtime only | Build-time execution through the link or use of its assumptions for build-time computation must reject in listed build contexts | not stated | 9
ASSUMED_BINDING_CONTRACT66 | "The prohibition follows the selected call and trust closure, including erased premises; neither a host-service grant nor a root flag can override it." | Runtime only | Build-time prohibition must follow call and trust closure including erased premises; host-service grant and root flag must not override | not stated | 9
ASSUMED_BINDING_CONTRACT67 | "Both static requirement operands use the existing [non-executing product-reference rules](scoped_execution.md#selecting-product-declarations-without-executing-them)." | Runtime only | Static requirement operands must follow non-executing product-reference rules | at a call | 9
ASSUMED_BINDING_CONTRACT68 | "Resolving them does not import the product implementation into the build-host execution closure." | Runtime only | Resolving operands must not import product implementation into build-host execution closure | not stated | 9
ASSUMED_BINDING_CONTRACT69 | "The complete registered relationship is still reviewed before runtime installation; mere visibility is not acceptance." | Runtime only | Full relationship must be reviewed before runtime installation; visibility alone must not count as acceptance | not stated | 9
ASSUMED_BINDING_CONTRACT70 | "Independently checked calls to the underlying supplier can remain build-admissible if they do not traverse or rely on the assumed link." | Runtime only | Supplier calls may be build-admissible only when they do not traverse or rely on the assumed link | at a call | 9
ASSUMED_BINDING_CONTRACT71 | "A runtime interpreter may install an accepted relationship under its runtime policy; being interpreted does not turn runtime execution into build execution or make guest address IDs into host pointers." | Runtime only | Interpreter install under runtime policy must not reclassify runtime as build execution or guest IDs as host pointers | not stated | 10
ASSUMED_BINDING_CONTRACT72 | "Package install, update and audit label every such relationship **Extremely high risk - assumed binding contract**." | Review and delivery | Package install, update, and audit must label each relationship with the stated risk label | not stated | 9
ASSUMED_BINDING_CONTRACT73 | "Ordinary applications should refuse the relationship, including when a library requests it." | Review and delivery | Ordinary applications must refuse the relationship when a library requests it | not stated | 9
ASSUMED_BINDING_CONTRACT74 | "Changed relationship or implementation identity needs fresh explicit acceptance." | Review and delivery | Identity changes must require fresh explicit acceptance | not stated | 9
ASSUMED_BINDING_CONTRACT75 | "A dependency's own lock or an ordinary provider selection is not that decision." | Review and delivery | Dependency lock or ordinary provider selection must not substitute for explicit acceptance | not stated | 9
ASSUMED_BINDING_CONTRACT76 | "Preserve ordinary refutable-grant rejection, rejected shape/mode/custody mismatches, root-only acceptance, transitive audit and update invalidation, and build-time refusal through helpers, provider dispatch and erased evidence." | Review and delivery | Listed ordinary rejections, acceptance rules, audit invalidation, and build-time refusal paths must be preserved | not stated | SHARED
ASSUMED_BINDING_CONTRACT77 | "Assembly keeps its existing checked contract." | Review and delivery | Assembly must remain under its existing checked contract without change from this mechanism | not stated | 10

## Ambiguous sentences

Sentence | Reading A | Reading B
--- | --- | ---
"**Ordinary Omega programs should reject assumed bindings outright.**" | The compiler must reject any ordinary program that contains an assumed binding. | "Should" expresses policy guidance; mandatory rejection is not stated with "must" in this sentence. | 
"They remain available for exceptional programs that deliberately exceed the checked model, including some forms of self-modifying code." | Exceptional programs may compile with assumed bindings when other stated authorization and checks are satisfied. | "Available" means the language mechanism exists, without stating that compilation succeeds without further conditions named elsewhere. |
"The admitted caller-facing crossing establishes the qualification on the exact returned subject." | Only values returned through the installed caller-facing operation at the crossing receive the qualification. | Any return on code paths attributed to the crossing receives the qualification, and "exact returned subject" leaves scoping of attribution unstated in this sentence. |
"Explicit root selection outranks target defaults under ordinary provider precedence." | Whenever root selection is explicit, it wins over target defaults per ordinary provider precedence rules. | Outranking applies only in cases where both explicit root selection and target defaults are present under those precedence rules; the sentence does not state behavior when selection is implicit. |
"Ordinary applications should refuse the relationship, including when a library requests it." | Refusal is enforced at package acceptance or install/audit review for ordinary applications. | Refusal is a compile-time rejection when an ordinary application root links or retains the relationship. |
"`build.omg` may register the runtime relationship, but receives no callable binding or proposition evidence from registration." | Registration does not make the relationship callable or usable as proposition evidence during build. | Registration creates no binding or evidence artifacts at all for any phase, including runtime. |
"being interpreted does not turn runtime execution into build execution or make guest address IDs into host pointers." | Running under a runtime interpreter keeps build-time prohibitions from applying to guest runtime execution. | Build-host actions performed while hosting an interpreter might still be build execution even when guest code is interpreted; the sentence speaks only to classification of "runtime execution" and address IDs. |
"Failure to prove a relationship is not a refutation, and failure to find a contradiction establishes neither correctness nor consistency." | The checker must not treat unproved relationships as refuted or absence of detected contradiction as proof of correctness/consistency. | The first clause governs proof obligations; the second governs metadata attached to review; whether both bind the same pipeline stage is not stated. |
"An exceptional use must identify the concrete program requirement outside the checked model and the guarantees being relinquished." | The compiler or acceptance workflow must require documented identification before admitting the link. | This is a human/documentation obligation on the author; the compiler need not enforce identification beyond stated authorization checks. |
"A compiler rejection alone is not that justification." | Installing an assumed binding after a prior compiler rejection requires separate justification beyond the rejection message. | Any compilation failure, regardless of cause, cannot justify assumed binding without the exceptional-use identification named earlier. |

## Terms used without definition in this chapter

- assumed binding / assumed bindings / assumed binding contract
- ordinary Omega program / ordinary program / ordinary code / ordinary applications
- exceptional program / exceptional use
- checked model
- borrow-checker errors (as a category)
- root / consuming root / consuming runtime root / Root authorization
- Build / Build context / `builder`
- boundary trait requirement
- semantic relationship
- ordinary conformance checking
- `Binding` values
- runtime installation rule
- receiving/project acceptance / package acceptance / project acceptance
- requirement / application (static operands)
- provider / complete provider / executable realization / provider precedence
- Range / Writable / WritableMemory / RangeSupplier (example names)
- qualification / erased qualifications
- domain / domain-owner / issuer
- admitted entry premises / admitted exit premises
- authority and custody obligations (referenced)
- refutable-claim / `assumed-binding-refutable-claim`
- extremely-high-risk review
- caller-facing contract / caller-facing operation
- transitive trust record / trust closure
- non-executing product-reference rules
- build-host execution closure
- build entries
- host-service grant / root flag
- product implementation
- runtime-only classification
- lock model / compact readable risk row
- Psi / Omega (as pipeline owners)
- erased premises / erased evidence
- positional subject mapping
- proved versus assumed obligations
- domain establishment authority
- root grant
- semantic dependencies
- guest address IDs
- build-time computation
- automatic build-time tests
- reflexive "self-modifying code" category boundary

CHAPTER COMPLETE

