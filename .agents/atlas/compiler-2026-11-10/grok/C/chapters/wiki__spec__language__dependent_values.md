# Value-dependent facts and views

Source: `wiki/spec/language/dependent_values.md` lines 1–332 (read).

## Rules

rule id | the exact sentence, quoted | the heading it is under | what must be checked, in plain words | when it must hold | lane
--- | --- | --- | --- | --- | ---
DEPENDENT_VALUES1 | "Runtime dependencies do not become const-generic arguments." | Static indices and runtime witnesses | Runtime program values used as dependencies must not be passed or treated as const-generic arguments | not stated | 7
DEPENDENT_VALUES2 | "access must prove the actual range lies inside its backing extent, not substitute the compiled record size for the foreign stride" | Static indices and runtime witnesses | For a runtime descriptor view using count and stride, access must prove the accessed range is within backing extent without substituting compiled record size for foreign stride | at the operation | 6
DEPENDENT_VALUES3 | "Row-major access similarly needs the relation between dimensions, coordinates, and total extent." | Static indices and runtime witnesses | Row-major access must prove the relation among dimensions, coordinates, and total extent | at the operation | 6
DEPENDENT_VALUES4 | "runtime witnesses do not imply variable-sized stack locals, implicit boxing, or a hidden global layout descriptor." | Static indices and runtime witnesses | Runtime witnesses must not be taken to authorize variable-sized stack locals, implicit boxing, or a hidden global layout descriptor | not stated | 1
DEPENDENT_VALUES5 | "The sanctioned route to a runtime-sized region inside an activation is an explicit bounded extent claim under [runtime-sized activation storage](../resources/activation_storage.md)." | Static indices and runtime witnesses | A runtime-sized region inside an activation is permitted only with an explicit bounded extent claim under runtime-sized activation storage | not stated | 2
DEPENDENT_VALUES6 | "Runtime values have no implicit proof tuple or hidden witness allocation merely because a fact names them." | Static indices and runtime witnesses | Facts naming runtime values must not introduce implicit proof tuples or hidden witness allocations | not stated | 8
DEPENDENT_VALUES7 | "It does not by itself allocate count elements, instantiate count-specific code, or authorize runtime-sized inline stack storage — that stays a bounded claim under [runtime-sized activation storage](../resources/activation_storage.md), including when the buffer's realization is activation-local." | Static indices and runtime witnesses | `Buffer<count>` alone must not allocate count elements, instantiate count-specific code, or authorize runtime-sized inline stack storage without a bounded activation-storage claim | not stated | 2
DEPENDENT_VALUES8 | "A `const` binder still requires static knowledge." | Static indices and runtime witnesses | A const value binder must have static knowledge of its subject | not stated | 7
DEPENDENT_VALUES9 | "runtime-capable indices do not reinterpret them or turn arbitrary runtime data into nominal types." | Static indices and runtime witnesses | Runtime-capable indices must not reinterpret const-indexed domain families or turn arbitrary runtime data into nominal types | not stated | 7
DEPENDENT_VALUES10 | "Prove a dependency statically, establish it through a visible guard or contracted decode/validator, or reject the use." | Establishment and witness loans | Each value dependency must be proved statically, established via a visible guard or contracted decode or validator, or the use must be rejected | not stated | SHARED
DEPENDENT_VALUES11 | "`as` follows [domain casts](domains.md#domain-casts)." | Establishment and witness loans | `as` casts on dependent values must follow domain-cast rules | at the operation | 5
DEPENDENT_VALUES12 | "The compiler does not insert residual checks to make an unproved refinement true." | Establishment and witness loans | The compiler must not insert residual checks to make an unproved refinement true | not stated | SHARED
DEPENDENT_VALUES13 | "An explicit runtime check owns its ordinary false outcome;" | Establishment and witness loans | An explicit runtime check must use the language's ordinary false outcome semantics | at the operation | 0
DEPENDENT_VALUES14 | "a decoder establishes only the predicates it checks, not firmware truth or routed authority." | Establishment and witness loans | A decoder may establish only the predicates it actually checks, not firmware truth or routed authority beyond those checks | not stated | 3
DEPENDENT_VALUES15 | "A live borrow of a dependent place also read-loans the witnesses on which its validity relies." | Establishment and witness loans | Creating a live borrow of a dependent place must register read-loans on all witnesses its validity relies on | at the operation | 2
DEPENDENT_VALUES16 | "Those witnesses cannot change while the dependent borrow is live." | Establishment and witness loans | Witness values pinned by a dependent borrow must not change while that borrow is live | across a pause | 2
DEPENDENT_VALUES17 | "Facts and loans are checked together before erasure; forgetting a layout parameter or a displayed qualification cannot remove this obligation." | Establishment and witness loans | Witness-loan obligations must be checked together with facts before erasure; layout parameters or displayed qualifications cannot erase this obligation | not stated | 2
DEPENDENT_VALUES18 | "Across calls, transport facts only through their exact parameter/result substitution and checked contract." | Establishment and witness loans | Cross-call fact transport must use only exact parameter and result substitution and checked contracts | at a call | 3
DEPENDENT_VALUES19 | "Machine-arrival requirements are proved on every incoming edge and assumed only at arrival." | Establishment and witness loans | Machine entry requirements must be proved on every incoming control-flow edge and assumed only at machine arrival | on every edge | 3
DEPENDENT_VALUES20 | "Mutation invalidates stale entry facts;" | Establishment and witness loans | Mutation must invalidate stale machine-entry facts | at the operation | 2
DEPENDENT_VALUES21 | "a backedge cannot prove itself from a premise invalidated by its own writes." | Establishment and witness loans | A loop backedge must not use entry facts that its own writes have invalidated | on every edge | 4
DEPENDENT_VALUES22 | "A data declaration's field qualifications and `requires` clauses define its default domain; omission adds no predicates beyond its ordinary field/type obligations." | Default domains and zero initialization | Default domain predicates come only from field qualifications and data `requires`; omitted clauses must not add predicates beyond ordinary field and type obligations | not stated | 5
DEPENDENT_VALUES23 | "Zero-initializability is a storage representation guarantee, not universal semantic membership or ambient write authority." | Default domains and zero initialization | Zero-initializability must be treated as a storage representation guarantee, not as universal semantic membership or ambient write authority | not stated | 1
DEPENDENT_VALUES24 | "A zeroed storage representation can be accessed as an established value only after its default-domain obligations hold." | Default domains and zero initialization | Accessing a zeroed storage representation as an established value requires default-domain obligations to hold first | at the operation | 5
DEPENDENT_VALUES25 | "Establishing validity does not grant access forbidden by that policy." | Default domains and zero initialization | Establishing default-domain validity must not grant reads or writes forbidden by domain-owned access policy | at the operation | 5
DEPENDENT_VALUES26 | "It remains attached to the governed field through parent borrows, projections, whole-place replacement, and domain removal." | Default domains and zero initialization | Domain-owned read or write restrictions must remain on the governed field through parent borrows, projections, whole-place replacement, and domain removal | not stated | 2
DEPENDENT_VALUES27 | "Invariant windows permit temporary invalidity only under otherwise-authorized access; they do not waive a reader or writer restriction." | Default domains and zero initialization | Invariant windows must not waive reader or writer restrictions; temporary invalidity is allowed only under otherwise-authorized access | not stated | 5
DEPENDENT_VALUES28 | "Fresh construction and policy installation follow the domain establishment rules, not an implicit file-local or constructor privilege." | Default domains and zero initialization | Fresh construction and policy installation must follow domain establishment rules without implicit file-local or constructor privilege | not stated | 5
DEPENDENT_VALUES29 | "Constraints on common fields and active-case payloads are written in data `requires`." | Default domains and zero initialization | Common-field and active-case payload constraints must appear only in data `requires`, not elsewhere | not stated | 1
DEPENDENT_VALUES30 | "Payload projections require a prior case guard; case declarations do not accept `requires`." | Default domains and zero initialization | Projecting a case payload requires a prior case guard; `requires` on case declarations must be rejected | at the operation | 1
DEPENDENT_VALUES31 | "Bracketed scalar range suffixes are not a second default-domain syntax." | Default domains and zero initialization | Bracketed scalar range suffixes must not be accepted as alternate default-domain syntax | not stated | 5
DEPENDENT_VALUES32 | "Boundary provider slots are not data fields and add no default-domain obligation." | Default domains and zero initialization | Boundary provider slots must not be treated as data fields or sources of default-domain obligation | not stated | 9
DEPENDENT_VALUES33 | "Provider planning must satisfy every demanded exact requirement application before realization; that independent check neither changes the containing data's validity nor manufactures authority or storage." | Default domains and zero initialization | Before provider realization, every demanded exact requirement application must be satisfied; that check must not change containing data validity or manufacture authority or storage | not stated | 9
DEPENDENT_VALUES34 | "Common-field and data-level obligations remain conjunctive with that case's payload validity." | Default domains and zero initialization | Common-field and data-level default-domain obligations must be conjunctive with the active case's payload validity | not stated | 1
DEPENDENT_VALUES35 | "The zero representation keeps the first declared case, even when its constraints fail for a particular specialization; there is no selection of a first possible case." | Default domains and zero initialization | Zero representation must use the first declared case only; the compiler must not select another first-possible case when constraints fail for a specialization | not stated | 1
DEPENDENT_VALUES36 | "If zero satisfies the default domain, the value is zero-constructible. Otherwise the type is gated: construction or checked qualification must establish the domain before observation." | Default domains and zero initialization | Types whose default domain is not satisfied by zero must gate observation until construction or checked qualification establishes the domain; zero-constructible types must allow zero when zero satisfies the domain | at the operation | 5
DEPENDENT_VALUES37 | "A literal without `..` supplies every runtime field." | Default domains and zero initialization | A literal without `..` must supply every runtime field | not stated | 1
DEPENDENT_VALUES38 | "With [explicit omission](data_and_literals.md#explicit-field-omission), it supplies every field whose zero value would violate its obligations and proves the complete coupling." | Default domains and zero initialization | Literal explicit omission must supply every field whose zero would violate obligations and prove complete coupling | not stated | 1
DEPENDENT_VALUES39 | "Zero-initializable storage does not implicitly authorize source-literal omission." | Default domains and zero initialization | Zero-initializable storage must not implicitly authorize source-literal field omission | not stated | 1
DEPENDENT_VALUES40 | "Establishment is monotone as observed: later mutation may temporarily open an invariant window, but no consumption sees the value outside its required domain." | Default domains and zero initialization | No consumption point may use a value outside its required domain; later mutation may open invariant windows only under the invariant-window rules | at the operation | 5
DEPENDENT_VALUES41 | "Generic bodies prove establishment from their declared contracts and available flow facts." | Default domains and zero initialization | Generic bodies must prove default-domain establishment from declared contracts and available flow facts only | not stated | 7
DEPENDENT_VALUES42 | "They may use proved zero-validity, but cannot rely on a later specialization to justify an otherwise unchecked observation." | Default domains and zero initialization | Generic code must not justify unchecked observation by appeal to a later specialization | at the operation | 7
DEPENDENT_VALUES43 | "Zeroing storage is permitted independently of establishing a value." | Default domains and zero initialization | Storage zeroing is permitted without establishing a value as a separate check from domain establishment | not stated | 1
DEPENDENT_VALUES44 | "Permitting [uninhabited domain declarations](domains.md#uninhabited-domains) does not relax this establishment gate." | Default domains and zero initialization | Uninhabited domain declarations must not relax the establishment gate | not stated | 5
DEPENDENT_VALUES45 | "An impossible qualification cannot be established merely because storage was allocated or zeroed." | Default domains and zero initialization | Storage allocation or zeroing must not establish an impossible domain qualification | not stated | 5
DEPENDENT_VALUES46 | "Gating propagates through containment;" | Default domains and zero initialization | Domain gating must propagate through value containment | not stated | 5
DEPENDENT_VALUES47 | "Machine-owned storage may begin zeroed while gated fields remain inaccessible until established." | Default domains and zero initialization | Gated fields on zeroed machine-owned storage must remain inaccessible until established | at the operation | 10
DEPENDENT_VALUES48 | "Storage validity is not a claim that partially established runtime-indexed arrays are automatically proved; such uses need exact element/extent establishment evidence." | Default domains and zero initialization | Partially established runtime-indexed arrays must not be treated as automatically proved; element and extent establishment evidence is required | at the operation | 6
DEPENDENT_VALUES49 | "Proof propositions and proof-only values with no runtime representation acquire neither ZII nor layout obligations." | Default domains and zero initialization | Proof propositions and proof-only values without runtime representation must not receive zero-initializability or layout obligations | not stated | 8
DEPENDENT_VALUES50 | "An ordinary `value: &mut T` argument carries the declared field/default-domain obligations of `T` at call, entry, and return." | Mutable arguments and declared field domains | `&mut T` parameters must carry `T`'s field and default-domain obligations at call, callee entry, and return | at a call | 2
DEPENDENT_VALUES51 | "The caller establishes them, the callee assumes them, and returning execution restores them under the ordinary invariant-window rules." | Mutable arguments and declared field domains | Caller must establish `T` obligations at call; callee assumes them at entry; return must restore them under invariant-window rules | at a call | 3
DEPENDENT_VALUES52 | "An additional input domain need not survive a call unless the ordinary contract guarantees it." | Mutable arguments and declared field domains | Additional input domain facts may expire after a call unless the contract guarantees they survive | at normal return | 3
DEPENDENT_VALUES53 | "Zeroed or partially established storage still needs establishment before a readable borrow;" | Mutable arguments and declared field domains | Zeroed or partially established storage must be established before a readable borrow is created | at the operation | 2
DEPENDENT_VALUES54 | "a qualified machine name grants no construction or validity exception." | Mutable arguments and declared field domains | A qualified machine name must not excuse missing construction or validity establishment | not stated | 10
DEPENDENT_VALUES55 | "That predicate-fact rule does not authorize dropping a governed subject's access policy." | Mutable arguments and declared field domains | Removing or invalidating flow predicates must not drop a governed subject's access policy | not stated | 5
DEPENDENT_VALUES56 | "Removing the displayed domain or invalidating a flow predicate cannot create a reader/writer grant; retained restrictions follow their governed lifetime." | Mutable arguments and declared field domains | Domain removal or predicate invalidation must not create new reader or writer grants; retained restrictions follow governed lifetime | not stated | 5
DEPENDENT_VALUES57 | "Lending it does not turn that obligation into an expiring input fact." | Lent storage retains its declared domains | Lending a local declared `value: T in D` must not turn restoration obligation for `D` into an expiring input fact | at a call | 2
DEPENDENT_VALUES58 | "A mutable loan, including a view whose displayed type omits `D` after a written [cast on the reference](domains.md#mutable-views), retains the original storage's obligations." | Lent storage retains its declared domains | Mutable loans and domain-stripped mutable views must retain the original storage's domain obligations | across a pause | 2
DEPENDENT_VALUES59 | "The owner's required predicates must hold before its qualified storage is consumed as valid or handed back to it." | Lent storage retains its declared domains | Owner's required domain predicates must hold before qualified storage is consumed as valid or handed back to the owner | at the operation | 5
DEPENDENT_VALUES60 | "A borrower's internal calls and terminal transfers check the receiving contracts and retain the owner's restoration obligation as described below; they do not each constitute handback to the owner." | Lent storage retains its declared domains | Borrower internal calls and terminal transfers must check receiving contracts and retain owner restoration obligation without treating each call as owner handback | at a call | 3
DEPENDENT_VALUES61 | "A call cannot silently change the local's declared qualification or make invalid storage usable under its old type." | Lent storage retains its declared domains | Calls must not silently change a lent local's declared qualification or make invalid storage usable under its old type | at a call | 2
DEPENDENT_VALUES62 | "If the original storage does not itself owe that qualification, the input fact may expire unless the contract promises it afterward." | Lent storage retains its declared domains | Additional input qualifications not owed by underlying storage may expire after call unless contract promises retention | at normal return | 3
DEPENDENT_VALUES63 | "Checking the call and loan handback must retain that distinction; the formal's spelling alone cannot decide which obligations the owner owes." | Lent storage retains its declared domains | Call and loan handback checking must distinguish owner restoration from expiring input facts without using formal spelling alone | at a call | 3
DEPENDENT_VALUES64 | "A cast cannot bypass them." | Lent storage retains its declared domains | Casts must not bypass predicate debt, establishment provenance, access policy, custody, or carry restrictions | at the operation | 5
DEPENDENT_VALUES65 | "Ratified 2026-11-05: existing `requires` and `ensures` compose repairs of a lender's named qualification. No new repair syntax is introduced." | Contract-based repair | Lender qualification repair must use only existing `requires` and `ensures` composition; new repair syntax must not be required or invented | not stated | 0
DEPENDENT_VALUES66 | "The receiving machine must establish its own parameter/default domains and entry requirements from the current contents." | Contract-based repair | Callee must establish its own parameter, default-domain, and entry requirements from current storage contents | at entry | 3
DEPENDENT_VALUES67 | "exclusive custody and the exact restoration obligation remain attached to that storage until handback." | Contract-based repair | Exclusive custody and exact restoration obligation for the lender's qualification must remain on storage until handback | across a pause | 2
DEPENDENT_VALUES68 | "That cast never cancels the obligation: each removed domain is re-established by its own `+D` rules for the current contents." | Contract-based repair | Cast removing displayed domains must not cancel restoration; each removed domain must be re-established by its own `+D` rules for current contents at handback | at the operation | 5
DEPENDENT_VALUES69 | "A helper's checked normal-completion guarantees may discharge the enclosing machine's restoration promise." | Contract-based repair | Helper normal-completion `ensures` may discharge the enclosing machine's restoration promise when checked | at normal return | 3
DEPENDENT_VALUES70 | "At a terminal call, prove the successor's requirements at entry and compose its guarantees into the enclosing completion contract." | Contract-based repair | Terminal calls must prove successor entry requirements and compose successor guarantees into enclosing completion contract | at a call | 3
DEPENDENT_VALUES71 | "A future guarantee is not a fact at the transfer's entry." | Contract-based repair | Successor guarantees must not be treated as facts at transfer entry before they are established | at a call | 3
DEPENDENT_VALUES72 | "Missing restoration evidence rejects the enclosing completion or handback;" | Contract-based repair | Enclosing completion or handback must be rejected when restoration evidence is missing | at normal return | 3
DEPENDENT_VALUES73 | "an extra helper layer or a recursive cycle cannot turn an unproved promise into evidence." | Contract-based repair | Helper layers and recursive cycles must not convert unproved restoration promises into evidence | not stated | 4
DEPENDENT_VALUES74 | "Recoverable outcomes must satisfy the obligations of their actual handback;" | Contract-based repair | Recoverable failure outcomes must satisfy obligations of the actual handback path | not stated | 3
DEPENDENT_VALUES75 | "This distinction does not weaken a data type's default domain." | Contract-based repair | Contract-based repair must not weaken a data type's default domain requirements | not stated | 5
DEPENDENT_VALUES76 | "A successor accepting `&mut Tree` still requires `Tree`'s default domain on arrival, even if it promises that domain on completion." | Contract-based repair | Successor entry must require the value type's default domain even when completion promises restore that domain | at entry | 5
DEPENDENT_VALUES77 | "An unestablished whole, absent field or invalid reference cannot be passed as a valid value." | Contract-based repair | Unestablished wholes, absent fields, or invalid references must be rejected as valid call arguments | at a call | 2
DEPENDENT_VALUES78 | "For multi-step repair, the intermediate view must itself be valid under its weaker declared type." | Contract-based repair | Each intermediate weaker view in multi-step repair must be valid under its declared type | not stated | 5
DEPENDENT_VALUES79 | "The named predicate and its defining conditions must not take different admission paths merely because mutation occurred." | Contract-based repair | Re-establishing predicate membership after mutation must use the same admission path for named predicate and defining conditions | not stated | 8
DEPENDENT_VALUES80 | "unfolding a predicate cannot manufacture them." | Contract-based repair | Predicate unfold must not manufacture establishment routes, provenance, access policy, or cleanup policy | not stated | 8
DEPENDENT_VALUES81 | "Unaffected facts remain available under the exact mutation frame; invalidated facts require new evidence." | Contract-based repair | Facts outside the mutation frame remain available; facts invalidated by mutation require new evidence | at the operation | 2
DEPENDENT_VALUES82 | "For `per-element-array-bounds`, use an ordinary pure, total, terminating machine over the collection as a predicate in `requires`." | Collection invariants | Per-element array bound invariants must be expressed via an ordinary pure, total, terminating machine predicate in `requires`, not via new quantifier syntax | not stated | 4
DEPENDENT_VALUES83 | "collection invariants introduce no `forall`, `foreach`, or bracketed scalar-range type suffix." | Collection invariants | Collection invariants must not introduce `forall`, `foreach`, or bracketed scalar-range type suffix | not stated | 4
DEPENDENT_VALUES84 | "Its use in a proposition must meet the ordinary [semantic evaluation](evaluation.md) admission rules." | Collection invariants | Using a collection predicate machine in a proposition must meet semantic evaluation admission rules | not stated | 8
DEPENDENT_VALUES85 | "Construction proves the collection predicate; mutation preserves or restores it under the ordinary invariant-window rules." | Collection invariants | Construction must prove collection predicate; mutation must preserve or restore it under invariant-window rules | at the operation | 1
DEPENDENT_VALUES86 | "Merely naming an arbitrary predicate does not grant the checker its elementwise consequences:" | Collection invariants | Naming a predicate must not automatically grant elementwise consequences without checked reasoning about the predicate machine | not stated | 8
DEPENDENT_VALUES87 | "Geometry, authority and custody are immediate requirements; target-shaped storage is not a valid value merely because a cast names its type." | Invariant windows | After exclusive-storage cast, geometry, authority, and custody are immediate; target-shaped storage is not valid solely because a cast names the target type | not stated | 5
DEPENDENT_VALUES88 | "Field initialization and checked evidence must establish the whole before ordinary typed consumption." | Invariant windows | The whole value must be established by field initialization and checked evidence before ordinary typed consumption | at the operation | 1
DEPENDENT_VALUES89 | "The cast's pending-view formation is allowed, not a general exception for later borrow creation, calls or publication." | Invariant windows | Pending-view formation from cast does not exempt later borrow creation, calls, or publication from establishment rules | at the operation | 2
DEPENDENT_VALUES90 | "Ownership restoration and domain restoration are separate obligations; satisfying one cannot discharge the other." | Invariant windows | Satisfying ownership restoration must not discharge domain restoration or vice versa | not stated | 2
DEPENDENT_VALUES91 | "A write known to preserve the default domain keeps it established. Otherwise the write opens a window on the place," | Invariant windows | Writes must either preserve default domain or open an invariant window on the written place | at the operation | 5
DEPENDENT_VALUES92 | "The domain must be proved again at the next consumption point:" | Invariant windows | Before each listed consumption kind, domain must be reproved when a window is open | at the operation | 5
DEPENDENT_VALUES93 | "A read relying on the domain or a dependent coupling." | Invariant windows | Reads relying on domain or dependent coupling require closed domain or open-window reproof | at the operation | 5
DEPENDENT_VALUES94 | "Borrow creation or a call consuming the affected domain." | Invariant windows | Borrow creation and calls consuming the affected domain require closed domain or reproof | at the operation | 2
DEPENDENT_VALUES95 | "Transfer, return, scope expiration or loan handback releasing the relevant custody or supplying a value under that domain." | Invariant windows | Transfer, return, scope end, and loan handback releasing custody or supplying a value under the domain require closed domain or reproof | at normal return | 2
DEPENDENT_VALUES96 | "Boundary/capability-carrying calls that may observe or expose the affected storage, including through authority not named as an argument." | Invariant windows | Boundary or capability calls that may observe or expose affected storage require closed domain or reproof | at a call | 3
DEPENDENT_VALUES97 | "A call need not close an unrelated domain window when its complete access, effect and outcome contracts prove that it cannot observe or expose the broken coupling, interfere with its witnesses or restoration, or release the custody excluding observers." | Invariant windows | A call may leave an unrelated domain window open only when complete access, effect, and outcome contracts prove no observation, exposure, witness interference, or custody release | at a call | 3
DEPENDENT_VALUES98 | "Unknown access/effects are treated conservatively." | Invariant windows | Calls with unknown access or effects must conservatively require closing domain windows | at a call | 3
DEPENDENT_VALUES99 | "This exception grants no new borrow authority and does not make an unfinished value valid;" | Invariant windows | The call window exception must not grant new borrow authority or validate unfinished values | at a call | 2
DEPENDENT_VALUES100 | "An unclosed window rejects with both the opening write or storage cast and the failing consumption point." | Invariant windows | Consumption with an unclosed invariant window must be rejected, reporting opening write or cast and consumption site | at the operation | 5
DEPENDENT_VALUES101 | "a dependent loan pins its witnesses, so a conflicting write cannot open a window in the first place." | Invariant windows | Conflicting writes to witnesses pinned by a dependent loan must be rejected so a window cannot open | at the operation | 2
DEPENDENT_VALUES102 | "Write-only access must restore validity from written values, structure, and supplied facts, never by reading its referent." | Invariant windows | Write-only access must re-establish validity from writes, structure, and supplied facts without reading the referent | at the operation | 2
DEPENDENT_VALUES103 | "Recoverable failure does not cancel a window." | Invariant windows | Recoverable failure must not cancel an open invariant window obligation | not stated | 3
DEPENDENT_VALUES104 | "Once established, a value cannot be discarded while its cleanup or obligations rely on a broken invariant." | Invariant windows | Discarding a value whose cleanup depends on a broken invariant must be rejected | at the operation | 2
DEPENDENT_VALUES105 | "Failed initial establishment instead creates no value, leaving raw storage under its existing storage claim." | Invariant windows | Failed initial establishment must not create a value; raw storage remains under existing storage claim only | not stated | 1
DEPENDENT_VALUES106 | "A crash may abandon an open window but supplies no survivor-safety theorem;" | Invariant windows | Crash may abandon open windows; compiler must not infer survivor safety from crash | not stated | 3
DEPENDENT_VALUES107 | "The ownership rules govern calls on detached values and disjoint siblings without weakening the domain fences below." | Invariant windows | Calls on detached values and disjoint siblings must follow ownership rules without weakening domain fences stated below | at a call | 2
DEPENDENT_VALUES108 | "Invalidate flow refinements atom by atom on overlapping written places;" | Mutation frames | On overlapping writes, flow refinements must be invalidated atom by atom per written place | at the operation | 2
DEPENDENT_VALUES109 | "The conservative call frame includes places reachable through exclusive borrows and separately authorized capability state." | Mutation frames | Default mutation frame for a call must include places reachable through exclusive borrows and authorized capability state | at a call | 2
DEPENDENT_VALUES110 | "Include argument/index evaluation writes as well as callee writes." | Mutation frames | Mutation frame must include argument and index evaluation writes in addition to callee writes | at a call | 2
DEPENDENT_VALUES111 | "Default-domain obligations and explicit domain qualifications must be re-established before consumption, not assumed to be unchanged values." | Mutation frames | Default-domain and explicit domain qualifications must be re-established before consumption after writes; must not assume unchanged values | at the operation | 5
DEPENDENT_VALUES112 | "Frames retain exact subject substitution across aliases, calls, and terminal transitions." | Mutation frames | Mutation frames must retain exact subject substitution across aliases, calls, and terminal transitions | not stated | 2
DEPENDENT_VALUES113 | "Where only collection-level origin is known, later field projections cannot recover narrower precision without new evidence." | Mutation frames | Field projections must not recover narrower fact precision from collection-level origin alone without new evidence | at the operation | 6
DEPENDENT_VALUES114 | "An unrepresentable origin or incomplete recursive summary uses the conservative ownership ceiling, never an empty write set." | Mutation frames | Unrepresentable origins or incomplete recursive summaries must use conservative ownership ceiling, not an empty write set | not stated | 2
DEPENDENT_VALUES115 | "Frame inference grants no borrow authority and cannot preserve facts through unmodeled external writes." | Mutation frames | Inferred mutation frames must not grant borrow authority or preserve facts across unmodeled external writes | not stated | 2

## Ambiguous sentences

Sentence | Reading A | Reading B
--- | --- | ---
"Runtime dependencies do not become const-generic arguments." | Any program value dependency at compile time must never be encoded as a const-generic argument, even if its value is statically known. | Only dependencies classified as runtime (not const-eligible) are forbidden as const-generic arguments; const-eligible dependencies may still become const-generic arguments per other rules. |
"An additional input domain need not survive a call unless the ordinary contract guarantees it." | Additional input domain facts always expire at call end unless an `ensures` or equivalent contract explicitly carries them forward. | Input domain facts may survive by ordinary call semantics even without an explicit guarantee; the sentence only says survival is not required. |
"If the original storage does not itself owe that qualification, the input fact may expire unless the contract promises it afterward." | Expiring input qualifications apply only when the underlying storage lacks that qualification; qualified storage always retains through the call. | Even when storage owes a qualification, call contracts might still allow input-only qualifications to expire unless promised afterward. |
"A call need not close an unrelated domain window when its complete access, effect and outcome contracts prove that it cannot observe or expose the broken coupling, interfere with its witnesses or restoration, or release the custody excluding observers." | The callee contract alone suffices to leave the window open; caller need not prove isolation. | Both caller context and callee contract must jointly show non-interference; the sentence does not say which side must prove it. |
"Include argument evaluation, callbacks and authorized capability state in that determination." | Non-closure of a window requires modeling argument evaluation, callbacks, and capability state in the non-interference proof. | Those factors are optional refinements; omission defaults to conservative treatment rather than mandatory modeling. |
"Checked implementations may derive a narrower complete frame; opaque or unknown dynamic calls retain the signature/authority ceiling." | Only implementations marked or proven checked may narrow frames; all others use the ceiling. | Any implementation the checker can analyze may narrow frames; opaque or unknown calls always use the ceiling regardless of source visibility. |
"A complete empty frame differs from an unknown one." | An empty frame proven complete permits preserving facts; an unknown frame must invalidate conservatively. | Empty and unknown frames differ only for diagnostics; both invalidate the same fact set unless stated otherwise elsewhere. |
"Body-derived frames are implementation evidence, not changes to public contract or specialization identity." | Narrower frames from bodies cannot change observable checking of public callers or specialization keys. | Body-derived frames may affect checking of the same function's internal code but must not alter the declared public contract text; whether caller-visible checking changes is not stated in this sentence. |

## Terms used without definition in this chapter

- Range predicates
- flow-sensitive facts
- in-scope program values
- exact values and places
- nominal types
- systems contract
- selected foundation
- mathematical source binders
- W-based profile
- Const index
- static specialization key
- licensed symbolic expression
- static application identity
- Runtime-capable generic index
- non-const value binder
- Stored witness
- View witness
- Flow fact
- erased proposition
- generic staging
- static eligibility
- canonical encoding
- semantic evaluation
- runtime-index compatibility
- Type relevance
- erased witness identity/multiplicity
- proof erasure
- proof-only values
- runtime descriptor view
- backing extent
- foreign stride
- row-major access
- Boundary output parameters
- stored witnesses (as postconditions)
- Dynamic-sized regions
- checked views
- provisioned buffers/storage
- runtime-sized activation storage
- Layout plans
- recasts
- proof tuple
- hidden witness allocation
- runtime-capable value binder
- storage realization
- activation-local
- const-indexed domain families
- binding time
- visible guard
- contracted decode/validator
- residual checks
- unproved refinement
- firmware truth
- routed authority
- dependent place
- read-loans
- layout parameter
- displayed qualification
- exact parameter/result substitution
- checked contract
- Machine-arrival requirements
- incoming edge
- stale entry facts
- backedge
- default domain
- field qualifications
- `dropped by`
- automatic cleanup selection
- uninhabited domain
- Zero-initializability / ZII
- zeroed storage representation
- domain-owned read/write restrictions
- governed field
- parent borrows
- invariant windows
- policy installation
- common fields
- active-case payloads
- case guard
- Bracketed scalar range suffixes
- Boundary provider slots
- exact requirement application
- provider planning / realization
- zero representation
- first declared case
- first possible case
- zero-constructible / gated type
- explicit omission
- complete coupling
- monotone establishment
- zero-validity
- Machine-owned storage
- runtime-indexed arrays
- `value: &mut T`
- invariant-window rules
- additional input domain
- qualified machine name
- access policy
- reader/writer grant / restriction
- `value: T in D`
- restoration obligation
- expiring input fact
- mutable loan
- cast on the reference / mutable views
- handback
- predicate debt
- establishment provenance
- custody and carry restrictions
- Contract-based repair
- exclusive custody
- `+D` rules
- restoration promise
- terminal call
- future guarantee
- recoverable outcomes
- multi-step repair
- intermediate view
- predicate-only membership
- mutation frame
- Collection invariants
- per-element-array-bounds
- denotation
- elementwise consequences
- compiler-derived proof debt
- `relax` mode
- runtime poison flag
- mutable truth bit
- Exclusive storage recasts
- deferred establishment
- target-establishment debt
- pending-view formation
- resource cleanup
- Borrowed-storage ownership windows
- restoration debt
- establishing store
- ownership restoration
- domain restoration
- detached values
- disjoint siblings
- consumption point
- broken coupling
- mutation frame / complete frame / empty frame
- signature/authority ceiling
- flow refinements
- subject substitution
- collection-level origin
- conservative ownership ceiling
- unmodeled external writes
- current analysis bounds

CHAPTER COMPLETE
