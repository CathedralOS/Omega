# Patterns and dispatch

Source: `wiki/spec/language/patterns.md` lines 1–249 (read).

## Rules

rule id | the exact sentence, quoted | the heading it is under | what must be checked, in plain words | when it must hold | lane
--- | --- | --- | --- | --- | ---
PATTERNS1 | "A dispatch evaluates its subject once, tests alternatives in authored order, and executes only the selected arm." | Patterns and dispatch | The subject expression is evaluated exactly once; arms are tried in source order; only the matching arm's body or edge runs | at the operation | 1
PATTERNS2 | "Matching contributes facts valid for that subject and program point." | Patterns and dispatch | Facts introduced by a successful match must be scoped to the matched subject and the current program point | at the operation | 8
PATTERNS3 | "Failed alternatives do not execute their bodies." | Patterns and dispatch | Arms that do not match must not run their bodies or take their control edges | at the operation | 0
PATTERNS4 | "An expression `match` produces compatible arm values;" | Patterns and dispatch | All `match` arms must yield values mutually compatible with the match's result type | at normal return | 1
PATTERNS5 | "a `transition` selects an outgoing control edge." | Patterns and dispatch | A matching transition arm must select exactly one outgoing control edge | on every edge | 0
PATTERNS6 | "Both use the same pattern vocabulary." | Patterns and dispatch | `match` and `transition` must accept the same pattern forms with the same meaning | not stated | 1
PATTERNS7 | "Decomposing a `some` package uses one stable subject/version opening for all dependent fields." | Patterns and dispatch | `some` package decomposition must use a single stable subject or version opening shared by all dependent fields | at the operation | 1
PATTERNS8 | "Patterns cannot guess its concrete witness, equate it with another package's witness, or let dependent bindings escape their valid scope." | Patterns and dispatch | Patterns must not infer a concrete witness, equate witnesses across packages, or let dependent bindings outlive their valid scope | at the operation | 1
PATTERNS9 | "This is ordinary decomposition under the [package opening rules](some_packages.md#opening-by-stable-subject), not a runtime type test." | Patterns and dispatch | `some` decomposition must follow package opening rules; it must not be implemented or checked as a runtime type test | at the operation | 1
PATTERNS10 | "Scalars use value patterns." | Pattern subjects and bindings | Scalar dispatch subjects must use value patterns | at the operation | 0
PATTERNS11 | "Records use structural or domain patterns." | Pattern subjects and bindings | Record subjects must use structural or domain patterns | at the operation | 1
PATTERNS12 | "Case-bearing subjects use case patterns, domain patterns, and payload bindings." | Pattern subjects and bindings | Case-bearing subjects must use case patterns, domain patterns, and payload bindings as applicable | at the operation | 1
PATTERNS13 | "Tuple dispatch combines component patterns;" | Pattern subjects and bindings | Tuple dispatch must decompose into component patterns for each tuple element | at the operation | 1
PATTERNS14 | "`_` waives a component without creating a value or fact about it." | Pattern subjects and bindings | A `_` component pattern must not introduce a binding, value, or fact for that component | at the operation | 1
PATTERNS15 | "Payload bindings require a known case shape." | Pattern subjects and bindings | Payload bindings are permitted only when the case shape is known | at the operation | 1
PATTERNS16 | "`field` binds the field, `field as name` renames it, `field as _` waives it, and `field: value` is ordinary projected equality." | Pattern subjects and bindings | Field selectors must bind, rename, waive, or test projected equality according to these spellings | at the operation | 1
PATTERNS17 | "Without `..`, a structural pattern mentions every field; adding a field invalidates incomplete patterns." | Pattern subjects and bindings | Structural patterns without `..` must list every field; schema drift (new fields) must invalidate incomplete patterns | not stated | 1
PATTERNS18 | "`..` opts out of this drift check in arm position." | Pattern subjects and bindings | `..` in an arm position must opt the pattern out of the exhaustive field mention / drift check | not stated | 1
PATTERNS19 | "Extraction uses the saved subject and retains ordinary ownership and borrow obligations." | Pattern subjects and bindings | Pattern extraction must use the saved subject snapshot and must preserve ownership and borrow obligations | at the operation | 2
PATTERNS20 | "A record decomposes into its declared fields." | Data and case decomposition | Record patterns must decompose along declared fields only | at the operation | 1
PATTERNS21 | "A mixed value decomposes into its common fields in authored declaration order, followed by one case-only component." | Data and case decomposition | Mixed data patterns must bind common fields in declaration order then exactly one case component | at the operation | 1
PATTERNS22 | "All inline cases contribute to that one final component, not one component per case." | Data and case decomposition | Multiple inline cases must map to a single case component in decomposition, not separate components per case | at the operation | 1
PATTERNS23 | "Inline case declarations form a consecutive tail after the common fields under [data declarations](data_and_literals.md)." | Data and case decomposition | Inline cases must be treated as a consecutive tail after common fields per data declaration rules | not stated | 1
PATTERNS24 | "Named and positional data patterns express the same decomposition." | Data and case decomposition | Named and positional patterns for the same data type must decompose identically | at the operation | 1
PATTERNS25 | "These positions are semantic declaration order, not stable schema numbers, physical field offsets or layout-policy order." | Data and case decomposition | Positional pattern indices must follow semantic declaration order, not schema numbers, physical offsets, or layout-policy order | at the operation | 1
PATTERNS26 | "For a record without cases, the positional sequence contains only its fields." | Data and case decomposition | Positional patterns on caseless records must have arity equal to field count only | at the operation | 1
PATTERNS27 | "Named patterns account for every common field and the case component once; positional patterns have the corresponding exact arity." | Data and case decomposition | Named patterns must cover each common field and the case component once; positional patterns must have matching arity | at the operation | 1
PATTERNS28 | "An explicit `..` may waive the remaining components." | Data and case decomposition | `..` may waive unspecified trailing components in positional or named forms | at the operation | 1
PATTERNS29 | "`_`, `field as _` and `case as _` ignore bindings, not ownership, cleanup or validity obligations." | Data and case decomposition | Ignore forms waive bindings only; ownership, cleanup, and validity obligations still apply | at the operation | 2
PATTERNS30 | "`case` is a reserved pattern selector, not a stored field or a general field access." | Data and case decomposition | `case` in patterns must be treated as reserved selector syntax, not as an ordinary field name or access | at the operation | 1
PATTERNS31 | "It introduces no schema member, stable number, storage slot, additional discriminator or independently constructed enum." | Data and case decomposition | The `case` selector must not add schema members, stable numbers, slots, discriminators, or independent enum construction | not stated | 1
PATTERNS32 | "Ordinary field renaming remains `field as binding`; `field: value` remains a value test, not a rename." | Data and case decomposition | Field rename and value-test spellings must not be conflated | at the operation | 1
PATTERNS33 | "The inner case pattern destructures only that case's payload." | Data and case decomposition | Nested case patterns may bind only the selected variant's payload, not common fields | at the operation | 1
PATTERNS34 | "Common-field names in a case pattern are retired: obtain them in the outer data pattern." | Data and case decomposition | Common fields must not appear in inner case patterns; reject or require outer data pattern for them | at the operation | 1
PATTERNS35 | "A pure sum is already case-only and supports direct case matching without a redundant outer pattern." | Data and case decomposition | Pure sums must allow direct case matching without an enclosing data pattern when appropriate | at the operation | 1
PATTERNS36 | "Case names retain their original carrier-qualified paths, as `Command::GetUser` above; projection does not invent a new case owner." | Data and case decomposition | Case patterns must use carrier-qualified case paths; decomposition must not invent new case owners | at the operation | 1
PATTERNS37 | "A case view retains the original subject, static arguments and exact storage identity." | Data and case decomposition | Case views must preserve original subject, static arguments, and storage identity | at the operation | 2
PATTERNS38 | "Outer bindings and still-valid facts remain available inside nested matches;" | Data and case decomposition | Valid outer bindings and facts must remain in scope in nested matches | at the operation | 8
PATTERNS39 | "selecting a case combines its fact with the original value's established default- and named-domain predicates, including case-dependent type equalities." | Data and case decomposition | On case selection, combine case fact with existing default- and named-domain predicates including case-dependent equalities | at the operation | 8
PATTERNS40 | "It does not drop common-field constraints or create validity/authority evidence." | Data and case decomposition | Case selection must not discard common-field constraints or fabricate validity or authority evidence | at the operation | 8
PATTERNS41 | "Mutation invalidates facts under ordinary footprint rules." | Data and case decomposition | After mutation, invalidate facts per ordinary footprint invalidation rules | at the operation | 2
PATTERNS42 | "Borrowed decomposition retains the subject's lifetime, access and loans." | Data and case decomposition | Decomposition of borrowed subjects must retain lifetime, access mode, and active loans | at the operation | 2
PATTERNS43 | "Owned decomposition follows ordinary move, affine cleanup and linear-custody rules;" | Data and case decomposition | Owned decomposition must follow move, affine cleanup, and linear-custody rules | at the operation | 2
PATTERNS44 | "the common fields and active payload cannot be duplicated or disposed of twice." | Data and case decomposition | Common fields and active payload must not be duplicated or dropped twice through decomposition | at the operation | 2
PATTERNS45 | "The view cannot escape its backing lifetime or widen access." | Data and case decomposition | Case or decomposition views must not outlive backing storage or gain wider access than the source reference | at the operation | 2
PATTERNS46 | "Each dispatch evaluates its own subject once and proves its own coverage;" | Data and case decomposition | Each dispatch must have its own single evaluation of its subject and its own coverage proof | at the operation | 4
PATTERNS47 | "projecting the case component never reevaluates the expression that produced the outer value." | Data and case decomposition | Projecting the case component must not re-run the outer subject expression | at the operation | 0
PATTERNS48 | "`Command::case` is a type expression naming the case component of the mixed data `Command`, and `Command<T>::case` names it for one application of generic data." | Case projection type | `Data::case` and generic `Data<T>::case` must name the case component type per data declaration | not stated | 7
PATTERNS49 | "It is the type of the component a `case as name` pattern already binds, so a machine can accept that component without receiving, or decomposing again, the complete value:" | Case projection type | Projection type must match what `case as name` binds; callees may take projection without full value decomposition | at a call | 7
PATTERNS50 | "Decomposing `&Command` binds `operation` at `&Command::case`, and decomposing `&mut Command` binds it at `&mut Command::case`." | Case projection type | Reference decomposition must bind the case component at the corresponding reference projection type | at the operation | 2
PATTERNS51 | "The type's identity is the data declaration, its static arguments and this projection." | Case projection type | Projection type identity must be fixed by data declaration, static arguments, and projection | not stated | 7
PATTERNS52 | "Two declarations with identical cases have distinct projection types." | Case projection type | Distinct data declarations must yield distinct projection types even when case sets are identical | not stated | 7
PATTERNS53 | "`case` is reserved, so the path cannot collide with a case or member name." | Case projection type | `::case` projection paths must not collide with case or member names due to reservation | not stated | 1
PATTERNS54 | "Naming the projection adds nothing to the component." | Case projection type | Projection naming must not add runtime or semantic content beyond the component | not stated | 1
PATTERNS55 | "It introduces no case owner, tag, storage slot, allocation, copy or independently selected layout," | Case projection type | Projection types must not introduce owners, tags, slots, allocation, copy, or independent layout | not stated | 1
PATTERNS56 | "A projection type is written only as the referent of a reference." | Case projection type | Projection types may appear only as reference referents, not as standalone value types in signatures except as references | not stated | 2
PATTERNS57 | "It has no literal, zero default or cast, and a value of it arises only by decomposition." | Case projection type | Reject literals, zero defaults, and casts to projection types; values arise only via decomposition | at the operation | 1
PATTERNS58 | "A by-value `Command::case` annotation rejects;" | Case projection type | By-value projection type annotations must be rejected | not stated | 1
PATTERNS59 | "`Data::case` also rejects for data with no common fields, which is already case-only, and for a record with no cases." | Case projection type | `Data::case` must reject for pure case-only data and for records without cases | not stated | 1
PATTERNS60 | "A reference to the projection is the view described above." | Case projection type | Reference-to-projection must implement the case view semantics described for decomposition | at the operation | 2
PATTERNS61 | "It retains the original storage identity, lifetime, access mode and loans, cannot escape its backing lifetime or widen access, and reaches only the case component: no operation recovers the enclosing value or its common fields from it." | Case projection type | Projection references must retain identity, lifetime, access, loans; forbid escape, widen access, and recovery of enclosing value or common fields | at the operation | 2
PATTERNS62 | "The common fields remain disjoint sibling places under ordinary projected-place rules." | Case projection type | Common fields and case component must remain disjoint sibling places under projected-place rules | not stated | 2
PATTERNS63 | "A machine receiving the projection is checked independently against its own contract." | Case projection type | Callee contracts for projection parameters must be checked separately from caller's full-value contract | at a call | 3
PATTERNS64 | "The type guarantees each payload field's declared type under the selected case." | Case projection type | Under a selected case, each payload field must have its declared type on the projection type | at the operation | 1
PATTERNS65 | "A data `requires` clause belongs to the complete value and is not implied by the projection type." | Case projection type | Data-level `requires` on the whole value must not be assumed from projection type alone | at a call | 3
PATTERNS66 | "Facts the caller holds about the complete value or its common fields do not cross the call;" | Case projection type | Caller facts about full value or common fields must not be available inside callee taking only projection | at a call | 8
PATTERNS67 | "a helper that needs one states it as its own requirement over the projection, and the caller proves it." | Case projection type | Needed facts must appear in callee requires over projection; caller must prove them at call | at a call | 3
PATTERNS68 | "Through `&mut Command::case` a helper may mutate the active payload or replace the case under ordinary rules." | Case projection type | `&mut` projection may allow payload mutation or case replacement only under ordinary mutation rules | at the operation | 2
PATTERNS69 | "The lender keeps every obligation of the complete value under [contract-based repair](dependent_values.md#contract-based-repair): its default-domain and named-domain predicates must be provable again at handback from the helper's guarantees." | Case projection type | After `&mut` projection call, lender must re-establish full-value domain predicates via contract-based repair at handback | at a call | 3
PATTERNS70 | "Matching an established value combines its [default-domain predicates](data_and_literals.md#case-constraints) and any established named-domain predicates with the selected case fact, all substituted onto the exact subject and payload bindings." | Case projection type | Match on established value must merge default-domain, named-domain, and case facts with correct substitution on subject and payload | at the operation | 8
PATTERNS71 | "Case-conditional consequences follow from that combination, not case-local clauses or a special GADT proof route." | Case projection type | Case-conditional typing must follow combined predicates, not separate case-local requires or GADT-only paths | at the operation | 8
PATTERNS72 | "Type equalities use ordinary generic equality reasoning; they may justify operations and results in the arm without a cast." | Case projection type | Case-dependent equalities justify arm operations without cast when ordinary generic equality applies | at the operation | 7
PATTERNS73 | "They do not change the selected type arguments." | Case projection type | Case-derived equalities must not alter selected generic type arguments | at the operation | 7
PATTERNS74 | "At joins, only facts justified on every surviving predecessor remain." | Case projection type | Join points after match must retain only facts valid on all predecessors | on every edge | 8
PATTERNS75 | "Payload mutation and case replacement retain ordinary subject invalidation and loan rules." | Case projection type | Mutating payload or replacing case must apply ordinary invalidation and loan rules | at the operation | 2
PATTERNS76 | "Inside a machine body, `let Pattern = expression;` uses the same named or positional data-decomposition patterns as dispatch." | Irrefutable data bindings | `let Pattern = expr` in machines must accept the same decomposition patterns as match or transition | at the operation | 1
PATTERNS77 | "The subject's checked type and live facts must prove the pattern irrefutable: every admitted value reaches that binding." | Irrefutable data bindings | `let` pattern must be provably irrefutable from type and live facts; every admissible value must match | at the operation | 8
PATTERNS78 | "A pattern that can fail rejects;" | Irrefutable data bindings | Potentially failing patterns in `let` must be rejected | not stated | 0
PATTERNS79 | "a binding introduces no hidden branch, trap, fallback or runtime assertion." | Irrefutable data bindings | Irrefutable `let` must not desugar to hidden control flow or runtime checks | at the operation | 0
PATTERNS80 | "Use ordinary match arms when alternatives remain possible." | Irrefutable data bindings | When match can fail, require explicit match rather than `let` | not stated | 0
PATTERNS81 | "Evaluate the initializer once and retain its exact subject snapshot; projections do not reevaluate it." | Irrefutable data bindings | `let` initializer evaluated once; projections use snapshot without reevaluation | at the operation | 0
PATTERNS82 | "Bindings enter the following lexical scope only after formation, irrefutability, access and custody obligations are discharged." | Irrefutable data bindings | Bindings become in scope only after formation, irrefutability proof, access, and custody checks succeed | at the operation | 2
PATTERNS83 | "They retain the same static arguments, domain facts and mutation-invalidation rules as dispatch bindings." | Irrefutable data bindings | `let` bindings must carry same static args, domain facts, and invalidation rules as match bindings | at the operation | 8
PATTERNS84 | "Owned decomposition follows ordinary move/copy disposition and residual cleanup; it adds no implicit cloning or relaxed multiplicity." | Irrefutable data bindings | Owned `let` decomposition must follow move, copy, cleanup rules without implicit clone or relaxed multiplicity | at the operation | 2
PATTERNS85 | "A field-value binding from borrowed storage still needs ordinary copy eligibility." | Irrefutable data bindings | Binding field values from borrowed storage requires copy eligibility | at the operation | 2
PATTERNS86 | "Non-copy fields require an otherwise-authorized explicit borrow or waiver, not an implicit field move." | Irrefutable data bindings | Non-copy fields cannot be moved implicitly from `let`; require explicit borrow or waiver | at the operation | 2
PATTERNS87 | "A case-view binding retains the source's borrow and backing lifetime rather than copying its carrier." | Irrefutable data bindings | Case-view `let` bindings must borrow with source lifetime, not copy carrier | at the operation | 2
PATTERNS88 | "No reference-pattern syntax, new binding mode or access authority follows from destructuring." | Irrefutable data bindings | Destructuring must not introduce new reference-pattern syntax, binding modes, or access authority | not stated | 2
PATTERNS89 | "Ignored components retain their ordinary drop, retirement and restoration obligations." | Irrefutable data bindings | Waived binding components still owe drop, retirement, and restoration | at the operation | 2
PATTERNS90 | "There is no separate `match expression as Pattern` header form." | Irrefutable data bindings | Reject or do not provide `match expr as Pattern` header syntax | not stated | 1
PATTERNS91 | "Ordinary scalar local declarations are unchanged." | Irrefutable data bindings | Scalar `let` declarations keep prior rules unrelated to data decomposition | not stated | 0
PATTERNS92 | "Every dispatch must cover the admitted subject." | Coverage and domain tests | Each dispatch must exhaustively cover all values admitted for its subject type and facts | not stated | 4
PATTERNS93 | "Missing runtime coverage is a compile error, not an implicit trap or fallthrough." | Coverage and domain tests | Incomplete coverage must fail compilation; no implicit unreachable trap or fallthrough | not stated | 4
PATTERNS94 | "A wildcard closes coverage." | Coverage and domain tests | A wildcard arm may satisfy coverage obligation when otherwise valid | not stated | 4
PATTERNS95 | "Coverage is checked separately for each authored dispatch." | Coverage and domain tests | Each match or transition must have its own coverage proof | not stated | 4
PATTERNS96 | "A later dispatch, ordinary result, or explicit crash cannot cover alternatives missing from an earlier dispatch, even when lowering places their control edges next to it." | Coverage and domain tests | Missing arms in an earlier dispatch cannot be justified by later constructs or layout adjacency | not stated | 4
PATTERNS97 | "Finite case coverage includes ordinary and historical-lineage sums, pure case-union domains, and prior facts excluding impossible cases." | Coverage and domain tests | Case coverage analysis must account for listed sum kinds and facts that rule out cases | not stated | 4
PATTERNS98 | "Boolean coverage includes true/false and complete Boolean tuple alternatives; complementary equality/inequality guards over the same unchanged subjects cover their domain." | Coverage and domain tests | Boolean and complementary guard patterns must be recognized as covering their domains | not stated | 0
PATTERNS99 | "Other value/comparison ladders and predicate-domain guards require a wildcard unless their coverage is otherwise established by the specified judgment." | Coverage and domain tests | Non-Boolean ladders and predicate-domain guards need wildcard or established coverage judgment | not stated | 4
PATTERNS100 | "Default- or named-domain constraints may exclude a case when selecting it contradicts the subject's established facts, including exact type equalities." | Coverage and domain tests | Coverage may treat cases as dead when domain facts or equalities contradict selection | not stated | 8
PATTERNS101 | "Coverage consumes that evidence; it does not require unrestricted theorem search over case predicates." | Coverage and domain tests | Use available evidence for dead arms; do not require open-ended theorem proving | not stated | 8
PATTERNS102 | "If impossibility is not established, the case still needs coverage or an explicit proof discharging the dead arm." | Coverage and domain tests | Without impossibility proof, arm must remain covered or explicitly discharged | not stated | 8
PATTERNS103 | "Failure to construct or prove a case is not evidence that it cannot occur." | Coverage and domain tests | Must not treat failed construction or proof as proof of unreachable case for coverage | not stated | 8
PATTERNS104 | "An intentional Unit completion is explicit, for example `_ -> {}`." | Coverage and domain tests | Unit-completing arms must be explicit in source | at normal return | 0
PATTERNS105 | "A result-producing context still requires a compatible result." | Coverage and domain tests | Match in value context must still yield compatible type across arms including wildcard | at normal return | 1
PATTERNS106 | "A wildcard does not supply missing guarantees, discharge linear ownership, or satisfy a migration completeness theorem merely by matching unknown input." | Coverage and domain tests | Wildcard must not substitute for missing proof, linear discharge, or migration completeness | at the operation | 2
PATTERNS107 | "Executable domain tests require pure, finite runtime-checkable predicates and any routed provenance required by [domain membership](domains.md#refinement-and-executable-membership)." | Coverage and domain tests | Domain patterns used as executable tests must be pure, finite, checkable, with required provenance | at the operation | 5
PATTERNS108 | "A proof-visible proposition alone does not imply an executable test." | Coverage and domain tests | Proof-only propositions cannot serve as executable domain tests without executable predicates | at the operation | 8
PATTERNS109 | "Overlapping domain alternatives use first-match order;" | Coverage and domain tests | Overlapping domain arms must resolve by authored first-match order | at the operation | 5
PATTERNS110 | "unordered union proofs do not." | Coverage and domain tests | Unordered union proofs must not determine overlap resolution for domain arms | not stated | 8

## Ambiguous sentences

Sentence | Reading A | Reading B
--- | --- | ---
"Adding a field invalidates incomplete patterns." | Any structural pattern missing a newly added field becomes ill-typed or rejected at compile time on recompile. | Invalidation applies only in arm position without `..`, or only when the type is re-checked, while other positions might differ. |
"Prefer a local binding when only decomposition is intended." | Style guidance only; both forms must remain accepted. | The compiler or linter may warn or reject redundant match when `let` decomposition suffices. |
"owned decomposition keeps its existing rules inside the machine that performs it." | By-value projection types reject in signatures, but owned decomposition inside a machine body still follows pre-projection owned rules without using `Command::case` as value type. | Owned decomposition rules might differ between outer machine and helper depending on whether projection naming is used. |
"Coverage consumes that evidence; it does not require unrestricted theorem search over case predicates." | The compiler may use only facts already established in the typing context for dead-case exclusion. | "Consumes" might mean evidence is spent and cannot be reused elsewhere in the same proof, versus merely being available for the coverage check. |
"complementary equality/inequality guards over the same unchanged subjects cover their domain." | Pairs like `x == c` and `x != c` on the same subject jointly cover all values of that comparison domain. | Coverage applies only when subjects are literally the same binding without intervening mutation, versus syntactically the same expression. |
"Finite case coverage includes ordinary and historical-lineage sums, pure case-union domains, and prior facts excluding impossible cases." | The listed categories are examples of what the coverage algorithm must handle. | "Includes" might mean these are necessary and sufficient classes, versus a non-exhaustive list of supported analyses. |
"A wildcard does not supply missing guarantees, discharge linear ownership, or satisfy a migration completeness theorem merely by matching unknown input." | Wildcard arms still run their bodies under normal proof rules; they do not auto-prove obligations. | Wildcard might still permit compilation when other checks pass, but specific named theorems remain unproved—versus rejecting programs that rely on wildcard for those obligations. |
"Proof-visible proposition alone does not imply an executable test." | A domain guard in a pattern must lower to a runtime predicate to be a domain test arm. | Proof-visible facts might still contribute to coverage of impossible cases without being executable tests. |

## Terms used without definition in this chapter

- dispatch
- subject (pattern subject)
- arm / alternative
- program point
- compatible arm values
- control edge
- pattern vocabulary
- `some` package
- stable subject/version opening
- dependent fields
- witness (concrete witness)
- package opening rules
- runtime type test
- value patterns
- structural patterns
- domain patterns
- case patterns
- payload bindings
- tuple dispatch
- projected equality
- drift check
- saved subject
- mixed value
- common fields
- case-only component
- inline cases
- semantic declaration order
- stable schema numbers
- layout-policy order
- pure sum
- carrier-qualified paths
- case view
- static arguments
- storage identity
- default-domain predicates
- named-domain predicates
- case-dependent type equalities
- validity/authority evidence
- footprint rules
- affine cleanup
- linear-custody rules
- coverage (dispatch coverage)
- `Command::case` / `Data::case` (projection type)
- case owner
- contract-based repair
- case-conditional consequences
- GADT proof route
- historical-lineage sums
- pure case-union domains
- predicate-domain guards
- specified judgment (coverage)
- dead arm
- migration completeness theorem
- executable domain tests
- proof-visible proposition
- routed provenance
- unordered union proofs
- first-match order
- `DATA-DECOMPOSITION` (implementation tracker)
- `CASE-TYPE` (implementation tracker)

CHAPTER COMPLETE
