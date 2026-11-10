# Package declaration and carried-type boundaries

Source: `wiki/spec/packages/boundaries.md` lines 1–124 (read).

## Rules

rule id | the exact sentence, quoted | the heading it is under | what must be checked, in plain words | when it must hold | lane
--- | --- | --- | --- | --- | ---
BOUNDARIES1 | "`public` exposes declarations owned by the package;" | Package declaration and carried-type boundaries | Only package-owned declarations may be exposed under `public` | not stated | 1
BOUNDARIES2 | "`build.omg` declares its dependencies and selections." | Package declaration and carried-type boundaries | Dependencies and selections must be declared in `build.omg` before use for authority | not stated | 9
BOUNDARIES3 | "Undeclared aliases are not nameable," | Package declaration and carried-type boundaries | Reject source references that name a dependency alias not declared for the current scope | not stated | 1
BOUNDARIES4 | "and fully qualified spelling does not bypass that boundary." | Package declaration and carried-type boundaries | Reject name lookup via fully qualified spelling when the dependency-reach boundary is not satisfied | not stated | 1
BOUNDARIES5 | "There is no export item that relabels a dependency declaration." | Package declaration and carried-type boundaries | Reject export forms that would present a dependency declaration as if owned or renamed by the exporting package | not stated | 1
BOUNDARIES6 | "A subsystem needing a different dependency-reach set within a checked context is a separate package, not a hidden nested manifest." | Package declaration and carried-type boundaries | Reject checked contexts that embed a distinct dependency-reach set without a separate package manifest | not stated | 9
BOUNDARIES7 | "The root's explicit build and product scopes are distinct contexts, not nested manifests." | Package declaration and carried-type boundaries | Treat build scope and product scope as distinct checked contexts, not nested manifests inside one package | not stated | 9
BOUNDARIES8 | "Both scopes remain package-owned;" | Package declaration and carried-type boundaries | Build and product scopes must remain attributed to the owning package | not stated | 9
BOUNDARIES9 | "neither loaded source nor a borrowed Build grants the other's nameability." | Package declaration and carried-type boundaries | Do not grant nameability in one scope solely because source is loaded or Build is borrowed from the other scope | not stated | 9
BOUNDARIES10 | "A direct dependency in the requesting occurrence's scope authorizes authored selection of its declarations." | Selection versus carrying a type | Permit declaration selection only when a direct dependency in the requesting occurrence's scope authorizes it | at a call | 7
BOUNDARIES11 | "It is not required merely because that package's nominal type arrives through an already-declared dependency's API." | Selection versus carrying a type | Do not require a direct dependency edge solely because a nominal type from that package is carried through another dependency's API | not stated | 7
BOUNDARIES12 | "Such a value may be moved, borrowed, stored, returned, passed back through that API, and checked for multiplicity." | Selection versus carrying a type | Allow move, borrow, store, return, pass-back, and multiplicity checking for values carried through an authorized API without extra owner selection | at the operation | 2
BOUNDARIES13 | "Carrying it does not grant access to its owner's methods, fields, cases, operators, conformances, or ordinary consuming machines." | Selection versus carrying a type | Reject authored member selection on a carried type when dependency authority for the type owner is absent | at the operation | 7
BOUNDARIES14 | "Compiler-planned layout and automatic cleanup are carried type semantics." | Selection versus carrying a type | Apply layout and automatic cleanup according to carried-type semantics, not as newly granted owner API | not stated | 2
BOUNDARIES15 | "The hook a type selects with `dropped by` is compiler-only;" | Selection versus carrying a type | Treat `dropped by` hook selection as compiler-only, not consumer-authored API on the carrying package | not stated | 2
BOUNDARIES16 | "an authored `omega::core::drop(value)` is an ordinary consuming call whose resulting cleanup plan remains carried semantics." | Selection versus carrying a type | Classify authored `omega::core::drop` as an ordinary consuming call; cleanup plan stays carried semantics | at a call | 2
BOUNDARIES17 | "Borrowed erased views never acquire cleanup ownership of their referents." | Selection versus carrying a type | Do not transfer cleanup ownership of referents to borrowed erased views | at the operation | 2
BOUNDARIES18 | "Reflection elaborates member selection under the lexical author's dependency and visibility authority, not the generic caller's." | Selection versus carrying a type | Resolve reflection member selection using the lexical author's dependency and visibility, not the generic caller's | at a call | 8
BOUNDARIES19 | "An owner may supply a checked visitor or conformance to another package." | Selection versus carrying a type | Allow cross-package visitor or conformance only when supplied and checked by the owner | not stated | 7
BOUNDARIES20 | "That package invokes the supplied operation;" | Selection versus carrying a type | The receiving package may invoke only the supplied operation, not arbitrary owner members | at a call | 3
BOUNDARIES21 | "carrying its type or metadata does not authorize independently enumerating the owner's fields." | Selection versus carrying a type | Reject independent field enumeration of the owner when only type or metadata is carried | at the operation | 1
BOUNDARIES22 | "Exact foreign-domain qualification and evidence can likewise travel through an API without importing the domain for authored lookup." | Selection versus carrying a type | Allow foreign-domain qualification and evidence through APIs without treating the domain as imported for authored lookup | not stated | 8
BOUNDARIES23 | "Carrying does not expose its declaring package's other extensions or erase its separate ownership." | Selection versus carrying a type | Do not expose other extensions of the declaring package or collapse separate ownership merely because a value is carried | not stated | 7
BOUNDARIES24 | "Explicit domain selection follows file-local import exposure." | Selection versus carrying a type | Require explicit domain selection to follow file-local import exposure rules | not stated | 8
BOUNDARIES25 | "The transitive closure retains the exact type owner." | Selection versus carrying a type | Dependency and identity computations must retain the exact type owner through transitive closure | not stated | 7
BOUNDARIES26 | "Exact semantic dependencies affect rebuild/content identity for private uses and also API compatibility for public-signature uses." | Selection versus carrying a type | Record exact semantic dependencies for private rebuild/content identity and public-signature API compatibility | not stated | 9
BOUNDARIES27 | "A coarse whole-package dependency may conservatively over-reject until exact declaration edges exist;" | Selection versus carrying a type | May reject conservatively on coarse whole-package edges when exact declaration edges are missing | not stated | 9
BOUNDARIES28 | "it cannot expand source nameability." | Selection versus carrying a type | Coarse dependency edges must not widen source nameability beyond declared authority | not stated | 1
BOUNDARIES29 | "Owned erased payloads carry their exact movement/cleanup descriptor," | Selection versus carrying a type | Erased payloads must carry the exact movement and cleanup descriptor of the owned type | not stated | 2
BOUNDARIES30 | "not a new declaration selection or trait-evidence grant." | Selection versus carrying a type | Carried erased payloads must not create new declaration selection or trait-evidence grants | not stated | 8
BOUNDARIES31 | "Public contracts, data/domain facts, and trait contracts use public-interface exposure." | Exposure and declaration selection | Public contracts, data/domain facts, and trait contracts must use public-interface exposure rules | not stated | 1
BOUNDARIES32 | "Executable bodies, nested machines, local annotations/casts, owned storage, and ranking witnesses remain private implementation." | Exposure and declaration selection | Listed implementation artifacts must remain private, not public-interface exposed | not stated | 10
BOUNDARIES33 | "The public termination promise is `terminates`, not the measure used to prove it." | Exposure and declaration selection | Expose only `terminates` publicly; do not expose the ranking or measure witness as public interface | not stated | 4
BOUNDARIES34 | "A public interface cannot expose a private declaration for consumer source selection." | Exposure and declaration selection | Reject public interfaces that would let consumers select a private declaration in source | not stated | 1
BOUNDARIES35 | "Private issuer references in a domain's authorization catalog are the limited evidence-only exception described below." | Exposure and declaration selection | Allow private issuer references in domain authorization catalogs only under the stated evidence-only exception | not stated | 8
BOUNDARIES36 | "Independently nameable declarations own their own visibility." | Exposure and declaration selection | Visibility attaches to independently nameable declarations themselves | not stated | 1
BOUNDARIES37 | "Only genuine members of one exact module, trait or named implementation inherit that owner's visibility;" | Exposure and declaration selection | Member visibility inheritance applies only to genuine members of one exact module, trait, or named implementation | not stated | 1
BOUNDARIES38 | "data does not own machine declarations." | Exposure and declaration selection | Do not treat data types as owning machine declarations for visibility inheritance | not stated | 10
BOUNDARIES39 | "Qualified declaration heads retain exact namespace/dependency selection without a data-attachment route;" | Exposure and declaration selection | Qualified declaration heads must keep exact namespace and dependency selection; no selection via data attachment | not stated | 7
BOUNDARIES40 | "qualification does not admit a transitive-only owner." | Exposure and declaration selection | Reject qualification paths that select a transitive-only owner without direct dependency authority | not stated | 7
BOUNDARIES41 | "Exported boundary supply has interface exposure even without ordinary `public`." | Exposure and declaration selection | Treat exported boundary supply as interface-exposed even when not marked ordinary `public` | not stated | 9
BOUNDARIES42 | "Trait parents and body requirements normalize to semantic composition edges but retain every authored selection." | Exposure and declaration selection | Normalize trait parents and body requirements to composition edges without dropping authored selections | not stated | 7
BOUNDARIES43 | "Generic bounds select their exact right-hand trait application;" | Exposure and declaration selection | Generic bounds must select the exact right-hand trait application | not stated | 7
BOUNDARIES44 | "a named conformance selects its implementation and explicit trait arguments, with no data subject." | Exposure and declaration selection | Named conformance selection must bind implementation and explicit trait arguments without a data subject | not stated | 7
BOUNDARIES45 | "Type parameters, evidence binders and local value roots remain lexical bindings." | Exposure and declaration selection | Type parameters, evidence binders, and local value roots must remain lexical bindings, not declaration-selection rows | not stated | 7
BOUNDARIES46 | "Every selected implementation needs dependency authority." | Exposure and declaration selection | Each selected implementation must have dependency authority in the requesting scope | not stated | 7
BOUNDARIES47 | "Ordinary type inference neither selects a conformance nor grants transitive-only access." | Exposure and declaration selection | Type inference must not select conformances or grant transitive-only access | not stated | 7
BOUNDARIES48 | "An exact `satisfies` edge selects its trait and overload-resolved requirement," | Exposure and declaration selection | Each exact `satisfies` edge must select trait and overload-resolved requirement | not stated | 7
BOUNDARIES49 | "including an exact operator or boundary-trait requirement." | Exposure and declaration selection | Operator and boundary-trait requirements must be selected exactly on `satisfies` edges | not stated | 7
BOUNDARIES50 | "It follows the realizing machine's interface exposure, including exported boundary/accepted supply." | Exposure and declaration selection | `satisfies` exposure follows the realizing machine's interface exposure, including exported boundary or accepted supply | not stated | 10
BOUNDARIES51 | "Selection settles before supply admission;" | Exposure and declaration selection | Fix declaration selection before admitting supply | not stated | 9
BOUNDARIES52 | "rejection of the supply does not erase the declaration it selected." | Exposure and declaration selection | Retain the selected declaration identity when supply admission is rejected | not stated | 9
BOUNDARIES53 | "Nested machine-parameter contracts and generic applications retain the same selections." | Exposure and declaration selection | Preserve the same authored selections through nested machine-parameter contracts and generic applications | not stated | 10
BOUNDARIES54 | "Each domain establishment, read, or write route selects an exact trait requirement or concrete machine," | Exposure and declaration selection | Domain establish/read/write routes must select an exact trait requirement or concrete machine | at the operation | 8
BOUNDARIES55 | "retaining its target kind and author-side dependency/visibility checks." | Exposure and declaration selection | Domain routes must retain target kind and apply author-side dependency and visibility checks | at the operation | 8
BOUNDARIES56 | "The three catalogs retain their separate roles." | Exposure and declaration selection | Keep establishment, read, and write catalogs distinct without merging roles | not stated | 8
BOUNDARIES57 | "Repeated authored routes retain their occurrences even when equal semantic alternatives normalize to one route." | Exposure and declaration selection | Preserve repeated authored route occurrences after normalization to equal semantic routes | not stated | 8
BOUNDARIES58 | "A public domain may authorize a private requirement or machine accessible to its author under the private-issuer rule." | Exposure and declaration selection | Allow public domain authorization of private requirements or machines only under the private-issuer rule for the author | not stated | 8
BOUNDARIES59 | "The catalog retains exact identities and dependencies for verification without making the selected route callable, externally implementable, or source-nameable." | Exposure and declaration selection | Catalog entries keep identities and dependencies for verification but must not become callable, externally implementable, or source-nameable solely via catalog retention | not stated | 8
BOUNDARIES60 | "Public predicates, carrier types, and ordinary signatures keep their existing exposure rules." | Exposure and declaration selection | Apply unchanged exposure rules to public predicates, carrier types, and ordinary signatures | not stated | 1
BOUNDARIES61 | "Lexical subjects/binders do not become declaration-selection rows." | Exposure and declaration selection | Do not promote lexical subjects or binders into declaration-selection rows | not stated | 7
BOUNDARIES62 | "Value calls, Unit/discarded calls, static arguments, constructor fields, member projections, and intrinsic unary expressions retain independent selections." | Exposure and declaration selection | Each listed syntactic site must retain its own independent declaration selections | at the operation | 3
BOUNDARIES63 | "Computed receivers and normalization cannot erase their authored selections;" | Exposure and declaration selection | Preserve authored selections through computed receivers and normalization | not stated | 3
BOUNDARIES64 | "the selected field must agree with the checked projection." | Exposure and declaration selection | Selected field on member projection must match the checked projection result | at the operation | 1
BOUNDARIES65 | "Constants retain declaration provenance after value substitution." | Exposure and declaration selection | After constant value substitution, retain original declaration provenance | not stated | 1
BOUNDARIES66 | "An intrinsic receives its closed checked identity, not a fictional package owner." | Exposure and declaration selection | Intrinsics must use closed checked identity, not an invented package owner | not stated | 9
BOUNDARIES67 | "Quotient formation retains carrier, relation, trait/application, and proof-conformance selections." | Exposure and declaration selection | Quotient formation must retain carrier, relation, trait or application, and proof-conformance selections | not stated | 8
BOUNDARIES68 | "The relation and trait inherit the data interface's exposure." | Exposure and declaration selection | Relation and trait exposure in quotient formation follows the data interface exposure | not stated | 8
BOUNDARIES69 | "The selected equivalence proof is private formation custody, absent from mathematical API identity but still subject to visibility and dependencies." | Exposure and declaration selection | Treat equivalence proof as private formation custody excluded from mathematical API identity while enforcing visibility and dependencies | not stated | 8
BOUNDARIES70 | "Capture exact authored selections before source relationships are erased;" | Admit before execution | Record exact authored selections before erasing source relationships | not stated | SHARED
BOUNDARIES71 | "finalize late receiver/overload/conformance choices after checking." | Admit before execution | Finalize receiver, overload, and conformance choices only after checking completes | not stated | 3
BOUNDARIES72 | "Missing, ambiguous, or unjoinable package-source choices reject." | Admit before execution | Reject compilation when package-source choices are missing, ambiguous, or unjoinable | not stated | 9
BOUNDARIES73 | "Final execution consumes the finalized selections." | Admit before execution | Execution must use only finalized selections | not stated | SHARED
BOUNDARIES74 | "Earlier effect-free evaluation requires exact admitted targets, or proof that the complete candidate set stays within admitted owners." | Admit before execution | Effect-free evaluation before execution needs exact admitted targets or proof all candidates stay within admitted owners | not stated | SHARED
BOUNDARIES75 | "An implementation unable to establish that gate must reorder or split its work, not execute first and check authority afterward." | Admit before execution | Compiler pipeline must not run execution before authority or admission gates are established | not stated | SHARED
BOUNDARIES76 | "Admission walks both caller-to-callee edges and selections in reachable bodies." | Admit before execution | Admission must traverse caller-to-callee edges and selections in all reachable bodies | on every edge | 3
BOUNDARIES77 | "Shared evaluation policy is reusable only after every authored application site is admitted." | Admit before execution | Do not reuse shared evaluation policy until every authored application site is admitted | not stated | SHARED
BOUNDARIES78 | "Literal/builtin spelling does not substitute for a checked intrinsic identity." | Admit before execution | Reject using literal or builtin spelling in place of checked intrinsic identity | not stated | 9
BOUNDARIES79 | "Carried semantic dependencies and authored selections are separate compiler-private facts;" | Admit before execution | Maintain carried semantic dependencies and authored selections as distinct compiler-private facts | not stated | SHARED
BOUNDARIES80 | "neither requires another public IR stage." | Admit before execution | Neither carried semantic dependencies nor authored selections may require an extra public IR stage | not stated | SHARED
BOUNDARIES81 | "Packages normally compose statically and can optimize across package boundaries." | Admit before execution | Treat packages as static composition units eligible for cross-package optimization | not stated | 9
BOUNDARIES82 | "They are not ABI or runtime replacement units by default." | Admit before execution | Do not treat packages as ABI or runtime replacement units unless component publication rules apply | not stated | 9
BOUNDARIES83 | "Independent realization and frozen replacement envelopes follow component publication." | Admit before execution | Independent realization and frozen replacement envelopes must follow component publication rules | not stated | 9

## Ambiguous sentences

Sentence | Reading A | Reading B
--- | --- | ---
"The package is the dependency-reach boundary." | Every nameability and declaration-selection check is scoped to declared package dependency reach. | "The package" names only manifest ownership; other documents may define additional boundaries not stated here. |
"A direct dependency in the requesting occurrence's scope authorizes authored selection of its declarations." | Only dependencies declared in the same scope as the requesting source occurrence authorize selection. | "Requesting occurrence" may mean any occurrence in the checked context or activation, and scope attribution is not defined in this sentence. |
"Such a value may be moved, borrowed, stored, returned, passed back through that API, and checked for multiplicity." | These operations are permitted whenever the value is carried through the API of an already-declared dependency without a direct owner dependency. | "May" lists allowed operations that still require separate type, loan, and multiplicity checks elsewhere; this sentence does not guarantee acceptance. |
"Compiler-planned layout and automatic cleanup are carried type semantics." | Layout and cleanup follow carried-type rules exclusively, not owner-package selection rules. | Carried semantics apply to layout and cleanup planning while other owner semantics might still apply to distinct checks not named here. |
"Reflection elaborates member selection under the lexical author's dependency and visibility authority, not the generic caller's." | Reflection never uses the generic caller's dependency set even when the caller is monomorphized or specialized. | "Generic caller" excludes only callers with unresolved type parameters; fully specialized callers might still count as lexical authors elsewhere. |
"Exact semantic dependencies affect rebuild/content identity for private uses and also API compatibility for public-signature uses." | Both identity and compatibility must consult exact semantic dependency edges for the respective use classes. | The sentence states what exact dependencies affect without saying the compiler must reject when coarse edges disagree with exact edges. |
"A coarse whole-package dependency may conservatively over-reject until exact declaration edges exist;" | Conservative rejection on coarse edges is permitted; programs may fail though exact edges would suffice. | Over-rejection is temporary only until edges are materialized; once edges exist rejection must match exact graph only. |
"Exported boundary supply has interface exposure even without ordinary `public`." | Boundary supply is treated as interface-exposed by kind even when `public` is absent. | Interface exposure applies only to forms classified as exported boundary supply elsewhere; this sentence does not define that classification. |
"Selection settles before supply admission; rejection of the supply does not erase the declaration it selected." | Selected declaration identity is fixed before supply checks and survives supply rejection. | Selection is logically prior but diagnostics or later phases might still hide the selection from the author after rejection. |
"Earlier effect-free evaluation requires exact admitted targets, or proof that the complete candidate set stays within admitted owners." | Effect-free evaluation is forbidden unless targets are admitted or a whole-set proof is available. | Effect-free evaluation may proceed under partial admission when only some candidates are used, and "complete candidate set" is undefined here. |
"An implementation unable to establish that gate must reorder or split its work, not execute first and check authority afterward." | The compiler must reject or reorder passes that would execute before admission. | This constrains compiler implementation quality; it does not state a program-level rejection when a non-conforming compiler runs anyway. |
"Shared evaluation policy is reusable only after every authored application site is admitted." | No shared policy across sites until each site has passed admission. | Shared policy may be computed early but must not affect results until admission completes; reuse of intermediate artifacts is not ruled out. |

## Terms used without definition in this chapter

- package
- dependency-reach boundary
- `public`
- `build.omg`
- aliases
- fully qualified spelling
- export item
- dependency declaration
- checked context
- hidden nested manifest
- build and product scopes
- nested manifests
- borrowed Build
- nameability
- direct dependency
- requesting occurrence's scope
- authored selection
- nominal type
- already-declared dependency's API
- multiplicity
- carried type / carrying
- methods, fields, cases, operators, conformances
- ordinary consuming machines
- compiler-planned layout
- automatic cleanup
- carried type semantics
- `dropped by`
- `omega::core::drop(value)`
- cleanup plan
- borrowed erased views
- cleanup ownership
- referents
- lexical author
- generic caller
- checked visitor
- conformance
- metadata
- foreign-domain qualification
- evidence
- authored lookup
- extensions
- separate ownership
- file-local import exposure
- transitive closure
- exact type owner
- exact semantic dependencies
- rebuild/content identity
- API compatibility
- public-signature uses
- coarse whole-package dependency
- exact declaration edges
- source nameability
- owned erased payloads
- movement/cleanup descriptor
- trait-evidence grant
- public-interface exposure
- public contracts
- data/domain facts
- trait contracts
- executable bodies
- nested machines
- local annotations/casts
- owned storage
- ranking witnesses
- private implementation
- public termination promise
- `terminates`
- measure
- consumer source selection
- authorization catalog
- private issuer references
- evidence-only exception
- independently nameable declarations
- genuine members
- exact module
- named implementation
- machine declarations
- qualified declaration heads
- namespace/dependency selection
- data-attachment route
- transitive-only owner
- exported boundary supply
- interface exposure
- trait parents
- body requirements
- semantic composition edges
- generic bounds
- right-hand trait application
- named conformance
- trait arguments
- data subject
- type parameters
- evidence binders
- local value roots
- lexical bindings
- dependency authority
- ordinary type inference
- exact `satisfies` edge
- overload-resolved requirement
- boundary-trait requirement
- realizing machine
- exported boundary/accepted supply
- supply admission
- nested machine-parameter contracts
- generic applications
- domain establishment, read, or write route
- target kind
- three catalogs
- authored routes
- semantic alternatives
- public domain
- private-issuer rule
- callable
- externally implementable
- public predicates
- carrier types
- declaration-selection rows
- Unit/discarded calls
- static arguments
- constructor fields
- member projections
- intrinsic unary expressions
- computed receivers
- normalization
- checked projection
- declaration provenance
- value substitution
- closed checked identity
- fictional package owner
- quotient formation
- proof-conformance selections
- data interface
- equivalence proof
- private formation custody
- mathematical API identity
- source relationships
- package-source choices
- unjoinable
- finalized selections
- effect-free evaluation
- admitted targets
- admitted owners
- shared evaluation policy
- application site
- literal/builtin spelling
- checked intrinsic identity
- carried semantic dependencies
- compiler-private facts
- public IR stage
- statically compose
- optimize across package boundaries
- ABI or runtime replacement units
- independent realization
- frozen replacement envelopes
- component publication

CHAPTER COMPLETE
