# Semantic reflection and typed visitation

Source: `wiki/spec/language/reflection.md` lines 1–425 (read).

## Rules

| rule id | the exact sentence, quoted | the heading it is under | what must be checked, in plain words | when it must hold | lane |
| --- | --- | --- | --- | --- | --- |
| REFLECTION1 | "No format-specific compiler operation, expansion keyword, AST quotation, mandatory object header, or implicit runtime registry follows." | Semantic reflection and typed visitation | Reflection must not add format-specific compiler operations, expansion keywords, AST quotation, mandatory object headers, or implicit runtime registries | not stated | SHARED |
| REFLECTION2 | "handles index only that owned graph." | Semantic descriptions | Schema graph handles must index only the owned description graph | at the operation | 1 |
| REFLECTION3 | "`schema.fields()` and `schema.cases()` borrow description records from the schema value, never compiler/evaluator storage." | Semantic descriptions | Field and case borrows must come from the schema value only, not compiler or evaluator storage | at the operation | 2 |
| REFLECTION4 | "Recursive references are graph edges with their substitutions, not an instruction to recursively expand every possible instance." | Semantic descriptions | Recursive type references must be treated as graph edges with substitutions, not as mandatory full recursive expansion | not stated | 7 |
| REFLECTION5 | "A field description retains its exact declaring owner/application/member, complete declared type, relevance, case ownership, name, and optional authored stable number." | Semantic descriptions | Produced field descriptions must retain exact owner, application, member, complete type, relevance, case ownership, name, and optional stable number | not stated | 1 |
| REFLECTION6 | "Complete type includes domain atoms, arithmetic policy, routed requirements, indices, reference access, and lifetime relationships." | Semantic descriptions | Complete declared type in descriptions must include listed semantic components | not stated | SHARED |
| REFLECTION7 | "Transparent aliases use ordinary normalization; physical shape does not determine type identity." | Semantic descriptions | Type identity in descriptions must use ordinary alias normalization, not physical shape | not stated | 1 |
| REFLECTION8 | "Retired numbers remain descriptive history rather than live fields or cases." | Semantic descriptions | Retired schema numbers must not be treated as live fields or cases | not stated | 1 |
| REFLECTION9 | "Source order and stable-number order remain distinct." | Semantic descriptions | Ordering by source and by stable number must not be conflated | not stated | 1 |
| REFLECTION10 | "The schema describes qualification requirements, not their establishment on an instance." | Semantic descriptions | Schema content must describe qualification requirements without establishing them on instances | not stated | 8 |
| REFLECTION11 | "Describing `Granted` supplies no grant, and reflecting an erased proof field supplies no proof." | Semantic descriptions | Schema or reflection of grants or erased proof fields must not supply grants or proofs | not stated | 8 |
| REFLECTION12 | "Descriptions have no language-level 32-field or 4096-byte cutoff." | Semantic descriptions | Language-level reflection must not impose 32-field or 4096-byte schema cutoffs | not stated | SHARED |
| REFLECTION13 | "inability to produce a complete description reports exhaustion or unsupported production, not a truncated schema." | Semantic descriptions | Incomplete description production must report exhaustion or unsupported production, not emit truncated schema | not stated | SHARED |
| REFLECTION14 | "Fixed capacities in an implementation of `core::layout::Schema` do not define the semantic reflection contract." | Semantic descriptions | Implementation capacity limits in layout Schema must not redefine the semantic reflection contract | not stated | 9 |
| REFLECTION15 | "`reflect::type_key<T>()` provides a comparable descriptive key for a fully static type application under ordinary canonical type equality." | Semantic descriptions | TypeKey for fully static applications must be comparable under canonical type equality | not stated | 7 |
| REFLECTION16 | "It is not a source type, address, cast, or conformance." | Semantic descriptions | TypeKey must not be used or produced as source type, address, cast, or conformance | at the operation | 7 |
| REFLECTION17 | "Runtime-dependent applications instead retain a schema recipe with explicit subject/parameter references." | Semantic descriptions | Runtime-dependent type applications must retain a schema recipe with explicit subject and parameter references | not stated | 7 |
| REFLECTION18 | "No guessed Boolean type equality follows from matching declaration names or symbolic descriptors." | Semantic descriptions | Type equality must not be inferred from matching declaration names or symbolic descriptors alone | not stated | 7 |
| REFLECTION19 | "Numeric descriptions, when exposed, retain the exact semantic carrier kind, integer width/signedness or IEEE format identity, not a width guessed from storage or a declaration's display name." | Semantic descriptions | Numeric metadata must retain exact carrier kind and width or format identity, not guesses from storage or names | not stated | 5 |
| REFLECTION20 | "Their names acquire no compiler-owned meaning." | Semantic descriptions | Core integer and float API names referenced from numeric descriptions must not acquire compiler-owned meaning | not stated | 5 |
| REFLECTION21 | "Knowing a carrier's extrema neither solves an arbitrary domain predicate nor establishes a routed qualification." | Semantic descriptions | Carrier extrema in descriptions must not establish domain predicates or routed qualifications | not stated | 8 |
| REFLECTION22 | "This adds no integer-width constructor or automatic primitive-member namespace." | Semantic descriptions | Reflection must not add integer-width constructors or automatic primitive-member namespaces | not stated | 5 |
| REFLECTION23 | "Graph-local indices are relocated/reconstructed within their graph, not compared as cross-artifact identities." | Semantic descriptions | Graph-local indices must not be used as cross-artifact identities | not stated | 1 |
| REFLECTION24 | "Replay checks the bound declaration/schema revision and every referenced application." | Semantic descriptions | Replay must check bound declaration or schema revision and every referenced application | not stated | 9 |
| REFLECTION25 | "a digest, offset, numeric key, or matching bytes alone authorizes no operation." | Semantic descriptions | Digests, offsets, numeric keys, or matching bytes alone must not authorize operations | at the operation | 8 |
| REFLECTION26 | "Display names and schema numbers are not alternative type identities." | Semantic descriptions | Display names and schema numbers must not be treated as alternative type identities | not stated | 1 |
| REFLECTION27 | "These compiler-known core queries add no keyword and follow ordinary lexical resolution, dependency and visibility rules." | Typed declaration selectors | `reflect::field` and `reflect::callable` must follow ordinary lexical resolution, dependency, and visibility without new keywords | at a call | 9 |
| REFLECTION28 | "They do not evaluate an instance, take a borrow, invoke a machine or acquire authority." | Typed declaration selectors | Field and callable selectors must not evaluate instances, borrow, invoke machines, or acquire authority | at a call | 3 |
| REFLECTION29 | "A symbolic declaration operand is not an otherwise first-class executable value." | Typed declaration selectors | Symbolic declaration operands used in selectors must not be treated as first-class executable values | not stated | 1 |
| REFLECTION30 | "`FieldInfo` retains exact owner/application/member, complete field type, relevance, case ownership and schema revision, as does enumeration of that same member." | Typed declaration selectors | FieldInfo and per-member enumeration must retain listed exact identities and metadata | not stated | 1 |
| REFLECTION31 | "It is neither an address nor an offset." | Typed declaration selectors | FieldInfo must not be treated as address or offset | not stated | 1 |
| REFLECTION32 | "Wrong-owner/application/revision or non-field selectors reject;" | Typed declaration selectors | Wrong owner, application, revision, or non-field selectors must reject | at a call | 1 |
| REFLECTION33 | "physical placement still needs the checked layout correspondence below." | Typed declaration selectors | Physical placement must use checked layout correspondence, not FieldInfo alone | not stated | 9 |
| REFLECTION34 | "A nested region retains its complete checked semantic path, not just the terminal field declaration shared by several containing instances." | Typed declaration selectors | Nested region descriptions must retain complete checked semantic path | not stated | 1 |
| REFLECTION35 | "It is not a function pointer, callable value, selected provider or conformance witness." | Typed declaration selectors | CallableInfo must not be treated as function pointer, callable value, provider, or conformance witness | not stated | 3 |
| REFLECTION36 | "A requirement descriptor authorizes only invocation through that requirement and its selected implementation when used in a domain policy." | Typed declaration selectors | Trait requirement CallableInfo may authorize invocation only through that requirement and selected implementation in domain policy | at a call | 8 |
| REFLECTION37 | "It does not authorize unrelated direct calls to a satisfying machine." | Typed declaration selectors | Requirement descriptors must not authorize direct calls to satisfying machines outside the requirement path | at a call | 3 |
| REFLECTION38 | "A bare trait does not mean all of its current or future requirements." | Typed declaration selectors | Bare trait reflection must not imply all current or future trait requirements | not stated | 7 |
| REFLECTION39 | "Mixed arrays of these callable descriptors are homogeneous." | Typed declaration selectors | Arrays mixing callable descriptors must be homogeneous in descriptor kind | not stated | 1 |
| REFLECTION40 | "`FieldInfo` cannot enter such an array or a caller-list parameter." | Typed declaration selectors | FieldInfo must reject entry into callable-descriptor arrays or caller-list parameters | not stated | 1 |
| REFLECTION41 | "No universal untyped symbol handle or string comparison stands in for either category." | Typed declaration selectors | Untyped symbol handles or string comparison must not substitute for FieldInfo or CallableInfo | not stated | 1 |
| REFLECTION42 | "An ambiguous overload rejects;" | Typed declaration selectors | Ambiguous overloads in selectors must reject | at a call | 3 |
| REFLECTION43 | "a caller-list context does not choose one by guessed signature." | Typed declaration selectors | Caller-list contexts must not resolve ambiguous overloads by guessed signature | at a call | 3 |
| REFLECTION44 | "Generic selectors retain exact applications/substitutions under ordinary generic identity rules." | Typed declaration selectors | Generic selectors must retain exact applications and substitutions per generic identity rules | not stated | 7 |
| REFLECTION45 | "Unresolved applications cannot be relabeled as concrete identities." | Typed declaration selectors | Unresolved generic applications must not be relabeled as concrete identities | not stated | 7 |
| REFLECTION46 | "Constructing, copying or serializing descriptions supplies no authorization." | Typed declaration selectors | Constructing, copying, or serializing descriptions must not grant authorization | not stated | 8 |
| REFLECTION47 | "Consumers validate their exact identities, scope and correspondence." | Typed declaration selectors | Consumers of descriptions must validate exact identities, scope, and correspondence | not stated | 8 |
| REFLECTION48 | "reflection does not publish their declarations as callable APIs." | Typed declaration selectors | Private issuer, reader, and writer descriptor declarations must not be published as callable APIs except the existing domain route-metadata exception | not stated | 8 |
| REFLECTION49 | "A checked correspondence joins an exact semantic member to the applicable physical field key and validated plan; it rejects wrong owner, application, revision, relevance, or case mappings." | Schema and layout | Semantic-to-layout correspondence must join exact members to physical field keys and validated plans and reject wrong mappings | not stated | 9 |
| REFLECTION50 | "There is no second offset calculator or interpretation of a semantic member index as its byte offset." | Schema and layout | Compiler must not provide a second offset calculator or equate semantic member index to byte offset | not stated | 9 |
| REFLECTION51 | "Semantic data remains layout-neutral." | Schema and layout | TypeSchema semantic data must remain layout-neutral | not stated | 9 |
| REFLECTION52 | "a transparent alias changes neither semantic identity nor the selected representation." | Schema and layout | Transparent aliases must not change semantic identity or selected representation | not stated | 1 |
| REFLECTION53 | "Geometry may be queried only after the selected [layout plan](../layouts/plans.md) is validated." | Schema and layout | Layout geometry queries must occur only after the selected layout plan is validated | at the operation | 9 |
| REFLECTION54 | "Core `size_of<T>()` and `align_of<T>()` consume that same plan, not a second interpretation of semantic schema fields." | Schema and layout | `size_of` and `align_of` must use the same validated layout plan, not a separate schema interpretation | at the operation | 9 |
| REFLECTION55 | "A layout cannot inspect its own unfinished result; dependency cycles reject." | Schema and layout | Layout computation must reject dependency cycles and must not inspect unfinished layout results | not stated | 9 |
| REFLECTION56 | "Erased fields have no physical field key or access decision." | Schema and layout | Erased fields must have no physical field key or layout access decision | not stated | 9 |
| REFLECTION57 | "Target-independent description does not silently acquire target geometry dependencies; consumers that use geometry retain them explicitly." | Schema and layout | Target-independent descriptions must not silently gain target geometry dependencies | not stated | 9 |
| REFLECTION58 | "It does not grant generic library code the caller's access context." | Access owner and delegation | Reflection elaboration must not grant generic library code the caller's access context | not stated | 8 |
| REFLECTION59 | "Carrying a foreign T does not authorize selecting its fields, even when its owner publishes structural data." | Access owner and delegation | Carrying a foreign type must not authorize field selection on that type | at the operation | 8 |
| REFLECTION60 | "The receiving library invokes that operation; it does not obtain permission to enumerate T or recursively inspect the types of supplied fields." | Access owner and delegation | Libraries receiving delegated visitor or projection operations must not gain permission to enumerate T or inspect supplied field types recursively | at a call | 8 |
| REFLECTION61 | "Recursive descent uses each field type's explicitly selected operation." | Access owner and delegation | Recursive reflection descent must use explicitly selected per-field-type operations | not stated | 3 |
| REFLECTION62 | "A metadata key or editable flag is not a delegated accessor." | Access owner and delegation | Metadata keys and editable flags must not serve as delegated accessors | not stated | 8 |
| REFLECTION63 | "Enumerating field descriptions grants neither domain reader nor writer authorization over a value." | Access owner and delegation | Field description enumeration must not grant domain reader or writer authorization | not stated | 8 |
| REFLECTION64 | "Runtime visitors and generated codecs must receive the same authorized, subject-bound access as handwritten operations;" | Access owner and delegation | Runtime visitors and generated codecs must require the same authorized subject-bound access as handwritten code | at the operation | 8 |
| REFLECTION65 | "compile-time execution and reflective projection cannot bypass the policy." | Access owner and delegation | Compile-time execution and reflective projection must not bypass domain access policy | not stated | 8 |
| REFLECTION66 | "An initial schema query requires authorized complete structural visibility." | Access owner and delegation | Initial schema queries must require authorized complete structural visibility | at a call | 8 |
| REFLECTION67 | "It does not hide inaccessible members while claiming complete coverage." | Access owner and delegation | Schema queries claiming complete coverage must not hide inaccessible members | at a call | 8 |
| REFLECTION68 | "Fields and cases inherit their semantic owner's visibility; there is no new independently private-field syntax." | Access owner and delegation | Field and case visibility must inherit semantic owner visibility without new private-field syntax | not stated | 8 |
| REFLECTION69 | "Private realization identities in a selected conformance do not become consumer-nameable declarations." | Access owner and delegation | Private realization identities in conformances must not become consumer-nameable declarations | not stated | 8 |
| REFLECTION70 | "In particular, a quotient does not expose its representative by reflection." | Access owner and delegation | Quotient types must not expose representatives through reflection | not stated | 8 |
| REFLECTION71 | "Only an observer licensed by its checked [quotient contract](../proofs/quotients.md#formation-and-representation) may do so." | Access owner and delegation | Quotient representative observation must require a licensed observer per quotient contract | at the operation | 8 |
| REFLECTION72 | "Routed authority, private storage, and BindingPrivate operations keep their respective access rules." | Access owner and delegation | Routed authority, private storage, and BindingPrivate must retain their ordinary access rules under reflection | not stated | 8 |
| REFLECTION73 | "Calls use existing exact requirement and generic-family matching; no anonymous expression or generated capture environment is required." | Typed runtime visitation | Runtime field visitation calls must use existing requirement and generic-family matching without mandating anonymous expressions or generated captures | at a call | 7 |
| REFLECTION74 | "The compiler resolves the member, exact owning application, actual complete field type, callback selection, and ordinary typed projection." | Typed runtime visitation | Compiler must resolve member, owning application, complete field type, callback selection, and typed projection | not stated | 3 |
| REFLECTION75 | "It checks every selected application and generates ordinary calls before Terminal Psi." | Typed runtime visitation | Every selected application must be checked and ordinary calls generated before Terminal Psi | not stated | 3 |
| REFLECTION76 | "Per-member specialization is semantics, not an optional optimization." | Typed runtime visitation | Per-member visitation specialization must be mandatory semantics, not optional optimization | not stated | 3 |
| REFLECTION77 | "A runtime loop over homogeneous FieldInfo records cannot turn a record variable or TypeKey into a generic type argument." | Typed runtime visitation | Runtime loops over FieldInfo must not turn a record variable or TypeKey into a generic type argument | at the operation | 7 |
| REFLECTION78 | "Runtime visitation covers all non-erased bindings, including zero-sized linear or authority-bearing fields." | Typed runtime visitation | Runtime visitation must cover all non-erased bindings including ZST linear or authority-bearing fields | at the operation | 2 |
| REFLECTION79 | "Declaration inspection includes erased bindings too, but they receive no runtime borrow." | Typed runtime visitation | Erased bindings may appear in declaration inspection but must receive no runtime borrow in visitation | at the operation | 2 |
| REFLECTION80 | "An empty record makes no field calls." | Typed runtime visitation | Empty records must produce no field visitation calls | at the operation | 3 |
| REFLECTION81 | "The default order is authored declaration order, not physical placement or stable number order." | Typed runtime visitation | Default visitation order must be authored declaration order | at the operation | 1 |
| REFLECTION82 | "An unsupported included field rejects with its path and failed obligation." | Typed runtime visitation | Unsupported included fields must reject reporting path and failed obligation | at the operation | 3 |
| REFLECTION83 | "Each call obtains a valid field subloan and fresh reborrow of the context." | Typed runtime visitation | Each visitation call must obtain valid field subloan and fresh context reborrow | at a call | 2 |
| REFLECTION84 | "Shared inspection neither consumes nor copies the record or its linear fields." | Typed runtime visitation | Shared inspection must not consume or copy the record or linear fields | at the operation | 2 |
| REFLECTION85 | "References are not silently dereferenced into recursive traversal." | Typed runtime visitation | References must not be silently dereferenced for recursive traversal | at the operation | 2 |
| REFLECTION86 | "Retention needs a valid declared relationship; ordinary escape rejection does not require general outlives-bound syntax." | Typed runtime visitation | Context retention must require valid declared relationship; escape rejection uses ordinary rules without general outlives syntax | not stated | 2 |
| REFLECTION87 | "Field type binding preserves all qualifications." | Typed runtime visitation | Visitor field type binding must preserve all qualifications | at a call | 8 |
| REFLECTION88 | "Exact type equality is distinct from argument compatibility." | Typed runtime visitation | Exact type equality and argument compatibility must be checked distinctly | at a call | 7 |
| REFLECTION89 | "the visitor never discards qualifications to find a callback." | Typed runtime visitation | Visitor matching must not discard qualifications to find a callback | at a call | 8 |
| REFLECTION90 | "Every removed domain is named in that cast, whether it is predicate knowledge or semantic/routed meaning, and owned claims require disposition or transfer." | Typed runtime visitation | Domain casts used by adapters must name every removed domain; owned claims require disposition or transfer | at a call | 8 |
| REFLECTION91 | "encoding its bits does not transfer custody or enable reconstruction of its authority." | Typed runtime visitation | Encoding claim bits must not transfer custody or reconstruct authority | at the operation | 2 |
| REFLECTION92 | "It first reports the selected semantic case exactly once, even for a case with no payload." | Sum coverage | Sum visitation must report selected semantic case exactly once | at the operation | 1 |
| REFLECTION93 | "It then visits common runtime fields once and the active payload's runtime fields, each in declaration order." | Sum coverage | Sum visitation must visit common fields once then active payload fields in declaration order | at the operation | 1 |
| REFLECTION94 | "Compile callback applications for every admitted case;" | Sum coverage | Compiler must compile callback applications for every admitted sum case | not stated | 3 |
| REFLECTION95 | "execution projects only the selected payload after testing the actual discriminator." | Sum coverage | Runtime must project only selected payload after testing actual discriminator | at the operation | 1 |
| REFLECTION96 | "Active-case evidence stays valid while payload loans are live." | Sum coverage | Active-case evidence must remain valid while payload loans are live | not stated | 8 |
| REFLECTION97 | "Retired cases are metadata, not executable cases." | Sum coverage | Retired sum cases must be metadata only, not executable cases | not stated | 1 |
| REFLECTION98 | "Stopping follows ordinary return/loan rules, not a nonlocal return from the callback's enclosing author." | Sum coverage | Partial sum visitation stopping must follow ordinary return and loan rules | at normal return | 3 |
| REFLECTION99 | "A full-coverage operation cannot report success after silently excluding fields or cases." | Sum coverage | Full-coverage operations must not report success after silent exclusion of fields or cases | at normal return | 3 |
| REFLECTION100 | "Encoding/inspection policy is an explicitly selected ordinary machine specialized per discovered member." | Explicit operation selection | Encoding or inspection policy must be an explicitly selected machine specialized per discovered member | not stated | 3 |
| REFLECTION101 | "FieldInfo describes the exact member; the compiler supplies Value statically." | Explicit operation selection | Policy must receive exact member from FieldInfo with Value supplied statically by compiler | not stated | 7 |
| REFLECTION102 | "EncodingSelection is a library-specific instance of a scoped typed selection context passed as an explicit parameter, not a runtime conformance lookup object." | Explicit operation selection | Selection context must be explicit parameter, not runtime conformance lookup | not stated | 3 |
| REFLECTION103 | "An encoder data type alone is not a request to discover a map." | Explicit operation selection | Encoder data types must not trigger automatic map discovery | not stated | 3 |
| REFLECTION104 | "All generic conformance arguments retain their ordinary explicit-selection rules." | Explicit operation selection | Generic conformance arguments in selection must follow ordinary explicit-selection rules | not stated | 7 |
| REFLECTION105 | "The selection context pins the member, qualified field type, and required operation." | Explicit operation selection | Selection context must pin member, qualified field type, and required operation | not stated | 3 |
| REFLECTION106 | "Each included field has exactly one valid selection on every successful policy completion." | Explicit operation selection | Each included field must have exactly one valid selection on successful policy completion | at normal return | 3 |
| REFLECTION107 | "Missing and multiple selections reject; selecting twice is not last-write-wins." | Explicit operation selection | Missing, multiple, or duplicate selections must reject; no last-write-wins | not stated | 3 |
| REFLECTION108 | "A skipped callback due to failure is not a successful omission." | Explicit operation selection | Callback skipped due to failure must not count as successful omission | not stated | 3 |
| REFLECTION109 | "a mutable context parameter alone does not establish them." | Explicit operation selection | Mutable selection context parameter alone must not establish selection obligations | not stated | 2 |
| REFLECTION110 | "The finalized selection set is independently checked for coverage and full requirement refinement, not merely a chosen operation's name/signature." | Explicit operation selection | Finalized selection set must be checked independently for coverage and requirement refinement | not stated | 3 |
| REFLECTION111 | "A table helper rejects duplicate member overrides and defines its type-rule ordering or disjointness contract explicitly." | Explicit operation selection | Table helpers must reject duplicate member overrides and define type-rule ordering or disjointness explicitly | not stated | 3 |
| REFLECTION112 | "There is no ambient default, most-specific search, import-order precedence, or arbitrary tie-break." | Explicit operation selection | Selection must not use ambient defaults, most-specific search, import-order precedence, or arbitrary tie-breaks | not stated | 3 |
| REFLECTION113 | "subsequent selection retains the resolved exact key, not the string as identity." | Explicit operation selection | Member selection must retain resolved exact key, not string identity | not stated | 1 |
| REFLECTION114 | "Forged or stale key/type pairs reject before projection." | Explicit operation selection | Forged or stale member key and type pairs must reject before projection | at the operation | 1 |
| REFLECTION115 | "The policy sees descriptions and its static policy environment, not a future runtime object's contents." | Selection evaluation and retained results | Compile-time selection policy must not observe future runtime object contents | not stated | 3 |
| REFLECTION116 | "It selects an encoder but does not execute runtime output or device access." | Selection evaluation and retained results | Selection evaluation must not execute runtime output or device access | not stated | 3 |
| REFLECTION117 | "It cannot mutate global compiler state, acquire Build authority, append provider-plan rows, or bypass visibility and conformance checking." | Selection evaluation and retained results | Typed selection operation must not mutate global compiler state, acquire Build authority, append provider plans, or bypass visibility or conformance | at the operation | 9 |
| REFLECTION118 | "The evaluation root owns the selection context, lends it explicitly to the policy, and freezes its completed choices into an ordinary owned symbolic result snapshot." | Selection evaluation and retained results | Evaluation root must own selection context, lend explicitly, and freeze completed choices into owned snapshot | not stated | 3 |
| REFLECTION119 | "Ordinary hermetic evaluation returns the snapshot; no mutable argument, evaluator reference, host address, or out-of-band compiler mutation escapes." | Selection evaluation and retained results | Hermetic selection evaluation must return snapshot without escaping mutable arguments, evaluator references, host addresses, or out-of-band mutation | at normal return | 3 |
| REFLECTION120 | "Validation resolves their exact declarations and rechecks member/type/requirement correspondence and authorized scope before generating calls." | Selection evaluation and retained results | Snapshot validation must resolve exact declarations and recheck correspondence and scope before call generation | not stated | 3 |
| REFLECTION121 | "Snapshot manufacture does not grant selection authority." | Selection evaluation and retained results | Manufacturing selection snapshots must not grant selection authority | not stated | 8 |
| REFLECTION122 | "Live runtime context values cannot influence compile-time selection." | Selection evaluation and retained results | Live runtime context values must not influence compile-time selection | not stated | 3 |
| REFLECTION123 | "No general facility for returning runtime machine symbols or treating conformance evidence as ordinary generic value atoms is introduced by reflection." | Selection evaluation and retained results | Reflection must not introduce returning runtime machine symbols or conformance evidence as generic value atoms | not stated | 3 |
| REFLECTION124 | "metadata alone is not an executable descriptor." | Selection evaluation and retained results | Metadata alone must not be treated as executable machine descriptor | not stated | 3 |
| REFLECTION125 | "Every generated call retains ordinary preconditions, post-state, effects, suspension/blocking acknowledgements, failures, crashes, and custody." | Contracts, recursion, and resources | Generated reflection calls must retain ordinary contract, effect, and custody obligations | at a call | 3 |
| REFLECTION126 | "A finite field set does not prove callback termination." | Contracts, recursion, and resources | Finite field sets must not be treated as proving callback termination | not stated | 4 |
| REFLECTION127 | "reflection adds no effect-forwarding syntax." | Contracts, recursion, and resources | Reflection must not add effect-forwarding syntax | not stated | 3 |
| REFLECTION128 | "Empty specialized reach does not discharge the other hermetic evaluation obligations." | Contracts, recursion, and resources | Empty specialized reach must not discharge other hermetic evaluation obligations | not stated | 3 |
| REFLECTION129 | "Every obligation in the resulting dependency component must still be discharged; a pending entry is not an assumed proof." | Contracts, recursion, and resources | Pending recursive derivation entries must not discharge obligations or count as assumed proof | not stated | 8 |
| REFLECTION130 | "Ever-growing distinct specializations do not become a finite graph by dropping their arguments." | Contracts, recursion, and resources | Distinct specializations must not be folded into a finite graph by dropping arguments | not stated | 7 |
| REFLECTION131 | "Resource exhaustion or unsupported production reports failure rather than emitting partial coverage." | Contracts, recursion, and resources | Resource exhaustion or unsupported production must fail rather than emit partial coverage | not stated | SHARED |
| REFLECTION132 | "Runtime recursion follows ordinary [tail-position rules](termination.md#calls-and-productive-loops)." | Contracts, recursion, and resources | Runtime reflection recursion must follow ordinary tail-position rules | at a call | 4 |
| REFLECTION133 | "No hidden recursive stack is generated." | Contracts, recursion, and resources | Compiler must not generate hidden recursive stacks for reflection recursion | not stated | 3 |
| REFLECTION134 | "a recursive schema alone does not establish that policy." | Contracts, recursion, and resources | Recursive schema must not by itself establish cyclic or shared object traversal policy | not stated | 3 |
| REFLECTION135 | "Opening one value does not reveal a concrete witness to generic reflection or create a downcast." | Contracts, recursion, and resources | Opening a `some` value must not reveal generic reflection witness or create downcast | at the operation | 8 |
| REFLECTION136 | "Dependent projections obey the same stable-subject and scope rules as ordinary package access; available operations/evidence must justify inspection of the abstract referent." | Contracts, recursion, and resources | Dependent reflection projections must obey stable-subject and scope rules with justified inspection | at the operation | 8 |
| REFLECTION137 | "An explicit `reflect::metadata<T>()` request materializes owned copy-eligible descriptive data under ordinary constant rules." | Runtime descriptions and access views | `reflect::metadata` must materialize owned copy-eligible data under ordinary constant rules | at the operation | 1 |
| REFLECTION138 | "Runtime lookup selects among already retained checked operations, not arbitrary addresses or new type arguments." | Runtime descriptions and access views | Runtime reflection lookup must select among retained checked operations only | at the operation | 3 |
| REFLECTION139 | "Availability of one does not imply all." | Runtime descriptions and access views | Availability of one runtime reflection interface must not imply availability of all | not stated | 3 |
| REFLECTION140 | "there is no accidental static-lifetime requirement on all inspectable values." | Runtime descriptions and access views | Inspectable value arguments must not acquire accidental static-lifetime requirements | not stated | 2 |
| REFLECTION141 | "Packed/fragmented fields without an addressable reference reject the borrowing route unless an explicitly selected copied-read operation supplies the required value and contract." | Runtime descriptions and access views | Packed or fragmented fields without addressable reference must reject borrowing unless explicit copied-read operation supplies value and contract | at the operation | 2 |
| REFLECTION142 | "Do not invent a temporary referent or change backing/lifetime identity." | Runtime descriptions and access views | Reflection must not invent temporary referents or change backing or lifetime identity for packed access | at the operation | 2 |
| REFLECTION143 | "Schema enumeration does not read devices." | Runtime descriptions and access views | Schema enumeration must not perform device reads | at the operation | 9 |
| REFLECTION144 | "A general inspector cannot silently issue Take, atomic loads, or MMIO transfers." | Runtime descriptions and access views | General inspectors must not silently issue Take, atomic loads, or MMIO transfers | at the operation | 10 |
| REFLECTION145 | "names or metadata cannot manufacture that access." | Runtime descriptions and access views | Names or metadata must not manufacture device access authority | not stated | 8 |
| REFLECTION146 | "Recasts do not bypass it." | Runtime descriptions and access views | Recasts must not bypass authorized access-view restrictions | at the operation | 8 |
| REFLECTION147 | "Decoded documents are not instances of the described type." | Construction and editing | Decoded reflection documents must not be treated as typed instances without construction | not stated | 1 |
| REFLECTION148 | "Encoding selection does not prove decoding is an inverse or that a grant can be reconstructed from bytes." | Construction and editing | Encoding selection must not prove decoding inverse or grant reconstruction from bytes | not stated | 8 |
| REFLECTION149 | "Mutable reflection is not an automatic counterpart of borrowing inspection." | Construction and editing | Mutable reflection must not be automatic counterpart to borrowing inspection | not stated | 2 |
| REFLECTION150 | "Metadata processing executes no getter, constructor, setter, or external operation merely to describe it." | Construction and editing | Metadata processing must not execute getters, constructors, setters, or external operations solely for description | not stated | 3 |
| REFLECTION151 | "Logical collections expose selected element operations rather than implicitly traversing allocator bookkeeping." | Construction and editing | Logical collection reflection must expose selected element operations only, not implicit allocator traversal | not stated | 6 |
| REFLECTION152 | "[Embedding](../build/embedding.md#reflection-and-debugging) exposes selected descriptions and checked callable operations, not a second mutation mechanism." | Embedded inspection and edits | Embedding reflection must expose selected descriptions and checked callables without a second mutation mechanism | not stated | 9 |
| REFLECTION153 | "An invariant-preserving setter still requires valid access at invocation: an edit can invalidate stronger live flow facts even when the type's domain stays true." | Embedded inspection and edits | Invariant-preserving setters invoked via reflection still require valid access at invocation | at a call | 8 |
| REFLECTION154 | "A work-budget pause retains the activation's loans, invariant windows, and restoration debts; it does not permit an editor to borrow or write the affected storage." | Embedded inspection and edits | Work-budget pause must retain activation obligations and must not permit editor borrow or write of affected storage | across a pause | 2 |
| REFLECTION155 | "Detached candidates can be edited, then submitted through an operation that obtains access and establishes the full contract when applying them." | Embedded inspection and edits | Applying detached edits must go through an operation that obtains access and establishes full contract | at the operation | 2 |
| REFLECTION156 | "Code bodies are not mutable reflection values." | Embedded inspection and edits | Code bodies must not be mutable reflection values | not stated | 1 |
| REFLECTION157 | "Authorized component replacement does not transplant old continuations into the edited code." | Embedded inspection and edits | Component replacement must not transplant old continuations into edited code | not stated | 10 |
| REFLECTION158 | "Debug snapshots may describe unestablished storage but cannot return it as an established typed value or callable authority." | Embedded inspection and edits | Debug snapshots must not return unestablished storage as established typed values or callable authority | not stated | 8 |
| REFLECTION159 | "Replay joins declaration, member, application, access scope, policy selection, and operation; corrupted descriptions or same-spelled user APIs cannot acquire compiler privileges." | Compiler ownership | Replay must join listed facets; corrupted descriptions or same-spelled user APIs must not acquire compiler privileges | not stated | SHARED |
| REFLECTION160 | "There is no new boundary trait or opaque admission merely because an operation is compiler-known." | Compiler ownership | Compiler-known reflection operations must not create new boundary traits or opaque admission | not stated | 9 |
| REFLECTION161 | "Logical schemas, optional library plans, and runtime tables do not authorize source splicing, new types, arbitrary method invocation, or a second layout system." | Compiler ownership | Schemas, library plans, and runtime tables must not authorize source splicing, new types, arbitrary invocation, or a second layout system | not stated | 9 |
| REFLECTION162 | "`reflect::field(Type::field)` selects a semantic field declaration and returns `FieldInfo`." | Typed declaration selectors | `reflect::field` must select a semantic field declaration and yield FieldInfo for valid operands | at a call | 1 |
| REFLECTION163 | "`reflect::callable(Path::machine)` selects either an exact concrete machine or an exact trait machine requirement and returns `CallableInfo`." | Typed declaration selectors | `reflect::callable` must select exact concrete machine or exact trait requirement and yield CallableInfo | at a call | 3 |
| REFLECTION164 | "A compiler-known reflection occurrence elaborates queries and projections under its lexical author's ordinary dependency and visibility authority for an explicitly selected authorized T." | Access owner and delegation | Reflection occurrences must elaborate under lexical author's dependency and visibility for explicitly selected authorized T | not stated | 8 |
| REFLECTION165 | "Structural visibility does not override [domain-owned access policy](domains.md#domain-owned-read-and-write-authority)." | Access owner and delegation | Structural visibility must not override domain-owned read and write authority | not stated | 8 |
| REFLECTION166 | "Public wrappers retain ordinary public-signature exposure rules." | Access owner and delegation | Public wrappers in reflection must follow ordinary public-signature exposure rules | not stated | 8 |
| REFLECTION167 | "Semantic opacity remains stronger than equal layout." | Access owner and delegation | Semantic opacity must prevail over equal layout for reflection access | not stated | 8 |
| REFLECTION168 | "`reflect::visit_runtime_fields<T, Visitor>` specializes the selected callback for each immediate non-erased field of a record." | Typed runtime visitation | `visit_runtime_fields` must specialize the callback per immediate non-erased record field | at the operation | 3 |
| REFLECTION169 | "Recursive descent and alternate order require explicit library policy." | Typed runtime visitation | Recursive descent and non-default visitation order must come from explicit library policy | not stated | 3 |
| REFLECTION170 | "These invocation loans can end before the next call; loans stored in the explicit context retain their own lifetimes." | Typed runtime visitation | Per-call invocation loans may end before next call; context-stored loans keep their lifetimes | at a call | 2 |
| REFLECTION171 | "A descriptive encoder may borrow an owned claim without consuming it;" | Typed runtime visitation | Descriptive encoders may borrow owned claims without consuming them when allowed | at the operation | 2 |
| REFLECTION172 | "Semantic case identity, authored wire numbers, and runtime discriminator values remain distinct;" | Sum coverage | Semantic case identity, wire numbers, and runtime discriminators must remain distinct | not stated | 1 |
| REFLECTION173 | "Partial consumers may use an explicitly declared ordinary result to stop or omit members." | Sum coverage | Partial sum consumers may stop or omit members via explicitly declared ordinary results | at normal return | 3 |
| REFLECTION174 | "Explicit policy failure need not fabricate a selection." | Explicit operation selection | Explicit policy failure must not require fabricating a selection | not stated | 3 |
| REFLECTION175 | "A partial-coverage contract may instead permit an explicit exclusion, with its coverage retained." | Explicit operation selection | Partial-coverage contracts may permit explicit exclusions while retaining stated coverage obligations | not stated | 3 |
| REFLECTION176 | "These obligations may be expressed with ordinary machine contracts or linear selection obligations," | Explicit operation selection | Selection obligations may use ordinary machine contracts or linear selection obligations | not stated | 2 |
| REFLECTION177 | "Member keys originate from the authorized schema." | Explicit operation selection | Member keys used in selection must originate from authorized schema | not stated | 8 |
| REFLECTION178 | "`select` names an exact conformance application or exact machine appropriate to its declared requirement." | Explicit operation selection | `select` must name exact conformance application or exact machine appropriate to declared requirement | at a call | 7 |
| REFLECTION179 | "The typed selection operation is new compiler-recognized behavior, not provider selection reused unchanged." | Selection evaluation and retained results | Typed selection must be distinct compiler-recognized behavior, not unchanged provider selection | not stated | 9 |
| REFLECTION180 | "The records describe choices, not proof by construction." | Selection evaluation and retained results | Selection snapshot records must describe choices only, not proof by construction | not stated | 8 |
| REFLECTION181 | "Policy code, static inputs, selections, target dependencies actually consulted, and schema revision participate in retained derivation dependencies." | Selection evaluation and retained results | Listed selection inputs must participate in retained derivation dependencies | not stated | 9 |
| REFLECTION182 | "Runtime [machine values](machine_values.md) require ordinary closed selection and callable-contract refinement;" | Selection evaluation and retained results | Runtime machine values require ordinary closed selection and callable-contract refinement | not stated | 3 |
| REFLECTION183 | "Static callback reach follows ordinary [automatic propagation](effects.md#static-callback-reach-dependencies);" | Contracts, recursion, and resources | Static callback reach in reflection must follow ordinary automatic propagation | not stated | 3 |
| REFLECTION184 | "Suspension/blocking stay fixed by the requirement." | Contracts, recursion, and resources | Suspension and blocking behavior in generated calls must follow the requirement | at a call | 3 |
| REFLECTION185 | "Recursive derivation is keyed by the exact type/application, selected operation contract and realization, and static policy dependencies." | Contracts, recursion, and resources | Recursive derivation identity must include exact type or application, operation contract and realization, and static policy dependencies | not stated | 7 |
| REFLECTION186 | "Re-entry to the same derivation refers to its pending identity rather than allocating another encoder." | Contracts, recursion, and resources | Re-entry to same derivation must refer to pending identity, not allocate duplicate encoder | not stated | 3 |
| REFLECTION187 | "Nested serialization that returns to process siblings uses explicitly provisioned pending-work storage or a valid tail-recursive formulation." | Contracts, recursion, and resources | Nested serialization returning to siblings must use explicit pending-work storage or valid tail recursion | at a call | 4 |
| REFLECTION188 | "A `some` declaration exposes its abstract binder and ordinary field schema." | Contracts, recursion, and resources | `some` declarations must expose abstract binder and ordinary field schema for reflection | not stated | 8 |
| REFLECTION189 | "Direct visitation does not retain a complete runtime table; names actually used by generated code still cost data." | Runtime descriptions and access views | Direct visitation must not retain complete runtime tables; generated-code names still incur data cost | not stated | 3 |
| REFLECTION190 | "Runtime machine values retain their selected code independently from explicit state arguments; `some` packages preserve checked relations between those fields." | Runtime descriptions and access views | Runtime machine values must retain selected code separately from state arguments with preserved `some` relations | not stated | 10 |
| REFLECTION191 | "Recursive adapter retention follows exact dependencies, not a global inventory of every known type." | Runtime descriptions and access views | Recursive adapter retention must follow exact dependencies only | not stated | 3 |
| REFLECTION192 | "Ordinary RAM borrows remain ordinary field projections." | Runtime descriptions and access views | RAM borrows in reflection must remain ordinary field projections | at the operation | 2 |
| REFLECTION193 | "[Access-view inspection](../resources/access.md) uses the same semantic member identities but invokes its exact authorized access operations." | Runtime descriptions and access views | Access-view inspection must use same member identities and invoke exact authorized access operations | at the operation | 8 |
| REFLECTION194 | "Explicitly selected device operations retain ordering, effects, supply, residency, extent, and loan evidence." | Runtime descriptions and access views | Device operations selected for reflection must retain ordering, effects, supply, residency, extent, and loan evidence | at the operation | 10 |
| REFLECTION195 | "BindingPrivate restrictions do not prevent delegation through an already authorized public accessor contract," | Runtime descriptions and access views | BindingPrivate must not block delegation through already authorized public accessor contracts | not stated | 8 |
| REFLECTION196 | "Owner-authorized construction establishes predicates, erased terms, routed evidence, and resources." | Construction and editing | Establishing instances from decoded candidates requires owner-authorized construction with listed obligations | at the operation | 2 |
| REFLECTION197 | "Owners supply checked setters or whole-object update operations with declared failure/partial-update and invariant behavior." | Construction and editing | Mutable reflection must use owner-supplied checked setters or updates with declared failure and invariant behavior | at the operation | 2 |
| REFLECTION198 | "UI clamps and editable flags are policy, not proof." | Construction and editing | UI clamps and editable flags must not count as proof obligations | not stated | 8 |
| REFLECTION199 | "Source edits go through ordinary compilation; another Psi-producing transformation must also submit a new checked artifact." | Embedded inspection and edits | Source edits must use ordinary compilation; Psi-producing transformations must submit new checked artifacts | not stated | 9 |
| REFLECTION200 | "Psi owns authorized schema queries, semantic member correspondence, typed policy selection, and per-member elaboration into ordinary checked data/calls before Terminal Psi." | Compiler ownership | Psi stage must own listed reflection elaboration before Terminal Psi | not stated | SHARED |
| REFLECTION201 | "Selection and execution share callable-family checking but retain their distinct evaluation and evidence obligations." | Compiler ownership | Selection and execution must share callable-family checking while keeping distinct evaluation and evidence obligations | not stated | 3 |
| REFLECTION202 | "`reflect::schema<T>()` produces an owned `reflect::TypeSchema` description graph for the selected type application." | Semantic descriptions | `reflect::schema` must produce owned TypeSchema for the selected type application | at a call | 1 |
| REFLECTION203 | "Actual projected values carry their own facts, loans, and provenance under [domains](domains.md) and [dependent values](dependent_values.md)." | Semantic descriptions | Projected runtime values must carry their own facts, loans, and provenance per domains and dependent values | not stated | 2 |
| REFLECTION204 | "Closed descriptions and retained symbolic references have deterministic encodings: versioned descriptor roles, exact owner/declaration/application correspondence, and exact member identity." | Semantic descriptions | Closed descriptions must use deterministic encodings with versioned roles and exact correspondence | not stated | 1 |
| REFLECTION205 | "Fingerprints index these records;" | Semantic descriptions | Fingerprints may index description records but must not alone authorize operations per REFLECTION25 | not stated | 1 |
| REFLECTION206 | "Private issuer/reader/writer descriptors have only the existing domain route-metadata exception;" | Typed declaration selectors | Private issuer, reader, writer descriptors allow only the existing domain route-metadata exception | not stated | 8 |
| REFLECTION207 | "Fixed-memory families and variable-length encoded carriers retain their different projection and conversion capabilities under the [representation contract](../layouts/plans.md#representation-applications)." | Schema and layout | Fixed-memory and variable-length carriers must retain distinct projection and conversion capabilities per representation contract | not stated | 9 |
| REFLECTION208 | "A selected representation application joins that same exact schema to its policy and physical geometry;" | Schema and layout | Representation applications must join the same exact schema to policy and physical geometry | not stated | 9 |
| REFLECTION209 | "An authorized owner can author a visitor or checked projection operation and pass it through a callable requirement or explicit conformance." | Access owner and delegation | Only authorized owners may author visitors or projections passed through requirements or conformances | not stated | 8 |
| REFLECTION210 | "[Package selection](../packages/boundaries.md) remains separate from carrying a type." | Access owner and delegation | Package selection must remain separate from merely carrying a type for reflection authority | not stated | 9 |
| REFLECTION211 | "Two nullary cases therefore remain distinguishable." | Sum coverage | Sum visitation must keep distinct nullary cases distinguishable | at the operation | 1 |
| REFLECTION212 | "Authored control flow determines member-override precedence and ordering of overlapping type rules." | Explicit operation selection | Member-override precedence and overlapping type-rule order must follow authored control flow | not stated | 3 |
| REFLECTION213 | "The same carrier may use different encodings at different members." | Explicit operation selection | Different members may select different encodings for the same carrier type | not stated | 3 |
| REFLECTION214 | "An ordinary static name lookup may find a member or return failure;" | Explicit operation selection | Static name lookup for members may succeed or fail under ordinary rules | at the operation | 1 |
| REFLECTION215 | "This needs no field-declaration generic binder such as `member<Player::health>()>`." | Explicit operation selection | Reflection must not require field-declaration generic binders like `member<Player::health>()` | not stated | 7 |
| REFLECTION216 | "Display labels and policy flags remain ordinary data." | Explicit operation selection | Display labels and policy flags must remain ordinary data, not authority | not stated | 1 |
| REFLECTION217 | "Selection uses the same staged per-member invocation machinery as encoding, with different arguments and evaluation eligibility." | Selection evaluation and retained results | Selection must use same staged per-member machinery as encoding with appropriate eligibility | not stated | 3 |
| REFLECTION218 | "It records an exact static declaration/application choice inside the evaluation-local selection context." | Selection evaluation and retained results | Typed selection must record exact static declaration or application choice in evaluation-local context | at the operation | 3 |
| REFLECTION219 | "The author's policy need not return first-class conformance evidence." | Selection evaluation and retained results | Policy need not return first-class conformance evidence | not stated | 8 |
| REFLECTION220 | "Libraries own work item/adapter representation, capacity, exhaustion, progress, and reference custody." | Contracts, recursion, and resources | Libraries own work-item representation and related resource contracts for nested serialization | not stated | 2 |
| REFLECTION221 | "Registries and adapter inventories are explicit library/build products with ordinary storage and lifecycle contracts." | Runtime descriptions and access views | Registries and adapter inventories must be explicit library or build products | not stated | 9 |
| REFLECTION222 | "Selected runtime interfaces separately expose descriptions, getters, setters, factories, serialization, or invocation." | Runtime descriptions and access views | Runtime reflection interfaces may expose listed capabilities separately | not stated | 3 |
| REFLECTION223 | "Explicit data arguments retain their ordinary borrowed lifetimes;" | Runtime descriptions and access views | Explicit data arguments in runtime reflection retain ordinary borrowed lifetimes | at a call | 2 |
| REFLECTION224 | "Stable format numbering, defaults, version migration, and reference identity remain codec/library contracts." | Construction and editing | Format numbering, defaults, migration, and reference identity stay codec or library contracts, not compiler proof | not stated | SHARED |
| REFLECTION225 | "Additional owner-level semantic choices belong in [OWNER_QUESTIONS.md](../../../OWNER_QUESTIONS.md), not a surviving proposal." | Compiler ownership | Unspecified owner-level semantic choices must not be implemented as surviving proposals in the language | not stated | SHARED |

## Ambiguous sentences

| Sentence | Reading A | Reading B |
| --- | --- | --- |
| "Reflection describes an explicitly selected type and specializes ordinary checked machines over its members." | The compiler must specialize checked machines over members whenever reflection is used on a selected type. | The sentence describes the reflection facility's purpose only; it does not state when specialization is mandatory versus optional library use. |
| "Libraries own inspection, encoding, editing, ordering, and migration policy." | The compiler must not assign default inspection, encoding, editing, ordering, or migration policy. | Libraries own policy at the library level; the compiler may still enforce minimum safety rules named elsewhere in the chapter. |
| "Typed visitation of dependent values remains possible with their actual subject and equality evidence; it does not require a fully static TypeKey for every field." | Visitation of dependent values must be accepted when subject and equality evidence are present even without per-field static TypeKey. | Visitation may remain possible in principle; this sentence does not state whether compilation must succeed for all such programs. |
| "[Core integer and float APIs](numeric_values.md#core-integer-and-float-apis) may use that authoritative metadata or ordinary exact type-equality refinement to compute library results." | Libraries may choose either metadata or type-equality refinement; the compiler must accept both paths when checks succeed. | "May use" permits library choice only; the compiler could require one path for certain contexts though the sentence does not say so. |
| "Explicit policy failure need not fabricate a selection." | On policy failure the compiler must not require a fabricated selection row. | Policy failure might still require explicit exclusion or error; "need not fabricate" does not state whether compilation continues or rejects. |
| "A partial-coverage contract may instead permit an explicit exclusion, with its coverage retained." | Partial-coverage contracts may record explicit exclusions while obligations for stated coverage remain checkable. | "Coverage retained" might mean documentation of intended coverage only, without compiler verification of the retained obligation. |
| "A skipped callback due to failure is not a successful omission." | Failed callbacks must not count toward successful partial coverage. | The sentence governs coverage accounting only; whether the enclosing policy compilation rejects or continues is not stated here. |
| "Empty specialized reach does not discharge the other hermetic evaluation obligations." | Other hermetic obligations remain even when specialized reach is empty. | "Empty specialized reach" might refer only to effect reach, leaving open whether other obligation classes are also undischarged. |
| "Cyclic/shared object traversal separately chooses whether to follow, identify, or omit references; a recursive schema alone does not establish that policy." | Compiler must not infer traversal policy from schema recursion alone; libraries must choose explicitly. | Schema recursion might still constrain which choices are valid though not select among them; the sentence does not state compiler rejection of implicit choices. |
| "BindingPrivate restrictions do not prevent delegation through an already authorized public accessor contract," | Delegation through an authorized public accessor remains allowed despite BindingPrivate. | BindingPrivate might still restrict which public accessors count as "already authorized"; authorization criteria are not in this sentence. |
| "Debug snapshots may describe unestablished storage but cannot return it as an established typed value or callable authority." | Debug APIs may expose descriptive data about unestablished storage without typed returns or authority. | Snapshots might return opaque handles that are not "established typed values" while still enabling later establishment; boundaries of "return" are not fully stated. |
| "There is no new boundary trait or opaque admission merely because an operation is compiler-known." | Compiler-known reflection operations must not trigger new boundary traits or opaque admission paths. | "Merely because" leaves open whether other reasons could still introduce boundary traits for the same operations. |

## Terms used without definition in this chapter

- reflection / semantic reflection
- typed visitation
- explicitly selected type
- specialize / specializes ordinary checked machines
- `reflect::schema` / `reflect::TypeSchema`
- description graph / nodes / handles
- declaring owner / application / member
- relevance
- case ownership
- stable number / retired numbers
- complete declared type / complete type
- domain atoms
- arithmetic policy
- routed requirements
- transparent aliases
- physical shape
- qualification requirements / establishment
- `Granted` (as described qualifier)
- erased proof field
- projected values / provenance
- storage and evaluation sponsorship
- exhaustion / unsupported production
- truncated schema
- `core::layout::Schema`
- semantic reflection contract
- `reflect::type_key` / TypeKey
- fully static type application
- canonical type equality
- runtime-dependent applications
- schema recipe
- subject/parameter references
- symbolic descriptors
- equality evidence
- semantic carrier kind
- IEEE format identity
- extrema
- routed qualification
- primitive-member namespace
- closed descriptions
- versioned descriptor roles
- graph-local indices
- cross-artifact identities
- fingerprints
- digest (as index key)
- display names / schema numbers
- `reflect::field` / `FieldInfo`
- `reflect::callable` / `CallableInfo`
- symbolic declaration operand
- schema revision
- nested region / checked semantic path
- trait machine requirement
- requirement descriptor
- bare trait
- caller-list parameter / caller-list context
- issuer/reader/writer descriptors
- domain route-metadata exception
- `layout::Schema`
- checked correspondence
- physical field key
- layout-neutral
- representation application
- fixed-memory families
- variable-length encoded carriers
- layout plan
- dependency cycles (layout)
- target geometry dependencies
- compiler-known reflection occurrence
- lexical author
- authorized T
- foreign T
- delegated accessor
- structural visibility
- complete structural visibility
- domain reader / domain writer
- subject-bound access
- public wrappers
- public-signature exposure
- private realization identities
- semantic opacity
- quotient / representative
- licensed observer
- BindingPrivate
- `reflect::visit_runtime_fields`
- Visitor (callback type parameter)
- generic-family matching
- anonymous expression / generated capture environment
- Terminal Psi
- homogeneous FieldInfo records
- zero-sized linear / authority-bearing fields
- unsupported included field
- field subloan
- fresh reborrow
- explicit context
- domain cast
- owned claim / disposition or transfer
- descriptive encoder
- sum visitor / active-case callback
- common runtime fields
- nullary cases
- admitted case
- semantic case identity
- authored wire numbers
- runtime discriminator values
- active-case evidence
- full-coverage operation
- encoding/inspection policy
- type rules / member rules
- EncodingSelection
- `select` (selection operation)
- encoder data type
- selection context
- partial-coverage contract
- explicit exclusion
- linear selection obligations
- table helper
- member keys
- field-declaration generic binder
- forged or stale key/type pairs
- typed selection operation
- evaluation root
- symbolic result snapshot
- hermetic evaluation
- proof by construction
- retained derivation dependencies
- runtime machine symbols
- conformance evidence as generic value atoms
- machine values (runtime)
- static callback reach
- specialized reach
- pending identity / pending entry
- pending-work storage
- cyclic/shared object traversal
- `some` declaration / abstract binder
- concrete witness
- stable-subject rules
- abstract referent
- `reflect::metadata`
- copy-eligible descriptive data
- runtime table
- registries / adapter inventories
- getters / setters / factories (runtime interfaces)
- packed/fragmented fields
- copied-read operation
- temporary referent
- access-view inspection
- Take / MMIO transfers (as operations)
- recasts
- decoded documents / candidates
- owner-authorized construction
- mutable reflection
- editable flags
- logical collections
- allocator bookkeeping
- embedding (reflection-and-debugging)
- work-budget pause
- detached candidates
- continuations (component replacement)
- debug snapshots
- unestablished storage
- Psi / Omega (ownership split)
- semantic member correspondence
- typed policy selection
- per-member elaboration
- callable-family checking
- corrupted descriptions
- logical schemas / optional library plans
- source splicing
- second layout system
- OWNER_QUESTIONS.md (as process bucket)

CHAPTER COMPLETE
