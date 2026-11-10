# Domains and qualification (lines 1–548)

Source: `wiki/spec/language/domains.md` lines 1–548 (read).

## Rules

rule id | the exact sentence, quoted | the heading it is under | what must be checked, in plain words | when it must hold | lane
--- | --- | --- | --- | --- | ---
DOMAINS1 | "A domain attaches a static theory to a carrier without adding a runtime tag, wrapper, hidden storage, or second object model." | Domains and qualification | Domain qualification must not introduce a runtime tag, wrapper, hidden storage, or second object model | not stated | 5
DOMAINS2 | "Predicate requirements, semantic contributions, authorized establishment and access routes, cleanup selection, and transparent aliases are independently normalized aspects of that theory." | Domains and qualification | Each listed domain-theory aspect must be normalized independently | not stated | 5
DOMAINS3 | "Multiplicity, content, carry, and provider evidence keep their own judgments." | Domains and qualification | Multiplicity, content, carry, and provider evidence must be judged separately from domain theory | not stated | SHARED
DOMAINS4 | "Qualification never selects or changes storage representation, capacity, live length, layout, or collection kind." | Domains and qualification | Qualification must not select or change storage representation, capacity, live length, layout, or collection kind | not stated | 5
DOMAINS5 | "In particular, `[u8; N] in D` is the same exact `N`-byte fixed-array carrier as `[u8; N]`; its `.len` is `N`, and removing `D` with a cast changes no bits or geometry." | Domains and qualification | Qualified fixed array must keep the same carrier geometry and bit pattern when the domain is removed by cast | not stated | 5
DOMAINS6 | "A variable-fill owner must name an explicit variable-length carrier such as `FixedVec<u8, N> in D` or `Vec<u8> in D`." | Domains and qualification | Variable-fill owners must use an explicit variable-length carrier type, not a fixed array qualification alone | not stated | 6
DOMAINS7 | "Ordinary accessor machines may lend that owner's live prefix as a qualified `&[u8] in D` view." | Domains and qualification | Accessor machines may expose owner's live prefix as qualified slice view when otherwise valid | at the operation | 2
DOMAINS8 | "Requirement spelling follows [unified requirement clauses](state_contracts.md#requirement-clauses)." | Declaration and membership | Domain and membership requirement clauses must follow unified requirement-clause spelling | not stated | 8
DOMAINS9 | "Domain declarations end with `;`, not a body." | Declaration and membership | Domain declarations must end with semicolon and must not have a body | not stated | 5
DOMAINS10 | "Empty `{ }` is also retired, not an alternative spelling." | Declaration and membership | Empty brace domain bodies must be rejected as an alternative spelling | not stated | 5
DOMAINS11 | "Predicates belong in `requires`, establishment routes and access restrictions belong to the policy selected by `access`, and executable operations are separate machines." | Declaration and membership | Predicates must be in requires; establishment and access restrictions must be in the selected access policy; executable operations must be separate machines | not stated | 5
DOMAINS12 | "A domain without predicate requirements remains distinct from one explicitly requiring `true`; neither form has an executable body." | Declaration and membership | Predicate-free and `requires true` domains must remain distinct types; neither may have an executable body | not stated | 5
DOMAINS13 | "In a domain's `requires` predicates and a data declaration's default-domain `requires`, `self` denotes the exact value being constrained:" | Declaration and membership | In domain and default-domain requires, `self` must denote the exact constrained value | not stated | 5
DOMAINS14 | "`Player::` determines the domain's carrier." | Declaration and membership | The domain's carrier must be determined by the `Carrier::` prefix in the domain declaration | not stated | 5
DOMAINS15 | "In data predicates the carrier is the containing data type." | Declaration and membership | In data default-domain predicates the carrier must be the containing data type | not stated | 1
DOMAINS16 | "Generic arguments are substituted in either context." | Declaration and membership | Generic arguments must be substituted into domain and data predicates | not stated | 7
DOMAINS17 | "Membership `candidate in AtMost<8>` substitutes that exact candidate for `self`; users supply no subject argument." | Declaration and membership | Membership tests must substitute the candidate for `self` without a separate subject argument | at the operation | 5
DOMAINS18 | "Each declaration has one subject." | Declaration and membership | Each domain or data declaration must have exactly one logical subject | not stated | 5
DOMAINS19 | "`self` is a logical subject, not a field, generic index, runtime parameter, implicit borrow, move, receiver or source of authority." | Declaration and membership | `self` in predicates must be treated only as a logical subject, not as field, parameter, borrow, move, receiver, or authority source | not stated | 5
DOMAINS20 | "Its scope is these predicates; it cannot be rebound and does not enter a separately declared machine or its contracts." | Declaration and membership | Predicate `self` must stay scoped to those predicates, must not be rebindable, and must not enter separate machine contracts | not stated | 5
DOMAINS21 | "Predicate calls retain their ordinary formation and evaluation obligations." | Declaration and membership | Predicate calls inside domain requires must satisfy ordinary formation and evaluation obligations | not stated | 8
DOMAINS22 | "A predicate may not assume its own goal to justify forming a call, projection or other partial term." | Declaration and membership | A domain predicate must not assume its own proof goal to form calls, projections, or partial terms | not stated | 8
DOMAINS23 | "This replaces explicit domain subject binders: `domain Player::Alive(player)` is rejected, as is a subject parameter on `data`." | Declaration and membership | Explicit domain subject binders and data subject parameters must be rejected | not stated | 5
DOMAINS24 | "Machine receiver syntax and the `Self` conformance type remain absent." | Declaration and membership | Machine receiver syntax and `Self` conformance type must not appear in the domain model | not stated | 10
DOMAINS25 | "Existing unqualified data-field names in data predicates remain field references; `self.field` names them explicitly." | Declaration and membership | Unqualified names in data predicates must resolve as field references; `self.field` must name the same fields explicitly | not stated | 1
DOMAINS26 | "A domain over `u16` classifies `u16` values, not values of any other carrier that happen to represent the same number." | Carrier-exact membership | A domain must classify only values of its declared carrier type | not stated | 5
DOMAINS27 | "Before substituting a candidate into the domain predicate, form membership against the domain's exact resolved applied carrier." | Carrier-exact membership | Membership must be formed against the domain's exact resolved applied carrier before predicate substitution | at the operation | 5
DOMAINS28 | "This rule applies equally to proof facts, contracts, qualifications, and executable membership tests." | Carrier-exact membership | Carrier-exact membership must apply in proofs, contracts, qualifications, and executable tests | not stated | 5
DOMAINS29 | "For `domain u16::Small requires self <= 10;`, `7u16 in Small` has the right carrier, while `7u8 in Small` is ill-formed even though seven fits both types." | Carrier-exact membership | Membership with a non-exact carrier operand must be rejected as ill-formed | at the operation | 5
DOMAINS30 | "An explicit exact conversion, `(7u8 as u16) in Small`, precedes membership in the other carrier's domain." | Carrier-exact membership | Cross-carrier membership must be preceded by an explicit exact conversion to the domain carrier | at the operation | 5
DOMAINS31 | "Anonymous literals follow ordinary expected-carrier landing; a suffix or an already typed operand cannot be re-landed by membership." | Carrier-exact membership | Membership must not re-land anonymous literals, suffixed literals, or already-typed operands onto a different carrier | at the operation | 5
DOMAINS32 | "Cross-carrier numeric equality remains a different judgment and grants no domain membership or conversion." | Carrier-exact membership | Cross-carrier numeric equality must not imply domain membership or implicit conversion | not stated | 5
DOMAINS33 | "Carrier correspondence uses retained type/declaration identities and the exact generic substitutions and declared type equations." | Carrier-exact membership | Carrier correspondence must use retained identities, exact generic substitutions, and declared type equations | not stated | 5
DOMAINS34 | "Matching domain names, matching record fields, or erasing an operand to its mathematical value cannot replace it." | Carrier-exact membership | Carrier correspondence must not be replaced by matching domain names, matching fields, or mathematical erasure | not stated | 5
DOMAINS35 | "A carrier-generic domain family has a separately applied carrier at each use." | Carrier-exact membership | Each use of a carrier-generic domain family must resolve its own applied carrier | not stated | 7
DOMAINS36 | "Sharing its definition does not transfer membership between those carriers." | Carrier-exact membership | Sharing a generic domain definition must not transfer membership across different applied carriers | not stated | 7
DOMAINS37 | "Scalar interval relationships use ordinary propositions, not a bracketed type suffix (`scalar-range-contract-spelling`)." | Carrier-exact membership | Scalar intervals must use ordinary propositions, not bracketed type-suffix spelling | not stated | 5
DOMAINS38 | "A call's checked postconditions apply to its exact result even when the receiving local is declared with the bare carrier." | Carrier-exact membership | Checked call postconditions must apply to the exact result type even when the local omits domain qualification | at a call | 3
DOMAINS39 | "A parameter `value: T in D` accepts only an argument whose static type carries the exact named domain `D`, and gives the body the fact `value in D`." | Carrier-exact membership | Qualified parameters must accept only arguments with the exact domain `D` and must give the body `value in D` | at a call | 5
DOMAINS40 | "A caller holding only a proof of membership adds the domain with a [domain cast](#domain-casts), as in `value as +D`." | Carrier-exact membership | Adding a domain to a bare carrier when only membership is proved must use an explicit positive domain cast | at the operation | 5
DOMAINS41 | "Equal predicates do not merge domain identities." | Carrier-exact membership | Distinct domain names with equal predicates must remain distinct identities | not stated | 5
DOMAINS42 | "Every `requires` proposition must hold." | Carrier-exact membership | Every `requires` proposition in domain declarations must be provable or satisfiable as required | not stated | 8
DOMAINS43 | "A Boolean-returning validator is not a logical declaration merely because it has call syntax." | Carrier-exact membership | A Boolean-returning machine call must not be treated as a logical `requires` declaration solely from call syntax | not stated | 8
DOMAINS44 | "checking qualification does not execute a validator." | Carrier-exact membership | Qualification checking must not execute validator machines | not stated | 5
DOMAINS45 | "Each callable descriptor in the policy's establishment list names an exact trait requirement or concrete machine as an alternative authorized origin, not an invocation." | Carrier-exact membership | Establishment-list entries must name authorized origins, not denote invocations | not stated | 8
DOMAINS46 | "Predicates alone establish predicate-only membership." | Carrier-exact membership | Membership without routes may be established by predicates alone when no routed provenance is required | not stated | 5
DOMAINS47 | "A routed domain additionally needs exact authorized provenance, even when all its predicates are proved." | Carrier-exact membership | Routed domains must require exact authorized provenance in addition to proved predicates | not stated | 8
DOMAINS48 | "A predicate-free domain with establishment routes still requires that provenance." | Carrier-exact membership | Predicate-free routed domains must still require establishment provenance | not stated | 8
DOMAINS49 | "A declaration with neither predicates nor establishment routes adds no predicate or issuer obligation: a valid bare `i32` may be qualified with `as +Km` without an owner grant." | Carrier-exact membership | Predicate- and route-free domains impose no issuer obligation; bare carriers may gain qualification via `as +D` without owner grant | at the operation | 5
DOMAINS50 | "absence of an issuer restriction does not waive those obligations." | Carrier-exact membership | Missing issuer restrictions must not waive independent reader/writer obligations from the access policy | not stated | 2
DOMAINS51 | "A predicate-free, route-free tag on a linear value may be introduced by an `as +D` cast; the same value is transferred, not copied, and its ownership obligations remain." | Carrier-exact membership | Introducing a tag on a linear value via `as +D` must transfer the value and preserve ownership obligations | at the operation | 2
DOMAINS52 | "Neither the carrier's multiplicity nor predicates or routes on unrelated domains can add a restriction." | Carrier-exact membership | Establishment restrictions must come only from the selected domain theory, not carrier multiplicity or unrelated domains | not stated | 5
DOMAINS53 | "Adding another domain over a carrier cannot change an existing domain's minting rules." | Carrier-exact membership | Additional domains over a carrier must not alter another domain's minting rules | not stated | 5
DOMAINS54 | "No carrier-wide custody marker or sibling-domain scan substitutes for declared establishment routes." | Carrier-exact membership | Establishment must use declared routes; carrier-wide markers or sibling scans must not substitute | not stated | 8
DOMAINS55 | "The domain owner has no ambient minting privilege." | Carrier-exact membership | Domain owners must not have ambient minting privilege without a route | not stated | 8
DOMAINS56 | "A result annotation or `ensures` creates an obligation, not evidence." | Carrier-exact membership | Result annotations and `ensures` must create proof obligations, not automatic evidence | not stated | 8
DOMAINS57 | "An exact-machine route authorizes only the named declaration's invocation." | Carrier-exact membership | Exact-machine establishment routes must authorize only the named declaration's invocation | at a call | 3
DOMAINS58 | "This [limited visibility exception](../resources/authority.md#private-issuer-routes) does not expose private carriers or relax public predicate/signature visibility." | Carrier-exact membership | Private issuer routes must not expose private carriers or relax public predicate or signature visibility | not stated | 5
DOMAINS59 | "Assignment, move, and permitted copy transport existing qualification between places of the same declared type; rebuilding equal carrier fields does not." | Carrier-exact membership | Assignment, move, and permitted copy must transport qualification between same declared-type places; field rebuild must not | at the operation | 2
DOMAINS60 | "Mutation invalidates subject-bound facts unless preserved or re-established." | Carrier-exact membership | Mutation must invalidate subject-bound facts unless they are preserved or re-established | at the operation | 2
DOMAINS61 | "Declared default-domain obligations are enforced at [consumption points](dependent_values.md#invariant-windows), not replaced by an assumption that old flow facts survived a write." | Carrier-exact membership | Default-domain obligations must be enforced at consumption points after writes, not assumed from stale flow facts | at the operation | 2
DOMAINS62 | "A carrier-qualified domain owns its visibility independently of its carrier." | Visibility and foreign declarations | Domain visibility must be independent of carrier visibility | not stated | 5
DOMAINS63 | "It is private unless declared `public`; publishing it does not publish a private carrier." | Visibility and foreign declarations | Domains must be private unless marked public; publishing a domain must not publish a private carrier | not stated | 5
DOMAINS64 | "Public signatures must be authorized to name both declarations under [package boundaries](../packages/boundaries.md)." | Visibility and foreign declarations | Public signatures naming a domain and its carrier must be authorized under package boundary rules | not stated | 9
DOMAINS65 | "Its declarations cannot change the carrier's validity, reveal private representation, or alter another domain's establishment routes." | Visibility and foreign declarations | Foreign domains over a carrier must not change carrier validity, reveal private representation, or alter other domains' routes | not stated | 5
DOMAINS66 | "importing the declaring module exposes its directly declared public domains in the importing source file; importing one domain exposes only that declaration." | Visibility and foreign declarations | Import exposure of domains must follow ordinary import scope: module import exposes public domains in that file; single-domain import exposes only that domain | not stated | 5
DOMAINS67 | "Sibling files, descendant modules, and transitive imports add no exposure." | Visibility and foreign declarations | Sibling files, descendant modules, and transitive imports must not expose domains beyond direct import rules | not stated | 5
DOMAINS68 | "Importing a carrier or ordinary machine does not expose unrelated domains merely because their source was loaded." | Visibility and foreign declarations | Importing a carrier or machine must not expose unrelated domains from the same source | not stated | 5
DOMAINS69 | "Direct qualified selection uses the [declaring-owner-first address](modules.md#foreign-attached-declaration-paths)." | Visibility and foreign declarations | Direct qualified domain selection must use declaring-owner-first addressing | not stated | 5
DOMAINS70 | "Graph preparation supplies their identities before name resolution; an absent package field cannot identify both a root and an injected library." | Visibility and foreign declarations | Name resolution must receive domain and carrier owner identities from graph preparation; one absent package field must not identify both root and injected library | not stated | 9
DOMAINS71 | "Repeated exposure of one exact declaration is not a collision; distinct visible declarations competing for the same carrier-qualified case, domain, or machine name reject without inherent-declaration or import-order priority." | Visibility and foreign declarations | Repeated exposure of one declaration is allowed; competing distinct visible carrier-qualified names must reject without priority rules | not stated | 5
DOMAINS72 | "Importing enables selection, not membership, minting, or implicit operator changes." | Visibility and foreign declarations | Import must enable selection only, not membership, minting, or implicit operator changes | not stated | 5
DOMAINS73 | "A value carrying a foreign qualification retains its exact identity and evidence without activating the domain for authored lookup." | Visibility and foreign declarations | Foreign-qualified values must retain exact identity and evidence without activating foreign domain lookup in the importer | not stated | 5
DOMAINS74 | "Carrying it does not grant additional dependency reach or expose other extensions." | Visibility and foreign declarations | Carrying a foreign-qualified value must not extend dependency reach or expose other extensions | not stated | 9
DOMAINS75 | "The former `established by`, `read by` and `written by` clauses are retired, not parallel ways of configuring the policy." | Domain-owned read and write authority | Retired access clauses must not be accepted as parallel policy configuration | not stated | 5
DOMAINS76 | "`access` belongs to the domain, not a field-private modifier." | Domain-owned read and write authority | Access policy selection must belong to the domain declaration, not field-private modifiers | not stated | 5
DOMAINS77 | "Explicit establishment, reader and writer lists are independent ceilings: omission leaves that axis unrestricted by this policy, an empty list permits nobody, and a nonempty list contains alternative exact callable identities." | Domain-owned read and write authority | Establishment, reader, and writer lists must be independent ceilings with the stated omission, empty-list, and disjunctive-alternative semantics | not stated | 8
DOMAINS78 | "Authorizing writers does not authorize readers or issuers." | Domain-owned read and write authority | Writer authorization must not imply reader or issuer authorization | not stated | 8
DOMAINS79 | "Whole-subject, field, inherited and repeated restrictions intersect without order-dependent overrides." | Domain-owned read and write authority | Access restrictions must intersect conjunctively without order-dependent override | not stated | 8
DOMAINS80 | "A field rule alone does not require a whole-object rule." | Domain-owned read and write authority | A field access rule must be enforceable without a whole-object rule | not stated | 8
DOMAINS81 | "Backing/loan authority still limits every operation; default-permissive caller authorization manufactures none of it." | Domain-owned read and write authority | Backing and loan authority must still bound operations; permissive access policy must not manufacture backing or loans | not stated | 2
DOMAINS82 | "An ordinary named `AccessPolicy` computes the plan at compile time under the [access-plan contract](../resources/access.md#plans-and-admitted-supply)." | Domain-owned read and write authority | Named AccessPolicy must compute AccessPlan at compile time under the access-plan contract | not stated | 8
DOMAINS83 | "It consumes the semantic schema, not a layout." | Domain-owned read and write authority | Access plan computation must use semantic schema, not layout | not stated | 8
DOMAINS84 | "establishment checks their correspondence with actual backing and custody." | Domain-owned read and write authority | Establishment must check layout correspondence with actual backing and custody | not stated | 2
DOMAINS85 | "The separate `Access<P, T>` source wrapper is retired in favor of domain-qualified values/views." | Domain-owned read and write authority | The Access<P,T> wrapper syntax must be rejected in favor of domain-qualified values and views | not stated | 5
DOMAINS86 | "External/Atomic projections still use restricted operation accessors where an ordinary field loan would promise invalid stable/exclusive backing." | Domain-owned read and write authority | External/Atomic projections must use restricted accessors when ordinary field loans would be invalid | not stated | 10
DOMAINS87 | "An exact machine is authorized during its invocation; a requirement authorizes the invocation through that requirement and its selected implementation, not an unrelated direct call to a satisfying machine." | Domain-owned read and write authority | Access routes must authorize exact-machine invocation or requirement-selected implementation, not unrelated direct satisfying-machine calls | at a call | 3
DOMAINS88 | "No whole-file, package, namespace, carrier-owner, or same-trait privilege follows." | Domain-owned read and write authority | Access authorization must not grant whole-file, package, namespace, carrier-owner, or same-trait privilege | not stated | 8
DOMAINS89 | "These clauses change neither declaration visibility nor package dependency authority." | Domain-owned read and write authority | Access policy clauses must not change declaration visibility or package dependency authority | not stated | 9
DOMAINS90 | "The policy applies to the exact qualified value/storage occurrence and its structural subplaces, not every value of the carrier." | Governed subjects and operations | Policy must apply only to the exact qualified occurrence and structural subplaces | not stated | 8
DOMAINS91 | "Following a stored reference additionally obeys the referent's own authority and loan rules; the policy does not grant access to arbitrary referenced storage." | Governed subjects and operations | Stored-reference access must obey referent authority and loans; policy must not grant arbitrary referenced storage access | at the operation | 2
DOMAINS92 | "Introducing a foreign domain cannot impose a policy on unrelated values or existing domains." | Governed subjects and operations | Foreign domains must not impose policies on unrelated values or existing domains | not stated | 5
DOMAINS93 | "Reading stored contents, including discriminants, comparisons, codecs, and runtime reflection, requires read authority." | Governed subjects and operations | Listed read operations must require domain read authority | at the operation | 8
DOMAINS94 | "Changing contents, including projected writes, whole-value replacement of a governed place, and atomic stores, requires write authority." | Governed subjects and operations | Listed write operations must require domain write authority | at the operation | 8
DOMAINS95 | "Read-modify-write operations require both." | Governed subjects and operations | Read-modify-write operations must require both read and write authority | at the operation | 8
DOMAINS96 | "The operation's semantics determine its access, not whether its parameter spells `&` or `&mut`." | Governed subjects and operations | Access classification must follow operation semantics, not reference parameter spelling | at the operation | 8
DOMAINS97 | "An authorized route removes only its domain's access restriction for the supplied subject and applicable axis." | Governed subjects and operations | An authorized access route must remove only that domain's restriction for the supplied subject and axis | at a call | 8
DOMAINS98 | "It does not create backing, widen a borrow, bypass exclusivity, prove a predicate, waive cleanup, or confer concurrent access." | Governed subjects and operations | Authorized access routes must not create backing, widen borrows, bypass exclusivity, prove predicates, waive cleanup, or confer concurrent access | at a call | 2
DOMAINS99 | "In particular, write authorization does not make an ordinary shared reference writable." | Governed subjects and operations | Write authorization must not make ordinary shared references writable | not stated | 2
DOMAINS100 | "Atomic shared writes require both their ordinary atomic authority and any applicable domain write authorization." | Governed subjects and operations | Atomic shared writes must satisfy both atomic authority and applicable domain write authorization | at the operation | 10
DOMAINS101 | "Type/field metadata and already established proof facts are not new runtime reads; they cannot manufacture an executable view." | Governed subjects and operations | Metadata and established facts must not count as runtime reads that manufacture executable views | not stated | 8
DOMAINS102 | "Constructing a fresh valid carrier precedes establishing its qualification." | Establishment, transport, and helper calls | Fresh carrier construction must precede qualification establishment | at the operation | 5
DOMAINS103 | "Issuance checks the existing carrier, predicate, provenance, and custody rules and installs the access policy; it does not retroactively require a constructor to read or write an already-protected value." | Establishment, transport, and helper calls | Issuance must check carrier, predicates, provenance, and custody and must not retroactively require constructors to access already-protected values | at the operation | 8
DOMAINS104 | "Existing governed storage cannot use reconstruction, zero initialization, or an invariant window to bypass its policy." | Establishment, transport, and helper calls | Reconstruction, zero init, and invariant windows must not bypass installed access policies on governed storage | not stated | 2
DOMAINS105 | "Introducing a policy requires compatible live loans: no surviving alias may retain an access path that violates the policy for the governed lifetime." | Establishment, transport, and helper calls | Policy introduction must reject incompatible live loans that violate the policy | not stated | 2
DOMAINS106 | "Moving an intact governed value transports its policy and custody." | Establishment, transport, and helper calls | Move of intact governed value must transport policy and custody | at the operation | 2
DOMAINS107 | "A permitted whole-value copy preserves the policy on the copy; it does not expose its fields as unrestricted values." | Establishment, transport, and helper calls | Permitted whole-value copy must preserve policy on the copy and must not expose fields as unrestricted | at the operation | 2
DOMAINS108 | "Compiler-generated byte movement is not source-level observation or permission to inspect the bytes." | Establishment, transport, and helper calls | Compiler byte movement must not count as source-level observation or inspection permission | not stated | 2
DOMAINS109 | "Public field names or ownership of the enclosing record do not waive an installed field policy." | Establishment, transport, and helper calls | Public fields or enclosing ownership must not waive an installed field policy | not stated | 8
DOMAINS110 | "Copyability does not grant read authority or remove qualification." | Establishment, transport, and helper calls | Copyable primitives must not gain read authority or lose qualification from copyability alone | at the operation | 2
DOMAINS111 | "Argument admission still proves carrier validity, qualification, lifetime, and the required ordinary loan." | Establishment, transport, and helper calls | Forwarding restricted values must still prove carrier validity, qualification, lifetime, and required loan at admission | at a call | 3
DOMAINS112 | "Bare parameter spelling, wrappers, callbacks, and indirect calls cannot replace that argument-admission proof." | Establishment, transport, and helper calls | Parameter spelling, wrappers, callbacks, and indirect calls must not substitute for argument-admission proof | at a call | 3
DOMAINS113 | "Helpers cannot retain the grant after that scope ends; suspension retains its ordinary call/loan obligations rather than extending them." | Establishment, transport, and helper calls | Delegated access must end with the authorizing invocation scope; suspension must not extend grants | at a call | 3
DOMAINS114 | "Returning a restricted reference transports the restriction, not a fresh unrestricted grant." | Establishment, transport, and helper calls | Returned restricted references must carry the restriction, not a fresh unrestricted grant | at normal return | 2
DOMAINS115 | "Access permission alone does not prove a mutex's exclusion, ordering, or payload-custody protocol." | Establishment, transport, and helper calls | Domain access permission must not prove mutex exclusion, ordering, or payload custody | not stated | 10
DOMAINS116 | "A policy introduced solely on a temporary borrowed view governs that loan and its surviving descendants; it does not permanently install a new policy on an otherwise unqualified lender." | Temporary qualified views | Temporary-view policy must govern only the loan cohort and must not permanently install policy on the lender | not stated | 2
DOMAINS117 | "If the domain names establishment routes, its introduction needs the exact authorized or existing evidence; neither a reference annotation nor a cast mints that evidence." | Temporary qualified views | Temporary introduction on routed domains must have authorized evidence; references and casts must not mint it | not stated | 8
DOMAINS118 | "While the qualified loan or any descendant remains live, the applicable access policy must be respected." | Temporary qualified views | Live qualified loans and descendants must respect the applicable access policy | on every edge | 2
DOMAINS119 | "A cast that removes a non-owning qualification forgets its displayed meaning, but does not end the loan or authorize broader access to still-governed backing." | Temporary qualified views | Non-owning qualification removal via cast must not end the loan or broaden access to governed backing | at the operation | 5
DOMAINS120 | "A bare returned view therefore cannot launder the restriction beyond the issuing call." | Temporary qualified views | Returned views must not launder access restrictions beyond the issuing call | at normal return | 2
DOMAINS121 | "Returning from a function alone is insufficient if a derived view survives it." | Temporary qualified views | Function return must not end temporary view obligations while derived views survive | at normal return | 2
DOMAINS122 | "Ending one temporary view does not clear an independently governed value's policy, mint membership or discharge a resource claim." | Temporary qualified views | Ending a temporary view must not clear independent policies, mint membership, or discharge resource claims | not stated | 2
DOMAINS123 | "Every applicable atom must authorize the requested access." | Composition and removal | Every applicable access-policy atom must authorize the requested access | at the operation | 8
DOMAINS124 | "Alternatives within one list are disjunctive; independent domains and inherited policies compose conjunctively." | Composition and removal | List alternatives must be disjunctive; independent domains must compose conjunctively | not stated | 8
DOMAINS125 | "Alias expansion cannot drop a constituent's policy, and a child domain cannot widen a parent's access." | Composition and removal | Alias expansion must retain constituent policies; child domains must not widen parent access | not stated | 8
DOMAINS126 | "Changing a route set is an interface change, not a private-body detail." | Composition and removal | Access route set changes must be treated as interface changes | not stated | 8
DOMAINS127 | "Removing a displayed qualification never increases access." | Composition and removal | Removing displayed qualification must not increase access | at the operation | 5
DOMAINS128 | "A bare or recast view must retain the governed storage's access restrictions; if the target/use requires broader authority, it rejects." | Composition and removal | Bare or recast views must retain governed access restrictions; uses needing broader authority must reject | at the operation | 5
DOMAINS129 | "A `-D` entry cannot discharge the policy merely because a domain is non-owning or has no predicates." | Composition and removal | Negative domain casts must not discharge policy solely because the domain is non-owning or predicate-free | at the operation | 5
DOMAINS130 | "These policies add no runtime permission bits, storage wrapper, alias/newtype declaration form, or field-private syntax." | Composition and removal | Access policies must not introduce runtime permission bits, wrappers, alias/newtype forms, or field-private syntax | not stated | 8
DOMAINS131 | "Mutex exclusion, ordering and payload custody still require their own checked protocol; caller authorization alone proves none." | Composition and removal | Mutex protocols must be proved separately; caller authorization must not substitute | not stated | 10
DOMAINS132 | "`dropped by` selects one exact independent static cleanup machine for an owned qualified occurrence." | Domain-selected automatic cleanup | Each owned qualified occurrence may select at most one exact cleanup machine via `dropped by` | not stated | 2
DOMAINS133 | "It names the operation the compiler invokes at cleanup, not a list of callers permitted to discard the value." | Domain-selected automatic cleanup | `dropped by` must name the compiler-invoked cleanup operation, not permitted discard callers | not stated | 2
DOMAINS134 | "The carrier's owning package authorizes the selection." | Domain-selected automatic cleanup | Cleanup machine selection must be authorized by the carrier's owning package | not stated | 9
DOMAINS135 | "The selected machine has an explicit mutable reference parameter for the carrier and must meet the [cleanup contract](ownership.md#consumers-and-cleanup)." | Domain-selected automatic cleanup | Selected cleanup machines must take explicit mutable reference to carrier and satisfy the cleanup contract | not stated | 2
DOMAINS136 | "Name resolution is exact: no overload guessing, trait/satisfier discovery, or lookup by the name `drop`." | Domain-selected automatic cleanup | Cleanup selection resolution must be exact with no overload guessing, trait discovery, or `drop` name lookup | not stated | 2
DOMAINS137 | "This clause introduces no `Drop` trait or boundary requirement." | Domain-selected automatic cleanup | `dropped by` must not introduce a Drop trait or boundary requirement | not stated | 7
DOMAINS138 | "The result permits zero or one distinct selected machine application." | Domain-selected automatic cleanup | Normalized cleanup selection must permit zero or one distinct machine application per owned subject | not stated | 2
DOMAINS139 | "If the parent selects none, a child may add one." | Domain-selected automatic cleanup | Child domains may add a dropper when parent selects none | not stated | 5
DOMAINS140 | "A different machine or different application conflicts." | Domain-selected automatic cleanup | Conflicting cleanup machine selections must reject | not stated | 2
DOMAINS141 | "A child cannot override an inherited selection; conjunction does not order or chain droppers." | Domain-selected automatic cleanup | Child domains must not override inherited dropper; multiple droppers must not chain | not stated | 2
DOMAINS142 | "Reject conflicting concrete qualifications; generic selection must remain consistent under substitution." | Domain-selected automatic cleanup | Conflicting concrete cleanup qualifications must reject; generic selection must stay consistent under substitution | not stated | 7
DOMAINS143 | "Fields are separate owned subjects with their own cleanup plans, not competing selections on their enclosing record." | Domain-selected automatic cleanup | Field cleanup must be per-field owned subject, not competing record-level selection | not stated | 1
DOMAINS144 | "Zero selected operations permits ordinary structural cleanup only where disposal is otherwise legal; it does not discharge an unresolved linear claim." | Domain-selected automatic cleanup | No selected dropper allows structural cleanup only when legal; it must not discharge unresolved linear claims | not stated | 2
DOMAINS145 | "Selection does not itself prove the hook's preconditions or give it read/write or issuance authority." | Domain-selected automatic cleanup | Cleanup selection must not prove hook preconditions or grant read/write/issuance authority | not stated | 2
DOMAINS146 | "Borrowing does not authorize disposal of the referent." | Domain-selected automatic cleanup | Borrowing must not authorize disposal of the referent | not stated | 2
DOMAINS147 | "Alias expansion and domain removal cannot remove cleanup debt or evade a selection conflict." | Domain-selected automatic cleanup | Alias expansion and domain removal must not remove cleanup debt or evade selection conflicts | not stated | 2
DOMAINS148 | "Domains classify values within the carrier's ordinary validity." | Refinement and executable membership | Domain membership must stay within the carrier's ordinary validity | not stated | 5
DOMAINS149 | "A domain cannot license observing a value outside its default domain." | Refinement and executable membership | Domains must not license observing values outside their default domain | not stated | 5
DOMAINS150 | "Establishing membership requires all carrier, predicate, and provenance obligations; the target qualification cannot be assumed to prove itself." | Refinement and executable membership | Membership establishment must prove carrier, predicate, and provenance; target qualification must not assume itself | not stated | 8
DOMAINS151 | "Mentioning an uninhabited domain or declaring a machine parameter qualified by it is legal." | Uninhabited domains | Uninhabited domains may appear in mentions and qualified parameters | not stated | 5
DOMAINS152 | "each caller must establish that contract for its actual arguments." | Uninhabited domains | Callers to machines with uninhabited qualified parameters must establish the entry contract for actual arguments | at a call | 3
DOMAINS153 | "Conclusions proved under an impossible premise remain conditional and cannot manufacture an inhabitant or discharge that premise at a reachable call." | Uninhabited domains | Proofs under impossible premises must stay conditional and must not manufacture inhabitants or discharge premises at reachable calls | not stated | 8
DOMAINS154 | "Construction, qualification, argument supply, and default-domain establishment must still prove the obligations they require." | Uninhabited domains | Construction, qualification, calls, and default-domain establishment must still prove required obligations even when domain may be uninhabited | not stated | 8
DOMAINS155 | "Zero initialization and invariant windows provide no exemption." | Uninhabited domains | Zero init and invariant windows must not exempt membership or default-domain proof obligations | not stated | 2
DOMAINS156 | "An unproved membership obligation rejects that attempted establishment; it need not diagnose whether the domain is uninhabited or the proof automation is insufficient." | Uninhabited domains | Failed membership proof must reject the establishment attempt without requiring uninhabitability diagnosis | not stated | 8
DOMAINS157 | "Stronger proof automation must not make a well-formed declaration illegal merely by discovering a contradiction." | Uninhabited domains | Proof automation discovering contradiction must not invalidate well-formed domain declarations | not stated | 8
DOMAINS158 | "`A::B::C` is a single-parent refinement of `A::B`: its predicate requirements include the parent's requirements." | Refinement and tests | Single-parent domain refinements must include parent predicate requirements | not stated | 5
DOMAINS159 | "Multiple-parent predicate reuse is explicit, as `requires value in X & Y`." | Refinement and tests | Multiple-parent predicate reuse must be explicit via intersection membership syntax | not stated | 5
DOMAINS160 | "Establishing the child also requires the parent's routed provenance when present; nesting a name cannot turn a routed parent into predicate-only membership." | Refinement and tests | Child establishment must inherit parent routed provenance when present | not stated | 8
DOMAINS161 | "A type carrying the child carries every ancestor, and a [domain cast](#domain-only-casts) names each of them." | Refinement and tests | Qualified child types must carry all ancestor domains; casts must name each ancestor | not stated | 5
DOMAINS162 | "An ordinary value match is ordered, so overlapping domain patterns are permitted and the first matching arm wins." | Refinement and tests | Domain match patterns may overlap; first matching arm must win | at the operation | 1
DOMAINS163 | "each transition proves its destination's requirements." | Refinement and tests | Each match transition must prove destination requirements | on every edge | 8
DOMAINS164 | "A match without a wildcard over a known domain union must be exhaustive." | Refinement and tests | Non-wildcard matches over known domain unions must be exhaustive | not stated | 1
DOMAINS165 | "Unordered domain-union reasoning requires mutually exclusive alternatives; a child and its parent are not such alternatives." | Refinement and tests | Unordered union reasoning must use mutually exclusive alternatives; parent and child are not alternatives | not stated | 8
DOMAINS166 | "An executable domain test requires pure, finite, runtime-checkable predicates." | Refinement and tests | Executable domain tests must use pure finite runtime-checkable predicates | at the operation | 5
DOMAINS167 | "Subdomain tests check the parent before the child's added predicates." | Refinement and tests | Subdomain runtime tests must check parent predicates before child predicates | at the operation | 5
DOMAINS168 | "Quantifiers, opaque proof calls, and nonexecutable facts do not become runtime tests." | Refinement and tests | Non-executable predicate forms must not become runtime domain tests | not stated | 8
DOMAINS169 | "A test that cannot establish membership rejects; no hidden runtime domain tag is added." | Refinement and tests | Failed runtime membership test must reject; compiler must not add hidden runtime domain tags | at the operation | 5
DOMAINS170 | "For routed membership, predicate tests still require the retained authorized provenance." | Refinement and tests | Routed membership runtime tests must still require authorized provenance | at the operation | 8
DOMAINS171 | "Neither a successful Boolean test nor inherited predicate knowledge can mint it." | Refinement and tests | Successful runtime tests or inherited predicates must not mint routed provenance | at the operation | 8
DOMAINS172 | "Runtime testing refines proof knowledge." | Refinement and tests | Runtime domain tests must refine but not replace proof knowledge | at the operation | 8
DOMAINS173 | "It changes neither the tested value's declared type nor operator selection; a cast adds the domain." | Refinement and tests | Runtime tests must not change declared type or operator selection; adding a domain requires a cast | at the operation | 5
DOMAINS174 | "Do not generate a domain merely to replace an anonymous interval." | Carrier-exact membership | Compiler or checking must not synthesize a domain only to encode an anonymous interval | not stated | 5
DOMAINS175 | "Locals receive facts from their initializer, guards, calls, and checked qualification." | Carrier-exact membership | Locals must receive domain and predicate facts from initializer, guards, calls, and checked qualification | not stated | 0
DOMAINS176 | "Data `requires` couplings, including case-conditional predicates, remain ordinary [default-domain obligations](dependent_values.md#default-domains-and-zero-initialization)." | Carrier-exact membership | Data requires couplings must be enforced as ordinary default-domain obligations | not stated | 2
DOMAINS177 | "Each selected arm imports the domain facts, not a changed declared type for the matched value, and each transition proves its destination's requirements." | Refinement and tests | Match arms must import domain facts without changing declared type; transitions must prove destination requirements | on every edge | 1

## Ambiguous sentences

Sentence | Reading A | Reading B
--- | --- | ---
"Explicit establishment, reader and writer lists are independent ceilings: omission leaves that axis unrestricted by this policy, an empty list permits nobody, and a nonempty list contains alternative exact callable identities." | Omission on an axis means no access-policy restriction on that axis for the domain. | Omission means the axis is unrestricted only for this access policy, while backing/loan rules may still forbid operations.
"A declaration with neither predicates nor establishment routes adds no predicate or issuer obligation: a valid bare `i32` may be qualified with `as +Km` without an owner grant." | `as +Km` is always permitted for any valid bare i32 when the domain has no predicates or routes. | The sentence describes the Km-style predicate-free route-free case; whether other predicate-free domains share the same cast rules without further text is not stated.
"Importing enables selection, not membership, minting, or implicit operator changes." | Import never enables membership or minting in the importing module. | Import might still enable syntax that participates in membership elsewhere; only selection is enabled in the importer according to this sentence.
"If the parent selects none, a child may add one." | Any child domain may add a dropper when the parent has none. | Only children in the normalized inheritance hierarchy may add one; which child relationships qualify is defined elsewhere.
"No general satisfiability decision or witness search is required." | The compiler must never run satisfiability or witness search for uninhabited domains. | The compiler need not implement general satisfiability; optional warnings or explanations may still use partial checks.
"An executable domain test requires pure, finite, runtime-checkable predicates." | Every part of the domain predicate must be runtime-checkable for an executable test. | Only the portion needed for the test must be runtime-checkable; quantifiers and opaque parts are excluded per the next sentences.
"Removing a predicate or unit domain remains available when it does not evade an access or custody obligation." | Any predicate or unit domain may be removed when access and custody still hold. | Removal remains available only when it does not evade access or custody; whether the compiler must allow all such removals is not fully spelled out in this sentence alone.
"Predicate-free does not mean uninhabited." | Predicate-free domains must be treated as inhabited. | Predicate-free domains may still be uninhabited for other reasons; the sentence only negates equivalence with uninhabited.

## Terms used without definition in this chapter

- domain
- qualification
- carrier
- static theory
- predicate requirements
- semantic contributions
- establishment routes
- access routes
- cleanup selection
- transparent aliases
- multiplicity
- content
- carry
- provider evidence
- variable-fill owner
- FixedVec
- establishment list
- access policy
- AccessPolicy
- AccessPlan
- reflect::TypeSchema
- reflect::callable
- reflect::field
- default domain
- routed domain
- predicate-only membership
- provenance
- minting
- issuer restriction
- reader/writer obligations
- carrier-qualified domain
- declaring-owner-first address
- graph preparation
- injected library
- foreign qualification
- governed subject
- establishment ceiling
- reader list
- writer list
- semantic schema
- observation demands
- backing
- custody
- delegated access
- qualified loan cohort
- loan restoration
- dropped by
- cleanup machine
- normalized theory
- alias constituents
- domain cast
- domain-only cast
- single-parent refinement
- domain union
- executable domain test
- runtime-checkable predicates
- uninhabited domain
- invariant window
- consumption points
- authority establishment
- receipts
- admitted routes
- checked routes
- limited visibility exception
- linear-carrier-vacuous-qualification
- scalar-range-contract-spelling
- AlignmentSize
- ReservationAccess
- TypeSchema::root
- External/Atomic projections
- operation accessors
- displaced content
- non-owning qualification
- semantic interface identity
- cleanup debt
- carrier-generic domain family
- expected-carrier landing
- denotational value terms
- validator
- Boolean-returning validator
- trait requirement route
- exact-machine route
- boundary routes
- requirement routes
- package boundaries
- foreign carrier
- sibling-domain scan
- carrier-wide custody marker
- rebuild(ing) equal carrier fields
- subject-bound facts
- whole-value copy
- byte movement
- mutex
- payload-custody protocol
- child domain
- parent domain
- mutually exclusive alternatives
- wildcard
- hidden runtime domain tag
- authorized provenance

CHAPTER COMPLETE
