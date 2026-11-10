# Lifetimes and carried borrows

Source: `wiki/spec/language/lifetimes.md` lines 1–106 (read).

## Rules

rule id | the exact sentence, quoted | the heading it is under | what must be checked, in plain words | when it must hold | lane
--- | --- | --- | --- | --- | ---
LIFETIMES1 | "A lifetime relates a borrowed result or stored view to storage that remains valid for its use." | Lifetimes and carried borrows | Each lifetime must relate borrowed results and stored views to storage that stays valid for their use | not stated | 2
LIFETIMES2 | "It does not create a loan, widen access, or identify a referent by itself." | Lifetimes and carried borrows | A lifetime annotation alone must not be treated as creating a loan, widening access, or identifying a referent | not stated | 2
LIFETIMES3 | "[Loan resources](../terminal-psi/loans.md) retain the exact captured place, lineage, formation and restoration evidence separately." | Lifetimes and carried borrows | Loan resources must retain captured place, lineage, and formation/restoration evidence separately from lifetime spelling | not stated | 2
LIFETIMES4 | "A `some` binder hides a type choice, not a reference lifetime." | Lifetimes and carried borrows | A `some` binder must not be interpreted as hiding a reference lifetime | not stated | 7
LIFETIMES5 | "Packages keep ordinary lifetime parameters or permitted elision;" | Lifetimes and carried borrows | Packages must use ordinary lifetime parameters or only permitted elision | not stated | 7
LIFETIMES6 | "their fields and concrete returned borrows remain tied to the same input/storage origins." | Lifetimes and carried borrows | Package fields and concrete returned borrows must stay tied to the same input or storage origins | at normal return | 2
LIFETIMES7 | "Opening or repacking cannot extend those lifetimes." | Lifetimes and carried borrows | Opening or repacking must not extend lifetimes beyond those origins | not stated | 2
LIFETIMES8 | "Lifetime binders use tick spelling in the declaration's `<>` list alongside type, const and static-machine parameters." | Binders and source applications | Lifetime binders must use tick spelling in the declaration `<>` list with type, const, and static-machine parameters | not stated | 7
LIFETIMES9 | "A reference places its lifetime before its access modifier: `&'buffer T`, `&'buffer mut T`, or `&'buffer write T`." | Binders and source applications | References must place the lifetime before the access modifier in the stated forms | not stated | 2
LIFETIMES10 | "Borrow-carrying data declares the lifetimes of its views:" | Binders and source applications | Borrow-carrying data must declare lifetimes for its contained views | not stated | 1
LIFETIMES11 | "Named data applications supply lifetime arguments before type/const/machine arguments." | Binders and source applications | Named data applications must supply lifetime arguments before type, const, and machine arguments | at a call | 7
LIFETIMES12 | "They must match the declaration's lifetime arity and use binders declared in the lexical owner." | Binders and source applications | Lifetime arguments must match declaration arity and binders declared in the lexical owner | at a call | 7
LIFETIMES13 | "Duplicate binders and undeclared tags reject." | Binders and source applications | Duplicate lifetime binders or undeclared lifetime tags must reject the program | not stated | 7
LIFETIMES14 | "Lifetime arguments are erased regions, separate from runtime generic arity, layout and monomorphization." | Binders and source applications | Lifetime arguments must be treated as erased regions distinct from runtime generic arity, layout, and monomorphization | not stated | 7
LIFETIMES15 | "Erasure does not remove semantic lifetime relations or loan obligations." | Binders and source applications | After erasure, semantic lifetime relations and loan obligations must remain in force | not stated | 2
LIFETIMES16 | "[Conformance telescopes](conformances.md#declaration-and-selection) have their own normalized public mapping and application-elision rules." | Binders and source applications | Conformance telescopes must use their separate normalized public mapping and application-elision rules | not stated | 7
LIFETIMES17 | "Static callable requirements may additionally bind fresh lifetimes for each invocation under [generic family matching](generics.md#invocation-lifetime-families)." | Binders and source applications | Static callable requirements may bind fresh lifetimes per invocation only under generic family matching | at a call | 7
LIFETIMES18 | "These are distinct from captured-environment lifetimes." | Binders and source applications | Invocation-family lifetimes must not be conflated with captured-environment lifetimes | at a call | 2
LIFETIMES19 | "The rule permits repeated reborrowing and requires universal requirement refinement;" | Binders and source applications | Repeated reborrowing under the rule must be permitted only with universal requirement refinement | at a call | 3
LIFETIMES20 | "it does not introduce general authored outlives bounds or higher-ranked whole-conformance arguments." | Binders and source applications | This rule must not be read as supplying general authored outlives bounds or higher-ranked whole-conformance arguments | not stated | 7
LIFETIMES21 | "An explicit result lifetime relates its reference leaves to the corresponding input lifetime." | Returned views | An explicit result lifetime must relate each reference leaf to the corresponding input lifetime | at normal return | 2
LIFETIMES22 | "The source must supply the result's declared access and remain live for the result's use." | Returned views | The chosen source must provide the declared access and remain live for uses of the result | at normal return | 2
LIFETIMES23 | "A signature does not authorize returning unrelated storage merely because its type has the same lifetime spelling." | Returned views | Programs must reject returning unrelated storage when only lifetime spelling matches the signature | at normal return | 2
LIFETIMES24 | "An admitted foreign boundary may establish a provider-owned reference under its complete safe contract, including through an output slot." | Returned views | At an admitted foreign boundary, a provider-owned reference may be established only under the complete safe contract | at normal return | 9
LIFETIMES25 | "Its backing need not be contained in the lifetime-related input object:" | Returned views | Provider-owned backing must not be required to lie inside the lifetime-related input object | at normal return | 2
LIFETIMES26 | "authority comes from [provider admission](../build/foreign_storage.md#provider-owned-views-and-completion), not from the lifetime spelling." | Returned views | Authority for such results must come from provider admission, not lifetime spelling alone | at normal return | 9
LIFETIMES27 | "Ordinary checked machines still establish their result's backing and access." | Returned views | Non-foreign checked machines must establish backing and access for their results | at normal return | 3
LIFETIMES28 | "For an elided output, one unambiguous reference input supplies the relation." | Returned views | Elided output lifetimes may be supplied only when exactly one reference input is unambiguous | at normal return | 2
LIFETIMES29 | "A qualified machine name, parameter name, or first-argument position grants no priority among possible sources." | Returned views | Name or position must not break ties among possible lifetime sources | at normal return | 2
LIFETIMES30 | "Several unnamed carried references do not become one source merely because they occupy one parameter." | Returned views | Multiple unnamed carried references in one parameter must not be merged into a single lifetime source | at normal return | 2
LIFETIMES31 | "Ambiguous elision requires an explicit relation, not a guess by name or position." | Returned views | Ambiguous lifetime elision must reject unless an explicit relation is written | at normal return | 2
LIFETIMES32 | "Returned carriers preserve their complete structural lifetime frontier through nested records, active sum payloads, fixed arrays, qualifications and concrete generic arguments." | Returned views | Returned borrow-carrying values must preserve the full structural lifetime frontier through nested structure | at normal return | 1
LIFETIMES33 | "A field's output path retains its corresponding source paths and access; matching input/output field names are not required." | Returned views | Each output field path must retain matching source paths and access without requiring identical field names | at normal return | 1
LIFETIMES34 | "Within a selected input, every reference leaf carrying the selected lifetime remains a possible source unless exact correspondence narrows it." | Returned views | Reference leaves with the selected lifetime in the input remain possible sources until exact correspondence narrows the set | at normal return | 2
LIFETIMES35 | "Each possible source must support the returned access." | Returned views | Every remaining possible source must be checked to support the returned access mode | at normal return | 2
LIFETIMES36 | "A direct reference returned from an owned borrow-carrying input follows the same rule without an enclosing output path." | Returned views | Direct references from owned borrow-carrying inputs must follow the same source and access rules without an output path | at normal return | 2
LIFETIMES37 | "The result carries loans into the original backing, not a borrow of the caller's private carrier storage." | Returned views | Result loans must target original backing storage, not the caller's private carrier | at normal return | 2
LIFETIMES38 | "Generic returned views require the complete instantiated frontier and its result-to-input relation." | Returned views | Generic returned views must have a complete instantiated frontier and a checked result-to-input relation | at normal return | 7
LIFETIMES39 | "A template-dependent frontier is not an empty one;" | Returned views | A template-dependent lifetime frontier must not be treated as empty | at normal return | 7
LIFETIMES40 | "discarding the result cannot excuse missing call admission." | Returned views | Discarding a generic result must not waive required call admission | at a call | 3
LIFETIMES41 | "General outlives syntax and broader multiple-source result relations are not supplied by the single-source elision rule." | Returned views | Single-source elision must not be used to supply general outlives syntax or multi-source result relations | at normal return | 2
LIFETIMES42 | "Unresolved loans cannot be omitted." | Returned views | Unresolved loans on returned views must not be omitted from checking | at normal return | 2
LIFETIMES43 | "Moving a borrow-carrying value transfers its exact contained loan paths and access." | Carried-loan transport | Moves must transfer exact contained loan paths and access unchanged | at the operation | 2
LIFETIMES44 | "Constructing an enclosing field or array element prefixes those paths;" | Carried-loan transport | Field or array-element construction must prefix loan paths without changing their targets | at the operation | 2
LIFETIMES45 | "it does not merge unrelated siblings." | Carried-loan transport | Construction must not merge loan paths of unrelated sibling fields or elements | at the operation | 2
LIFETIMES46 | "The leaves of one value formed together are held at once, so they obey the compatibility of one call's operands: an exclusive or write-only leaf must not overlap another leaf of that value." | Carried-loan transport | Co-formed reference leaves must satisfy operand compatibility; exclusive or write-only leaves must not overlap other leaves of the same value | at a call | 2
LIFETIMES47 | "Projecting a field retains that field's loans." | Carried-loan transport | Field projection must retain that field's loans | at the operation | 2
LIFETIMES48 | "A fixed index can retain its element's loans, while an unresolved dynamic selection conservatively retains all possible sources." | Carried-loan transport | Fixed indexing may retain element loans; unresolved dynamic indexing must conservatively retain all possible sources | at the operation | 2
LIFETIMES49 | "Assignment evaluates the replacement while old loans remain active, ends the overwritten field's carried loans, then installs the replacement's exact loans." | Carried-loan transport | Assignment must evaluate the replacement under active old loans, end overwritten carried loans, then install the replacement's exact loans | at the operation | 0
LIFETIMES50 | "If an owned move already emptied the field under a [borrowed-storage invariant window](ownership.md#borrowed-storage-invariant-windows), its contained loans moved with the removed value; filling the hole does not end those loans a second time." | Carried-loan transport | After an owned move under a borrowed-storage invariant window, loans must not be ended again when filling the emptied field | at the operation | 2
LIFETIMES51 | "The store installs the replacement's loans and closes the exact place's restoration obligation, without ending the enclosing exclusive loan prematurely." | Carried-loan transport | The store must install replacement loans and close the exact place's restoration obligation without ending an enclosing exclusive loan early | at the operation | 2
LIFETIMES52 | "Replacing one field neither preserves its former source nor releases a sibling's loans." | Carried-loan transport | Field replacement must not preserve the old field's sources or release sibling loans | at the operation | 2
LIFETIMES53 | "Domain-only casts, including removal of non-owning qualifications, preserve source places and access." | Carried-loan transport | Domain-only casts must preserve source places and access | at the operation | 5
LIFETIMES54 | "[Representation recasts](../layouts/recasts.md) instead require the complete validated footprint and overlap judgment." | Carried-loan transport | Representation recasts must use a complete validated footprint and overlap judgment | at the operation | 1
LIFETIMES55 | "Transitions may transport references only while their storage outlives the target path." | Carried-loan transport | Transitions carrying references must be rejected unless storage outlives the target path | on every edge | 10
LIFETIMES56 | "Persistent fields retain exact field/case/index provenance across control-flow edges only under valid source correspondence;" | Carried-loan transport | Persistent fields may keep field, case, or index provenance across edges only with valid source correspondence | on every edge | 10
LIFETIMES57 | "a coincident name or runtime index cannot supply it." | Carried-loan transport | Coincident names or runtime indices must not supply source correspondence | on every edge | 10
LIFETIMES58 | "Reference copies still denote live storage, unlike integer snapshots." | Carried-loan transport | Reference copies must continue to denote live storage | not stated | 2
LIFETIMES59 | "[Live-fact invalidation](state_contracts.md#mutation-and-subject-identity) and loan authority remain separate checks." | Carried-loan transport | Live-fact invalidation and loan authority must be checked independently | not stated | SHARED

## Ambiguous or two-way readings

"Packages keep ordinary lifetime parameters or permitted elision;" | Elision is allowed only where this chapter's notion of permitted elision applies to packages. | Any elision mechanism permitted anywhere in the language applies to packages. |
"their fields and concrete returned borrows remain tied to the same input/storage origins." | Origins are fixed at package definition boundaries and never change when consumers instantiate the package. | Origins are tied to whichever input or storage instance supplied each borrow at each use site. |
"Static callable requirements may additionally bind fresh lifetimes for each invocation" | Fresh lifetimes are optional additions when family matching succeeds. | Every static callable requirement always binds fresh lifetimes on each invocation when the linked rule applies. |
"The rule permits repeated reborrowing and requires universal requirement refinement;" | Repeated reborrowing is always allowed once universal refinement is discharged. | Repeated reborrowing is permitted only in the specific situations the linked generic-family rule names. |
"For an elided output, one unambiguous reference input supplies the relation." | Unambiguity is a property of the input type and carried-reference structure alone. | Unambiguity may also require knowing which inputs are actually passed at a call site. |
"Within a selected input, every reference leaf carrying the selected lifetime remains a possible source unless exact correspondence narrows it." | The compiler must enumerate all such leaves until correspondence rules prune the set. | Leaves that cannot legally back the returned access are excluded before counting as possible sources. |
"A template-dependent frontier is not an empty one;" | Template-dependent frontiers carry obligations even when instantiation is deferred. | A frontier that mentions only template parameters still counts as non-empty for admission purposes but carries no concrete loans until instantiated. |
"If an owned move already emptied the field under a borrowed-storage invariant window" | The invariant window is determined solely from ownership rules in the linked section. | Other borrow-state facts may also qualify as an invariant window for this assignment rule. |
"Transitions may transport references only while their storage outlives the target path." | Outliving is checked against the target path at the transition edge only. | Outliving must hold for the entire time the reference is retained in the target machine state. |
"Persistent fields retain exact field/case/index provenance across control-flow edges only under valid source correspondence;" | Provenance transport across edges is automatic when correspondence holds. | Provenance is retained only when explicit transition evidence records valid source correspondence. |
"Reference copies still denote live storage, unlike integer snapshots." | Copying a reference never breaks liveness obligations on the denoted storage. | Reference copies are semantically distinct from integer snapshots for fact tracking, without specifying all copy kinds covered. |

## Terms used without definition in this chapter

- lifetime / borrowed result / stored view
- loan / loan resources / loan paths / loan obligations / unresolved loans
- captured place / lineage / formation and restoration evidence
- referent
- `some` binder / reference lifetime
- lifetime parameters / permitted elision / opening / repacking
- input/storage origins
- tick spelling / access modifier / `mut` / `write` (reference modifiers)
- borrow-carrying data / views / carried references / carried-loan transport
- named data applications / lexical owner / undeclared tags / erased regions
- runtime generic arity / monomorphization
- conformance telescopes / normalized public mapping / application-elision rules
- static callable requirements / fresh lifetimes / generic family matching
- captured-environment lifetimes / universal requirement refinement
- authored outlives bounds / higher-ranked whole-conformance arguments
- explicit result lifetime / reference leaves / input lifetime
- unrelated storage / lifetime spelling
- admitted foreign boundary / provider-owned reference / safe contract / output slot
- lifetime-related input object / provider admission
- ordinary checked machines / backing
- elided output / unambiguous reference input / possible sources
- qualified machine name / first-argument position
- structural lifetime frontier / active sum payloads / qualifications
- output path / source paths / exact correspondence / returned access
- owned borrow-carrying input / caller's private carrier storage
- generic returned views / instantiated frontier / result-to-input relation
- template-dependent frontier / call admission
- single-source elision rule / general outlives syntax
- enclosing field / unrelated siblings / operand compatibility
- exclusive or write-only leaf / overlap
- fixed index / dynamic selection
- borrowed-storage invariant window / restoration obligation / enclosing exclusive loan
- domain-only casts / non-owning qualifications
- representation recasts / validated footprint / overlap judgment
- transitions / target path / persistent fields
- field/case/index provenance / valid source correspondence
- coincident name / runtime index
- reference copies / integer snapshots
- live-fact invalidation / loan authority

CHAPTER COMPLETE
