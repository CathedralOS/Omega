# Data, literals, and lexical framing

Source: `wiki/spec/language/data_and_literals.md` lines 1–297 (read).

## Rules

rule id | the exact sentence, quoted | the heading it is under | what must be checked, in plain words | when it must hold | lane
--- | --- | --- | --- | --- | ---
DATA_AND_LITERALS1 | "A record has only fields; a sum has only cases; a mixed declaration has common fields plus one active case and its payload." | Data, literals, and lexical framing | Each `data` declaration must classify as record-only, sum-only, or mixed with exactly one active case and its payload at a time | not stated | 1
DATA_AND_LITERALS2 | "Runtime callable fields store explicit machine values; they do not attach machine declarations or an implicit receiver to data." | Data, literals, and lexical framing | Callable fields must store explicit machine values and must not attach machine declarations or an implicit receiver to the data type | not stated | 10
DATA_AND_LITERALS3 | "Inline case declarations form a consecutive tail after all common fields." | Data, literals, and lexical framing | Inline cases must appear only as a consecutive tail after all common fields | not stated | 1
DATA_AND_LITERALS4 | "A common field after the first inline case is retired." | Data, literals, and lexical framing | A common field declared after the first inline case must be rejected as retired placement | not stated | 1
DATA_AND_LITERALS5 | "This source order does not constrain a selected layout policy's physical placement or add a case-group schema member, number or discriminator." | Data, literals, and lexical framing | Compiler must not treat authored source order as constraining layout physical placement or as adding a case-group schema member, number, or discriminator | not stated | 1
DATA_AND_LITERALS6 | "It introduces no fields, cases, nominal carrier or storage wrapper." | Data, literals, and lexical framing | A transparent alias form `data Name<Parameters> = TypeExpression;` must introduce no fields, cases, nominal carrier, or storage wrapper | not stated | 7
DATA_AND_LITERALS7 | "The spelling is `#N case Name`, not `case #N Name`." | Construction and case identity | Numbered case identity syntax must be `#N case Name`; the reversed form must be rejected | not stated | 1
DATA_AND_LITERALS8 | "there is no shared `case { ... }` declaration block." | Construction and case identity | Shared `case { ... }` declaration blocks must be rejected | not stated | 1
DATA_AND_LITERALS9 | "Cases remain distinct mutually exclusive schema members, not simultaneously present record fields." | Construction and case identity | Cases must be modeled as mutually exclusive schema members, not as simultaneous record fields | not stated | 1
DATA_AND_LITERALS10 | "the payload's `#1` does not identify a common field." | Construction and case identity | Payload numbering scopes must be separate; a payload `#N` must not resolve to a common-field identity | not stated | 1
DATA_AND_LITERALS11 | "There is no extra case-group field, number or discriminator." | Construction and case identity | The schema must not gain an extra case-group field, number, or discriminator from prefix case identities | not stated | 1
DATA_AND_LITERALS112 | "Unnumbered alternatives remain `case Name`." | Construction and case identity | Cases without a `#N` prefix must use the `case Name` form | not stated | 1
DATA_AND_LITERALS113 | "A numbered member writes its identity before its declaration kind." | Construction and case identity | Numbered members must place `#N` before the field or case declaration kind | not stated | 1
DATA_AND_LITERALS12 | "A record or case literal without `..` supplies every ordinary runtime field of the constructed value." | Explicit field omission | Literals without `..` must initialize every ordinary runtime field | at the operation | 1
DATA_AND_LITERALS13 | "Each omitted value and the complete aggregate must satisfy their default-domain, qualification, establishment and custody obligations." | Explicit field omission | Zero-filled omissions and the full aggregate must meet default-domain, qualification, establishment, and custody obligations | at the operation | 1
DATA_AND_LITERALS14 | "The marker supplies no authority, borrowed reference, callable or proof witness merely because storage can be zeroed." | Explicit field omission | `..` must not grant authority, borrows, callables, or proof witnesses solely from zeroed storage | at the operation | 2
DATA_AND_LITERALS15 | "let missing: Options = Options { timeout_ms: 1000 }; // Rejected: retries missing." | Explicit field omission | Exhaustive record literals missing a required runtime field must be rejected | at the operation | 1
DATA_AND_LITERALS16 | "A literal with `..` accepts the new zero value only if the complete construction remains valid." | Explicit field omission | After adding a runtime field, `..` literals must still pass full-construction validity checks | at the operation | 1
DATA_AND_LITERALS17 | "A required nonzero or computed value must still be supplied explicitly." | Explicit field omission | Fields that cannot be validly zero or computed-default must appear explicitly in the literal | at the operation | 1
DATA_AND_LITERALS18 | "There is at most one marker, after all authored field initializers; it has no operand and evaluates nothing." | Explicit field omission | At most one `..`, only after all field initializers, with no operand and no evaluation | at the operation | 1
DATA_AND_LITERALS19 | "`Type { }` is exhaustive only when there are no runtime fields." | Explicit field omission | Empty braces are exhaustive only if the type has no runtime fields | at the operation | 1
DATA_AND_LITERALS20 | "The marker is neither record-update/spread syntax nor a pattern rest binding; it changes no array length, range endpoint or positional-call rule." | Explicit field omission | `..` in literals must not be treated as spread, pattern rest, or as altering array length, range endpoints, or positional-call rules | not stated | 1
DATA_AND_LITERALS21 | "It does not select a case, initialize inactive payloads or bypass case constraints." | Explicit field omission | Case literal `..` must not select a case, initialize inactive payloads, or bypass case constraints | at the operation | 1
DATA_AND_LITERALS22 | "A bare payload-free case value is exhaustive only when it also has no runtime common fields; otherwise use its case literal with explicit common fields or `..`." | Explicit field omission | Bare payload-free case values without common runtime fields may be exhaustive; otherwise require explicit common fields or `..` | at the operation | 1
DATA_AND_LITERALS23 | "Nested literals each govern their own omissions; an outer marker does not authorize omissions inside an authored inner literal." | Explicit field omission | Inner authored literals must satisfy their own field coverage; outer `..` does not authorize inner omissions | at the operation | 1
DATA_AND_LITERALS24 | "Fields have no declaration-site default initializers." | Explicit field omission | Field declarations must not carry declaration-site default initializers | not stated | 1
DATA_AND_LITERALS25 | "Case-dependent projections require established case knowledge." | Explicit field omission | Projections that depend on case must have established case knowledge before use | at the operation | 1
DATA_AND_LITERALS26 | "Field and payload type spellings do not carry bracketed scalar range suffixes." | Explicit field omission | Field and payload types must reject bracketed scalar range suffixes on spellings | not stated | 1
DATA_AND_LITERALS27 | "Construction and every restoring write prove the owning default-domain clauses." | Explicit field omission | Construction and restoring writes must prove the owning default-domain clauses | at the operation | 1
DATA_AND_LITERALS28 | "Case-bearing values use a case literal, not record-only construction." | Explicit field omission | Sum values must be built with case literals, not record-only construction syntax | at the operation | 1
DATA_AND_LITERALS29 | "Common and payload fields may be named together and evaluate exactly once in authored order." | Explicit field omission | Case literals must evaluate common and payload fields exactly once in authored order | at the operation | 1
DATA_AND_LITERALS30 | "payload fields require the corresponding case knowledge." | Explicit field omission | Payload field access requires knowledge that the corresponding case is active | at the operation | 1
DATA_AND_LITERALS31 | "Collisions reject rather than selecting by priority." | Explicit field omission | Namespace collisions between cases, domains, and data must reject rather than resolve by priority | not stated | 1
DATA_AND_LITERALS32 | "Expected type, imports, and local context do not infer a case owner for a single-segment name: write `Light::On`, not `On`." | Explicit field omission | Single-segment case names must be rejected when a qualified `Type::Case` form is required | not stated | 1
DATA_AND_LITERALS33 | "`Command::Move` without its literal remains the case domain, not a partially constructed value." | Explicit field omission | Unqualified-or-domain-only case references must not denote partial construction when a literal is required | not stated | 1
DATA_AND_LITERALS34 | "Cases are nominal, not foreign integers." | Explicit field omission | Sum values must not be formed from raw foreign integers without nominal mapping | not stated | 1
DATA_AND_LITERALS35 | "No raw integer silently establishes a sum value." | Explicit field omission | Integer-to-sum conversions must not be silent; mapping must be explicit and checked | at the operation | 5
DATA_AND_LITERALS36 | "That is accessible only after the active value's default-domain obligations hold; inactive payloads need no establishment." | Explicit field omission | Case-bearing zero representation is usable only after default-domain obligations hold; inactive payloads need no establishment | not stated | 1
DATA_AND_LITERALS37 | "A repairable payload predicate alone does not establish that its excluded patterns remain unavailable throughout those uses." | Explicit field omission | Predicate repairability must not be treated as proving excluded patterns stay unavailable for the whole use | not stated | 8
DATA_AND_LITERALS38 | "For executable structural equality, compare common fields, case identity, and the active payload, never inactive storage/padding." | Explicit field omission | Executable structural equality must use common fields, case identity, and active payload only, excluding inactive storage or padding | at the operation | 1
DATA_AND_LITERALS114 | "Explicit `core::Identity<T>(left, right)` states proof-relevant identity without requiring an executable equality algorithm." | Explicit field omission | `core::Identity<T>` must express proof-relevant identity without mandating an executable equality algorithm | at the operation | 8
DATA_AND_LITERALS115 | "Structural comparison exposes checked component comparisons." | Explicit field omission | Structural comparison must surface checked comparisons per component | at the operation | 1
DATA_AND_LITERALS116 | "A payload-free case with no runtime common fields can also denote a constructed value." | Explicit field omission | Payload-free cases without runtime common fields may denote a fully constructed value without a case literal | not stated | 1
DATA_AND_LITERALS39 | "records and payload-bearing sums select the declared synthesis/conformance required by Equatable." | Explicit field omission | Record and payload-bearing sum equality must use the declared Equatable conformance, not an implicit algorithm | not stated | 1
DATA_AND_LITERALS40 | "Adding a payload requires that declaration rather than silently changing tag equality into field equality." | Explicit field omission | Adding a payload to a sum must require explicit Equatable declaration rather than silently switching equality model | not stated | 1
DATA_AND_LITERALS41 | "conversion to identity requires the applicable checked bridge law and cannot silently reinterpret float fields." | Explicit field omission | Identity conversion must use the checked bridge law and must not silently reinterpret float fields | at the operation | 5
DATA_AND_LITERALS42 | "Cases have no `requires` clause." | Case constraints | Case members must not have their own `requires` clauses | not stated | 1
DATA_AND_LITERALS43 | "Accessing `self.lo` or `self.hi` above requires the Range case established by the preceding conjunct." | Case constraints | Projections like `self.lo` in default-domain predicates require the guarding case to be established in the same predicate | not stated | 8
DATA_AND_LITERALS44 | "inactive payloads cannot be projected, and a predicate cannot assume the default domain it is establishing to justify its own terms." | Case constraints | Inactive payload projection must be rejected; predicates must not circularly assume the domain they establish | not stated | 8
DATA_AND_LITERALS45 | "A field's qualification constrains that field, not the enclosing sum's case." | Case constraints | Field qualifications apply only to that field, not to case validity of the whole sum | not stated | 1
DATA_AND_LITERALS46 | "This introduces no case-local generic or hidden existential binders." | Case constraints | Case constraints must not introduce case-local generics or hidden existential binders | not stated | 7
DATA_AND_LITERALS47 | "Omitted data predicates contribute true, not a waiver of payload validity." | Case constraints | Missing data-level `requires` means true for the predicate conjunction, not a waiver of per-payload validity | not stated | 1
DATA_AND_LITERALS48 | "Inactive payloads have no establishment or ownership obligation." | Case constraints | Inactive case payloads must carry no establishment or ownership obligation | not stated | 2
DATA_AND_LITERALS49 | "In a case-conditional predicate, an inactive alternative cannot satisfy a conjunction requiring its own case." | Case constraints | Inactive cases must not satisfy conjuncts that require their own case membership | not stated | 8
DATA_AND_LITERALS117 | "It may name enclosing generic parameters, common fields, `self`, and otherwise lexically available contract names." | Case constraints | Data default-domain clauses may reference only the listed names plus lexically available contract names | not stated | 8
DATA_AND_LITERALS118 | "Fields AND Predicate(self) AND OR over cases i (active_case == i AND Payload(i))" | Case constraints | Established values must satisfy common-field validity, data predicates, and exactly one active case payload | not stated | 1
DATA_AND_LITERALS50 | "Construction proves the complete default domain before producing an established value." | Case constraints | Value construction must prove the full default-domain formula before the value is established | at the operation | 1
DATA_AND_LITERALS51 | "Constructing `Integer` as `Value<bool>` rejects; matching that combination establishes a contradiction, not a conversion." | Case constraints | Case construction incompatible with generic parameters must reject; matching incompatible case/type pairs must yield contradiction, not conversion | at the operation | 7
DATA_AND_LITERALS52 | "A type equation reveals an equality; it does not reassign a generic parameter, reinterpret bytes, or insert a runtime type test." | Case constraints | Type equalities from matching must not reassign generics, reinterpret bytes, or insert runtime type tests | at the operation | 7
DATA_AND_LITERALS53 | "Before observation, the active value's complete conditions must hold again." | Case constraints | Before observing a value, all default-domain conditions for the active value must hold | at the operation | 1
DATA_AND_LITERALS54 | "Source is valid UTF-8, with ASCII syntax." | Lexical profile | Source files must be valid UTF-8 with ASCII syntax rules as specified | not stated | SHARED
DATA_AND_LITERALS55 | "Identifiers match `[A-Za-z_][A-Za-z0-9_]*` and compare by exact bytes." | Lexical profile | Identifiers must match the ASCII pattern and compare by exact byte sequence | not stated | SHARED
DATA_AND_LITERALS56 | "Other Unicode whitespace rejects." | Lexical profile | Unicode whitespace outside the four allowed ASCII whitespace characters must be rejected at lex time | not stated | SHARED
DATA_AND_LITERALS57 | "Host Unicode tables, normalization, or confusable-name heuristics cannot change tokenization." | Lexical profile | Tokenization must not depend on host Unicode tables, normalization, or confusable-name heuristics | not stated | SHARED
DATA_AND_LITERALS58 | "The retired `dyn` spelling has no source type or conversion form." | Lexical profile | `dyn` spellings must be rejected as they have no source type or conversion form | not stated | SHARED
DATA_AND_LITERALS59 | "Radix and grouping affect source notation, not numeric meaning or overflow policy." | Integer radix spellings | Literal radix and underscores must affect only notation; numeric value and overflow policy follow type semantics | not stated | 5
DATA_AND_LITERALS60 | "Codepoint escapes and raw newlines inside quotes reject." | Quoted bytes | Quoted byte literals must reject codepoint escapes and unescaped raw newlines inside quotes | not stated | 6
DATA_AND_LITERALS61 | "Byte length must match exactly: no truncation, padding, or hidden live-length field." | Quoted bytes | Fixed-array literals from quoted bytes must match exact byte length with no truncation, padding, or hidden length | not stated | 6
DATA_AND_LITERALS62 | "Temporary evaluator views cannot escape; only an owned value snapshot crosses its result boundary." | Quoted bytes | Evaluator temporary views of literal bytes must not escape; only owned snapshots may cross the evaluator result boundary | not stated | 1
DATA_AND_LITERALS63 | "`data` declares fields, cases, or both." | Data, literals, and lexical framing | Each `data` declaration must declare fields, cases, or both per the grammar | not stated | 1
DATA_AND_LITERALS64 | "Machines access values through their explicit parameters." | Data, literals, and lexical framing | Value access from machines must go only through explicit parameters | not stated | 10
DATA_AND_LITERALS65 | "A final standalone `..` explicitly requests zero values for omitted runtime fields." | Explicit field omission | Standalone trailing `..` must mean zero for each omitted ordinary runtime field | at the operation | 1
DATA_AND_LITERALS66 | "Adding a runtime field makes an exhaustive literal require an update." | Explicit field omission | Exhaustive literals must be updated when a new runtime field is added to the type | not stated | 1
DATA_AND_LITERALS67 | "`Type { .. }` requests zero for every runtime field." | Explicit field omission | `Type { .. }` must zero every runtime field of `Type` | at the operation | 1
DATA_AND_LITERALS68 | "For a case literal, the same marker covers omitted common fields and fields of the explicitly selected active payload." | Explicit field omission | One `..` in a case literal must cover omitted common and active-payload runtime fields only | at the operation | 1
DATA_AND_LITERALS69 | "That rule does not require or derive from `..`." | Explicit field omission | Erased-field nullary omission rules must not be implemented as or derived from the `..` marker | not stated | 1
DATA_AND_LITERALS70 | "Field and case-payload relationships belong to the data declaration's ordinary `requires`." | Explicit field omission | Cross-field and case-payload constraints must live in the data declaration's ordinary `requires`, not elsewhere | not stated | 1
DATA_AND_LITERALS71 | "Common fields are accessible independently of the active case." | Explicit field omission | Common fields must be accessible without establishing which case is active | at the operation | 1
DATA_AND_LITERALS72 | "Partial construction and disposal follow [ownership order](ownership.md#construction-and-disposal-order)." | Explicit field omission | Construction and disposal ordering must follow ownership order rules | at the operation | 2
DATA_AND_LITERALS73 | "A case implicitly declares its same-named domain." | Explicit field omission | Each case must declare an implicit same-named domain | not stated | 1
DATA_AND_LITERALS74 | "`Type::Case` therefore denotes case membership in domain position." | Explicit field omission | In domain position, `Type::Case` must denote case membership, not a value, unless other rules allow | not stated | 1
DATA_AND_LITERALS75 | "other case values use a case literal, including `..` when runtime fields are omitted." | Explicit field omission | Case values other than the narrow payload-free exception must use case literals, with `..` when fields are omitted | at the operation | 1
DATA_AND_LITERALS76 | "Cases and attached domains retain their member namespace; data does not own machine declarations." | Explicit field omission | Case/domain members keep their namespace; machine declarations must not be owned by the data declaration | not stated | 1
DATA_AND_LITERALS77 | "Case values always use their carrier-qualified member path." | Explicit field omission | Case values must use qualified paths, never unqualified case-only paths where qualification is required | not stated | 1
DATA_AND_LITERALS78 | "This rule is `bare-case-value-names` and applies uniformly in locals, arguments, returns, comparisons, and nested expressions." | Explicit field omission | Bare case name rules must apply in locals, arguments, returns, comparisons, and nested expressions | not stated | 1
DATA_AND_LITERALS79 | "`#N` is schema identity, not a runtime tag assignment." | Explicit field omission | `#N` on members must affect schema identity only, not runtime tag values | not stated | 1
DATA_AND_LITERALS80 | "Foreign codes cross in their declared integer carrier and use ordinary checked mapping machines, including an explicit unknown-code outcome." | Explicit field omission | Foreign integer codes must map through declared carrier and checked machines with an unknown-code outcome | at the operation | 5
DATA_AND_LITERALS81 | "The ordinary case-bearing zero representation selects the first declared case and recursively zeroed common fields/payload." | Explicit field omission | Default zero for case-bearing types must use the first declared case with recursive zero of common and payload fields | not stated | 1
DATA_AND_LITERALS82 | "Zero does not imply semantic emptiness or authority." | Explicit field omission | Zeroed values must not be treated as semantically empty or as carrying authority by default | not stated | 1
DATA_AND_LITERALS83 | "Stable external placement uses [layout policies](../layouts/plans.md), not inferred native offsets." | Explicit field omission | Externally stable layout must come from layout policies, not inferred native offsets | not stated | 1
DATA_AND_LITERALS84 | "Value equality and domain membership are distinct." | Explicit field omission | Equality facts and domain membership must not be conflated | at the operation | 1
DATA_AND_LITERALS85 | "Primitive and payload-free-sum equality is intrinsic." | Explicit field omission | Primitive and payload-free-sum types must use intrinsic equality, not declared Equatable | at the operation | 1
DATA_AND_LITERALS86 | "Value `==` retains that selected comparison in facts." | Explicit field omission | `==` must retain the selected equality comparison in the fact system | at the operation | 1
DATA_AND_LITERALS87 | "Case-dependent validity is an ordinary predicate of the whole value in the data declaration's default-domain `requires`." | Case constraints | Case validity must be expressed only through the data default-domain `requires`, not case-local clauses | not stated | 1
DATA_AND_LITERALS88 | "Its `self` subject follows [predicate subjects](domains.md#declaration-and-membership)." | Case constraints | Default-domain predicates must use `self` per predicate-subject rules | not stated | 8
DATA_AND_LITERALS89 | "The data clause uses ordinary well-formed contract propositions, including Boolean combinations, exact type equality, and value couplings." | Case constraints | Data `requires` clauses must be well-formed contract propositions of the listed kinds | not stated | 8
DATA_AND_LITERALS90 | "A conformance obligation does not select an implementation or manufacture evidence; ordinary explicit selection and proof-term rules still apply." | Case constraints | Conformance obligations must not auto-select implementations or evidence | not stated | 8
DATA_AND_LITERALS91 | "data `requires` makes it mandatory for every established carrier value." | Case constraints | When data declares `requires`, the predicate must hold for every established value of the carrier | not stated | 1
DATA_AND_LITERALS92 | "Matching combines that domain with the selected case fact through ordinary arm-local fact rules." | Case constraints | Pattern matching must merge default domain with case facts via arm-local fact rules | at the operation | 1
DATA_AND_LITERALS93 | "Value couplings obey the same establishment and mutation rules as type-wide invariants." | Case constraints | Value couplings in default-domain predicates follow dependent-value establishment and mutation rules | not stated | 1
DATA_AND_LITERALS94 | "Replacing a case or changing its payload invalidates dependent place facts as appropriate; snapshots keep their own exact subjects." | Case constraints | Case or payload change must invalidate dependent place facts; snapshots keep their subjects | at the operation | 2
DATA_AND_LITERALS95 | "Zero storage and generic bodies follow the existing default-domain gate." | Case constraints | Zero storage and generic bodies must satisfy the default-domain gate | not stated | 1
DATA_AND_LITERALS96 | "Checked encodings preserve the case conditions and induction/computation rules without selecting physical layout or merging nominal source identities." | Case constraints | Encodings must preserve case conditions and rules without picking layout or merging nominal identities | not stated | 8
DATA_AND_LITERALS97 | "Syntactic whitespace is only space, tab, carriage return, and line feed." | Lexical profile | Only the four ASCII whitespace characters may be syntactic whitespace | not stated | SHARED
DATA_AND_LITERALS98 | "Punctuation, operators, keywords, and numeric spellings use ASCII; non-ASCII bytes occur only in comments and literal bodies." | Lexical profile | Non-ASCII bytes outside comments and literal bodies must be rejected in tokens | not stated | SHARED
DATA_AND_LITERALS99 | "`some` introduces a per-value type binder in data parameter lists; it is not a visibility modifier or an alternative spelling of an ordinary type argument." | Lexical profile | `some` in data parameter lists must bind types per-value only, not as visibility or ordinary type args | not stated | 7
DATA_AND_LITERALS100 | "This profile has no Unicode identifiers or raw-payload literal form." | Lexical profile | Unicode identifiers and raw-payload literals must be rejected under this profile | not stated | SHARED
DATA_AND_LITERALS101 | "Integer literals support decimal, hexadecimal `0x`, binary `0b`, and octal `0o` spellings, digit underscores, and explicit carrier suffixes." | Integer radix spellings | Integer literals must accept only the listed radix forms, underscores, and suffixes | not stated | 5
DATA_AND_LITERALS102 | "Quoted-byte `\\xNN` escapes below are separate literal syntax." | Integer radix spellings | `\xNN` in quoted bytes must not be parsed as integer literal syntax | not stated | 6
DATA_AND_LITERALS103 | "A quoted literal contains bytes, has ordinary shared byte-view type `&[u8]`, and establishes no encoding domain." | Quoted bytes | Quoted literals must type as `&[u8]` and must not establish an encoding domain | not stated | 6
DATA_AND_LITERALS104 | "Its runtime shared view refers to immutable image storage, not a invocation-local owner." | Quoted bytes | Quoted literal views must refer to immutable image storage, not invocation-local ownership | not stated | 2
DATA_AND_LITERALS105 | "Source UTF-8 framing does not make the value text or normalize/transcode its contents." | Quoted bytes | UTF-8 source framing must not imply text typing or normalization of quoted literal bytes | not stated | 6
DATA_AND_LITERALS106 | "The byte specified by two hexadecimal digits." | Quoted bytes | `\xNN` escapes must use exactly two hexadecimal digits mapping to one byte | not stated | 6
DATA_AND_LITERALS107 | "Explicit qualification proves the chosen encoding's predicates, not a compiler-inferred text interpretation." | Quoted bytes | Encoding predicates require explicit qualification; compiler must not infer text encoding for literals | not stated | 6
DATA_AND_LITERALS108 | "In an exact-width owned fixed-array constant or evaluator result, a literal copies its bytes into the array." | Quoted bytes | Fixed-array constants and evaluator results must copy quoted literal bytes into the array | not stated | 6
DATA_AND_LITERALS109 | "Constant interning is an unobservable optimization, not source storage identity." | Quoted bytes | Constant interning must not change observable identity of literal storage | not stated | 1
DATA_AND_LITERALS110 | "The lexer copies literal-body source bytes except these byte escapes:" | Quoted bytes | Lexer must copy quoted literal body bytes verbatim except for the listed escape sequences | not stated | SHARED
DATA_AND_LITERALS111 | "build-resource inclusion requires admitted build inputs." | Quoted bytes | Including literal bytes as build resources must use only admitted build inputs | not stated | 9

## Ambiguous sentences

Sentence | Reading A | Reading B
--- | --- | ---
"A common field after the first inline case is retired." | The compiler must reject any common field placed after the first inline case. | "Retired" names a prior language rule elsewhere; this sentence only restates that rule without stating the diagnostic or enforcement point in this chapter. |
"This source order does not constrain a selected layout policy's physical placement or add a case-group schema member, number or discriminator." | Layout lowering must ignore source order for physical placement and must not invent case-group schema artifacts from order. | Source order is irrelevant only when a layout policy is already selected; without a policy, whether order may influence defaults is not stated. |
"Each omitted value and the complete aggregate must satisfy their default-domain, qualification, establishment and custody obligations." | The compiler must prove or check all listed obligations for each zeroed field and for the whole value at construction. | Obligations must hold semantically at runtime; whether the compiler must verify all at compile time versus deferring some is not stated per obligation kind. |
"Erased fields instead follow erased construction: an accessible nullary constructor may supply an omitted term without requiring runtime layout or a zero value." | For erased fields, omission may use a nullary constructor path distinct from `..` zeroing, per the linked erased-construction rules. | "Accessible nullary constructor" is undefined in this chapter; what counts as accessible and when omission is allowed without `..` is delegated entirely to the linked section. |
"Home-layout realization may encode case identity in the payload representation and omit a separate tag when it preserves the language's semantics, including zero's meaning and legal mutation, observation and custody." | Backend may omit tags only if full language semantics including zero, mutation, observation, and custody are preserved. | "May encode" permits optional optimizations; whether the compiler must validate preservation at compile time or only the layout pass is not stated. |
"The mathematical interpretation uses the selected W-based inductive profile." | Proof and semantics obligations for case constraints follow the W-based profile referenced. | Which obligations are proof-only versus executable compiler checks in the inductive profile is not stated here. |
"Punctuation, operators, keywords, and numeric spellings use ASCII; non-ASCII bytes occur only in comments and literal bodies." | Non-ASCII bytes outside comments and literal bodies must be rejected in tokenization. | "Literal bodies" might include only quoted-byte bodies or all literal forms; whether non-ASCII in numeric or other literal forms is allowed is not fully enumerated in this sentence. |
"Explicit qualification proves the chosen encoding's predicates, not a compiler-inferred text interpretation." | Text encoding facts require explicit qualification; the compiler must not infer encoding predicates for quoted bytes. | Quoted literals establish `&[u8]` without encoding domain; whether "explicit qualification" applies only to later conversions or also constrains literal forms is not stated in this subsection. |
"Unnumbered alternatives remain `case Name`." | Cases without `#N` must use `case Name` syntax only. | "Remain" might only preserve legacy programs while new code must use `#N`; whether unnumbered cases are optional or mandatory where no number is given is not stated. |
"A numbered member writes its identity before its declaration kind." | Schema members with numbers must list `#N` before `case` or field kind keywords. | "Writes its identity" might mean source syntax only or also a semantic identity table requirement beyond parse order. |
"For Value<T>, selecting Integer therefore establishes T == i32; selecting Range in Interval establishes self.lo <= self.hi." | Pattern matching must establish these equalities as facts in the stated examples. | Whether all analogous default-domain predicates must establish similar facts generally, or only these illustrated forms, is not stated as a universal rule. |
"This retains constrained-case GADT behavior through ordinary domains, without case-level `requires`, a wrapper, a new data species, implicit boxing, runtime type tests, or general existential packaging." | The implementation must not introduce the listed mechanisms when enforcing case constraints. | "This retains" describes intended behavior relative to GADT semantics; whether violation of any listed prohibition is a hard language error or documentation-only is not stated. |

## Terms used without definition in this chapter

- fields / cases / record / sum / mixed declaration
- machine values
- explicit parameters
- runtime callable fields
- machine declarations
- implicit receiver
- inline case declarations
- common fields
- data decomposition
- selected layout policy
- physical placement
- case-group schema member
- transparent type alias
- nominal carrier
- storage wrapper
- schema identity (`#N`)
- declaration kind
- payload identity scope
- numbering / retirement / declaration-order / stable-identity rules
- ordinary runtime field
- default-domain
- qualification
- establishment
- custody obligations
- constructor machines
- erased fields
- erased construction
- nullary constructor
- restoring write
- case literal
- case knowledge
- dispatch / pattern rules
- same-named domain
- domain position
- member namespace
- carrier-qualified member path
- bare-case-value-names
- foreign integers
- checked mapping machines
- unknown-code outcome
- case-bearing zero representation
- home-layout realization
- payload predicate
- layout policies
- value equality vs domain membership
- intrinsic equality
- synthesis/conformance
- Equatable
- `core::Identity<T>`
- proof-relevant identity
- checked bridge law
- default-domain `requires`
- predicate subjects
- well-formed contract propositions
- value couplings
- guarded formation
- case reasoning
- conformance obligation
- explicit selection
- Fields / Payload(i) / Predicate(self) / active_case
- arm-local fact rules
- dependent place facts
- snapshots
- default-domain gate
- W-based inductive profile
- checked encodings
- syntactic whitespace
- per-value type binder (`some`)
- visibility modifier
- Unicode identifiers
- raw-payload literal form
- carrier suffixes
- carrier-limit queries
- quoted literal
- shared byte-view type
- encoding domain
- immutable image storage
- invocation-local owner
- byte escapes
- exact-width owned fixed-array constant
- constant interning

CHAPTER COMPLETE
