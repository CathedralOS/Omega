# Source frontend ownership

The [pipeline map](../../pipeline.md) connects the source stages to checked Psi
and Terminal publication. These stages own increasingly resolved source meaning;
source acquisition and package closure remain compiler/package responsibilities.
They do not select providers, physical storage, native ABI plans, or installed
authority.

## Lexing and parsing

Value-position `match`, including a bare machine-tail expression, retains its
subject and ordered, source-linked arms through all source representations.
It is not a spelling for `transition`. Scalar computation dispatch saves the
subject once, evaluates patterns in order, and sends only the selected result
to the ordinary expression continuation. Source replay independently checks the
exact arm identities and coverage; it cannot treat an arbitrary last arm as a
default. [Terminal execution tests](checked-trees-to-lowered-psi/tests/value_dispatch.rs)
cover overlaps, call-argument composition, subject-once execution and skipped calls.

The [dispatch contract](../../../wiki/spec/language/patterns.md) is broader than
the current implementation. Wildcards and complete Boolean value alternatives
close coverage. Runtime scalar lowering currently supports Boolean/integer
subjects and Boolean/integer/float results; an anonymous-only numeric subject has no invented default width.
Structural/domain/payload patterns and ownership-bearing result/conditional
transfer joins remain explicit limitations. Checking still validates every arm's
type, including unreachable arms. Stable comparison facts belong to the exact
selected branch and are retired by writes to their inputs; they do not escape
the result join. The interpreter forwards an existing destination into only the
selected arm, preserving anonymous numeric landing without arithmetic desugaring.

Declared storage, parameter, return, typed-peer and exact-cast destinations flow
through result arms and array elements. Each complete anonymous arm keeps exact intermediates until
its own integral, in-range landing; even an unreachable fractional result rejects.
Width permission follows result edges, never the dispatch subject or patterns,
and independent call/constructor destinations retain their own custody. The
shared numeric destination collector drives width admission and fractional-origin
warnings for both ordinary arithmetic and Match results. A retained typed arm
fixes the result before an outer cast; otherwise a numeric result must be proven
anonymous before inheriting the cast destination. The shared result-type query
reads declarations, nested Match joins and builtin operation signatures after
checking selected meaning. Comparisons produce Boolean results, and shifts keep
their left operand's carrier. Typing retains policy-only builtin references so
computed arithmetic keeps Wrapping/Saturating/Trapping without inheriting input
range predicates or relying on an incidental result annotation. Cast policy is
retained until explicit erasure; incompatible Match result policies reject even
before an outer cast. A typed Match arm supplies its anonymous peers' landing
at their own result edges, even when no enclosing consumer supplies a destination.
Policy casts retain a shell over their exact target predicates in the existing
type table. Numeric Match joins preserve a shared exact reference, or weaken
ranges to a common carrier and policy; they cannot export the first arm's
predicate as a promise about another arm. An unknown result joins this numeric
path only when its expression is genuinely anonymous numeric. Unique direct
operators supply their exact declared builtin carrier/policy result, including
a concrete result independent of generic operand binders. Range analysis treats
the operation as a call with that result carrier, not builtin token arithmetic;
children still owe their own arithmetic obligations. Semantic-domain,
predicate-bearing and selected trait results requiring an instantiated reference
remain unresolved; declaration-local result subjects cannot become caller facts.

Semantic qualification casts retain a result-type handle over the exact authored
target. Ordinary domain normalization supplies aliases, indexed identities,
predicates and routes; the cast still owes membership independently. Match joins
compatible normalized domain results without treating different meanings as one
bare carrier. An outer erasing cast cannot repair incompatible arms: erasure
must be explicit in each arm. Static, unrouted scalar domains add no ownership
join; routed provenance and borrowed/owned results retain their separate fences.
Raw dependent range predicates are not merged by rendered spelling.

`cargo run -p omega -- --check --target macos_arm64 tests/omega/pass/expressions/match_domain_results/main.omg`
checks same-domain result selection. The corresponding negative
`tests/omega/fail/expressions/match_mixed_result_domains/main.omg` must reject
kilometres joined with miles before an outer bare-carrier cast. Predicate- and
route-free scalar casts now retain explicit `Qualification` computation nodes,
including their exact operands and normalized result types. Source replay checks
the authored selection, unique checked membership use, declaration obligations,
alias expansion and canonical indices; a pure payload node cannot replace the
cast. Terminal declarations retain the full scalar type: canonical domain sets
are shared across the reachable call closure, and explicit qualification binds
a fresh successor value without a runtime tag or operation. Every incoming edge,
ordinary call and result independently preserves that type.
`cargo run -p omega -- inspect-terminal --machine choose --target macos_arm64 tests/omega/pass/expressions/match_domain_results/main.omg`
publishes this fixture; `value_dispatch` replays both selections from canonical
bytes. Explicit same-carrier erasure uses that same computation node and a strict
subset qualification edge, including a bare destination. Its authored cast must
survive source replay even when later qualification restores the original tags.
Pure payload reconstruction cannot silently erase it. Qualified scalar calls and
Match results retain exact membership grants for compatible formals, including
closed indices. Predicate/routed evidence and erasure, plus shared structural
qualification transport, remain separate obligations, not bare-carrier fallbacks.

`cargo run -p omega -- inspect-terminal --machine choose --target macos_arm64 tests/omega/pass/expressions/explicit_scalar_tag_erasure/main.omg`
publishes an explicitly erased qualified Match/call result. The native regression
`cargo nextest run -p omega-native-differential-test --test scalar_array_results tag_erasure --no-fail-fast`
checks canonical replay, four-target publication, and matching-host execution.
No runtime tag is introduced: ordinary jumps carry fresh values, and Boolean
pattern equality uses the existing comparison and branch machinery.

`cargo run -p omega -- --check tests/omega/pass/expressions/declared_operator_match_result/main.omg`
checks a wider declared operator result joined with an anonymous numeric arm.
Its supplied checked realization does not yet make the nested operator a scalar
computation call: nonboundary declaration-to-body supply is undetermined under
[the direct-operator supply question](../../../OWNER_QUESTIONS.md#direct-operator-executable-supply).
Satisfaction alone cannot choose a body; later scalar computation and source
replay must retain the settled association.

Before asserted predicates may become result facts, each executing cast must prove
its asserted ranges from the live source environment. The arithmetic validator
owns that obligation; the expression scanner supplies selected reachability for
nested casts. Policy qualification cannot repair initial membership, and bounds
of composed operations describe each node's wrapped or saturated result, not its
unbounded mathematical intermediate. Float membership uses same-format finite,
non-NaN source and bound facts; unsupported membership remains a proof failure.
Matching full-width suffixed integer leaves retain their own
carrier through the same consumer-edge custody, including bitwise complement.

`cargo run -p omega -- inspect-terminal --machine choose --target macos_arm64 tests/omega/pass/expressions/anonymous_numeric_match_subject/main.omg`
exercises a Match whose subject and tested patterns are exact anonymous numbers.
Checking and independent source replay rederive first-match selection with the
existing exact-number evaluator. Only the chosen result becomes a computation;
no fixed-width subject or runtime rational carrier is invented. Even a leading
wildcard must not erase a call, typed value, or undefined subject. All-arm typing,
coverage and result-destination checks remain separate from selective execution.

`cargo run -p omega -- inspect-terminal --machine choose --target macos_arm64 tests/omega/pass/expressions/match_float_results/main.omg`
exercises f32 result selection inside ordinary calls. Match carries f32/f64
results through the same typed continuation as other scalar values, preserving
format and payload without arithmetic, conversion, or comparison of the result.
Boolean/integer subjects and exact anonymous selection are supported here;
floating-point subjects still need their independently retained comparison
meaning and lowering. A floating result does not authorize integer equality on
a floating subject.

[Lexing](source-files-to-tokens/src/lexer.rs) consumes loaded source records,
preserving source identity and byte spans. Numeric metadata and decoded literal
bytes are spelling-level payload, not typed values or proof facts. The closed
lexical profile uses explicit whitespace, ASCII identifiers and byte escapes;
host Unicode classification is not authority. Literal decoding copies source
bytes and expands fixed escapes, not a host-selected Unicode encoding.
[Lexical observations](source-files-to-tokens/src/observation.rs) retain source
and decoded bytes, token coordinates and diagnostics for differential comparison
with the product lexer. That comparison does not define the language.

[Parsing](tokens-to-syntax-trees/src/parser.rs) builds arena-backed syntax roots
and tables, retaining grammar, spans and literal structure without choosing
symbols, types, effects or proof evidence. The
[expression parser](tokens-to-syntax-trees/src/parser/expression.rs) uses an
explicit binary-operator stack and reversed unary prefixes; membership is a
separate grammar boundary, and postfix scratch is not retained across nested
primary parsing. Groups, aggregates, arguments and types still recurse. This is
not a stackless-parser claim; ordinary accepted input must not rely on an
enlarged host thread stack.

Preserve authored clause occurrences separately from normalized meaning:
memberless `reaches` differs from omission, and each authored `suspends` or
`blocks` keyword retains its span on machines and structural signatures.
Synthesized or omitted clauses must not acquire fictional source coordinates.
Exact requirement applications retain their authored lifetime/type/const/machine
arguments. Contracts and mathematical bundles retain logical source structure,
not an implicitly selected proof producer or runtime aggregate.

Anonymous `%` expressions remain trees for semantic formation checks; parsing
must not fold them into apparently valid integer leaves. Explicit non-returning
crashes stay distinct from ordinary terminal transitions. Retired grammar is
diagnosed rather than admitted as a compatibility syntax tree. Parser diagnostics
and the current language specification must agree.

## Value-generic staging

The settled [value-binder contract](../../../wiki/spec/language/generics.md#value-binders-and-const-requirements)
distinguishes runtime-capable `Count: u32` from static `const Count: u32`.
The current static-index evaluator and instance cache do not implement the new
runtime-value path. Preserve rejection of runtime subjects in const applications;
do not turn a failed constant evaluation into a guessed value or runtime fallback.

Implement distinct binder kinds, exact runtime-subject substitution, and ordinary
operand lowering with checked representation/custody under `RUNTIME-VALUE-GENERICS`
on the [execution board](../../../TASKS.md). Runtime witnesses are not canonical
static index bytes. Scope/name resolution, parameter modes, contract dependencies,
and result identity must survive through Terminal and artifact replay.

The [type-equation and range-matching rules](../../../wiki/spec/language/generics.md#structural-type-equations-and-inference)
add source type equality, endpoint extraction, and canonical interval matching.
Existing const substitution into `u64[0..=N]` does not establish reverse inference
from a supplied range. `generic_data/arguments.rs` still excludes range-qualified
arguments from its slug path, and constrained-shell substitution is not general
decomposition. `STRUCTURAL-GENERIC-MATCHING` tracks migration through source,
type identity, checking, evaluation, and artifact consumers; preserve unsupported
rejections until the corresponding representation and evidence are complete.

`FINITE-GENERIC-DISPATCH` implements the
[explicit family contract](../../../wiki/spec/language/generics.md#finite-specialization-boundary)
and [dynamic rows](../../../wiki/spec/terminal-psi/dynamic_dispatch.md#finite-generic-method-families).
Current nongeneric dynamic tables do not implement tuple expansion, runtime
family selection, or indexed result custody. Explicit source disjunctions, not
integer ranges or current callers, define the roster. Keep const rejection,
target coverage, and table completeness; no general reflection, implicit boxing,
or runtime-dependent inline storage is part of this work.

## Resolution and closed-instance normalization

Authored `module` paths establish namespace symbols without moving the ordered
root declaration slots. Semantic paths include the namespace; physical member
ownership and exact package provenance remain separate. Cross-file imports join
logical paths to the exact loaded source. A certified package alias can qualify
public declarations in other already-loaded sources of that same direct
dependency; each import still validates its own exact source. Package aliases do
not rename nominal identities or grant transitive selection authority. Static namespace calls retain
the authored `::` distinction from value-member `.` calls through parsing.

Record and case constructors retain their complete authored static name and
field evaluation order in syntax. Resolution selects either an exact `Data`
declaration or an actual `Variant` child of the selected owner; competing
interpretations reject. Path length does not choose the constructor kind.
Lookup preserves ambiguity separately from absence, with case eligibility
checked before precedence. Rejection diagnostics name the competing declarations
and retain the authored source's import context.
Resolved constructors retain the selected type, case and field symbols, the
complete constructor occurrence span and the case identifier span. The same
source-aware selection serves direct structural indices, so equal canonical
values retain each authored occurrence without bypassing package visibility or
direct-dependency authority. This follows the
[construction contract](../../../wiki/spec/language/data_and_literals.md#construction-and-case-identity)
and uses ordinary namespace resolution rather than a separate constructor scope.

Nominal data references, scalar free-machine calls, and literal constants used
in bodies have namespace coverage. Scalar constants include qualified Terminal
selection and independently executable artifacts; nominal aggregate body uses
have checked-source coverage. Constant substitution
uses exact module/package selection after lexical name assignment and retains
the selected declaration at the original use. This is not completion of the
[module/name contract](../../../wiki/spec/language/modules.md):
foreign/generic constant attachments, sum and specialized template normalization,
template attachments, trait defaults,
operator homes, package-prefixed bare case values, qualified case membership,
and the remaining declaration forms still need exact
namespace-aware resolution.
Later source extensions reuse the selected declaration's detached resolved
initializer. This keeps constructor selection in the declaring source and
deep-copies aggregate children at each use without re-reading the base source;
the [generated-source continuation](../../omega/compiler/compiler/generated_source.md)
retains its exercising compiler command.
Closed ordinary record applications select their exact module-owned template
and complete argument tuple. Nominal arguments retain declaration identity,
including nested applications and arrays; repeated qualification or narrow
imports of the same declaration share one instance. Each authored use retains
its own selection authority, and typed lowering independently checks its
application against the generated carrier's retained origin. Same-leaf templates
and arguments in different modules remain distinct.
The `package_compilation_inputs::module_generic_data` probes cover these
checked-source relationships and rejection of private or transitive-only access.
Generated-source normalization borrows the retained resolved predecessor for
nominal argument and domain selection. It does not synthesize retained templates
or expand the continuation's flat, root-owned nominal declaration boundary.
Unsupported scoped constants, module-owned sum and specialized templates,
template attachments, traits, conformances, domains, and operators still reject
before their bare-name transforms.
Closed data applications select named constant indices through the shared
source-aware resolver before folding. Loader import bindings reach evaluation
probes too. The normalizer checks the declared carrier, retains exact declaration
and initializer custody at each argument, and rejoins its canonical value to the
final symbol. Shared instance derivations do not inherit a caller's occurrence
exposure; equal values may deduplicate without losing distinct selections.
Parenthesized structural literal arguments use the same canonical encoder.
Their normalization retains the authored expression so constructor, case and
field selections survive atom replacement and syntax copying. A direct value
has no named-constant declaration; its index eligibility is checked separately
from named-constant copy and cleanup permission.
Concrete data fields and payloads also evaluate closed integer expressions in
data applications and root-owned domain families through a private typed probe.
Exact primitive carriers, builtin operator meaning and
package selection are checked before fixed-width arithmetic. Each argument retains
one canonical result, every selected constant occurrence and its builtin operator
occurrences. Nongeneric machine parameters, results, locals and casts resolve
named integer or Boolean indices and compound expressions containing named
constants in their original lexical scope before using the same probe. Boolean
destinations also route literal expressions through the probe; parsing retains
comparisons and logical operations instead of folding them as integers. Fixed-width
integer comparisons can produce Boolean indices. Runtime bindings cannot be
captured as constants. Public entry
signatures retain public exposure independently of body and internal-state uses.
Root scalar references also use resolved substitution, so locals and explicit
receiver fields can share a constant's spelling without changing its selection.
Named aggregate indices in those machine owners also resolve their lexical
paths before legacy materialization, including fixed-array destinations: a
runtime-qualified root cannot acquire a same-spelled static aggregate. Destination
syntax chooses the value route, never permission to skip lexical selection. The
selection prepass retains declaration custody without materializing aggregate
values. The two-file CLI check
`cargo run -p omega -- --check tests/omega/pass/modules/fixed_array_machine_indices/main.omg`
preserves static array identity across root and module machine owners; the same
command on `tests/omega/fail/modules/runtime_fixed_array_index/main.omg` rejects
a runtime qualifier. Module-owned fixed-array constants with integer/Boolean
literal leaves use the same exact declaration selector and structural encoder.
Every such declaration validates its extents and element carriers even when
private and unused; explicit integer landings cannot be erased during encoding.
The two-file CLI check
`cargo run -p omega -- --check tests/omega/pass/modules/module_array_constant_indices/main.omg`
covers same-leaf root/module constants with different canonical array values,
including `settings::Sizes::SIZE` and scalar `settings::Sizes::MAX`. Scoped constants
select an exact nongeneric data carrier in their declaring module and retain the authored carrier occurrence for
visibility checks. Relative attached names prefer their local module; narrow
imports expose only the exact selected leaf. Duplicate declarations, case
collisions and runtime qualifiers still reject. Scoped numeric/Boolean literals
also substitute into scalar bodies and computed machine indices; unused private
initializers still validate their declared carrier. Floating literals retain their
declared format and round directly to it. Public `f32`/`f64` literals encode
that format and its exact landed bits for declaration identity, alongside the
selected declaration's type and package owner. Signed infinities have exact
format-specific bits, and signed zeros remain distinct;
equivalent literals that round to the same value share the value encoding.
These declaration encodings do not admit floating generic/domain indices,
including an unused machine const binder. The two-file checked-source customer
is `tests/omega/pass/modules/public_float_constants/main.omg`; package review
retains the same encoding through serialization. Computed initializers and
public NaN identities still need their complete evaluation and explicit
representation contexts. Closed module-owned
record/case constants, including nested records and fixed arrays, use the existing
structural encoder after selecting each declared carrier and constructor in its
own source. Module-local nongeneric attachments use the same scope checks as
scalar constants. Receiving generic and domain arguments independently rejoin
the exact nominal carrier before accepting the canonical value; equal layouts
and encoded labels cannot grant identity. Fields encode in declaration order.
The `module_machine_indices::nominal` integration probes cover these checked-source
uses and hostile carrier, import and visibility controls. Module-owned templates,
foreign/generic attachments remain separate.
Closed nominal literal constants also substitute into ordinary bodies after
resolving each initializer in its declaring source. Constructor, case and field
selections survive each independent deep copy; a caller's same-spelled data
cannot replace their owners. Receiving locals, calls and results compare exact
nominal declarations, and selected field owners determine field obligations and
numeric landing. Root and module constants share this resolved substitution
path. Every retained named declaration, even unused and private, independently
requires a recursively copyable type with no cleanup; this includes inactive
case payloads and empty-array element types. Generic structural atom eligibility
remains a separate judgment and does not grant named-constant permission.
`cargo run -p omega -- --check tests/omega/pass/modules/nominal_constant_bodies/main.omg`
is the checked-source customer. The `package_compilation_inputs` nominal constant
body probes cover nested records, cases, independent uses, receiving identity,
privacy and direct-package exposure. The same customer with `inspect-terminal
--machine keep` still reports that the machine has no source-independent checked
scalar control plan; nominal aggregate execution needs that producer dependency.
Integer/Boolean array body
references use the same exact selector and deep-copy their literal trees per use.
Destination checks rejoin their declared dimensions and element identity, including
empty and nested-empty arrays; scalar leaf landings alone cannot retain that shape.
The CLI example above includes root, module-local and qualified array body uses.
Literal scalar indexing of these constants retains its declared element type,
bounds and selected indexing meaning through checking. The checked interpreter
evaluates the copied value; Terminal production selects its closed literal leaf
without creating constant storage. Nested scalar integer/Boolean projections execute
from independently decoded semantic/proof bytes. Integer/Boolean array literals and
array-valued constant projections construct actual owned Terminal payloads,
including nested and empty dimensions. Construction and return use the ordinary
ordered-operation path, including immutable array locals among scalar stores and
ordinary calls; replay checks their exact source statements, values, and types.
Runtime scalar operands reuse ordinary scalar expressions and computation graphs.
Elements finish in index order, preserving earlier values across mutating calls
and short-circuit control before the constructor commits its payload. Array-valued
constant projections still require closed unselected siblings; selection cannot
erase an evaluation or effect.
`cargo run -p omega -- inspect-terminal --machine selected_row tests/omega/pass/modules/module_array_constant_indices/main.omg`
publishes the material row; selecting `selected_empty_row` retains its exact empty
array type. Selecting `computed_row` exercises runtime scalar construction.
Array argument/result transport through calls and state transfers, and native
construction remain separate executable dependencies. Dynamic selectors, slicing and
explicitly borrowed projections still reject pending general value projection
and view-lifetime support; an ordinary typed-local copy already supports indexing.
Nominal aggregate body substitution remains separate. The `module_machine_indices` integration target covers distinct
module values, narrow imports, runtime shadowing, invalid unused declarations,
and scalar/array landing boundaries.
The comparisons preserve each operand carrier and use the shared typed integer
order operation; anonymous operands must land exactly in the selected peer carrier.
Boolean equality and inequality compose named constants and comparison results
without losing their separate authored selection occurrences.
Boolean `&&` and `||` evaluate only the selected right operand, while the probe
retains admission and authored custody for both operands. The two-file CLI check
`cargo run -p omega -- --check tests/omega/pass/modules/boolean_logic_indices/main.omg`
covers root and module selection under that schedule.
The two-file CLI check
`cargo run -p omega -- --check tests/omega/pass/modules/literal_boolean_indices/main.omg`
covers literal Boolean expressions and fixed-integer comparisons in both scopes.
Comparisons between two anonymous numeric operands consume exact rational values
without selecting an integer carrier or floating format. The shared evaluator
rejects undefined rational values even in unselected comparisons; typed operands
still require ordinary peer landing. The two-file CLI check
`cargo run -p omega -- --check tests/omega/pass/modules/rational_boolean_indices/main.omg`
covers fractional and decimal comparisons with their canonical Boolean results.
Scalar `match` indices retain their complete source trees for the
[typed evaluation owner](../semantics/build-time-evaluation/README.md), including
coverage and unselected-arm admission. Their source check is
`cargo run -p omega -- --check tests/omega/pass/modules/match_constant_indices/main.omg`.
Open templates, machine-computed nominal aggregate indices, constrained destinations, authored operators
and module-owned domain families remain outside this probe.
Domain indices retain the declared family's identity; equal results share canonical
type identity without discarding the original constant or operator occurrences.
Unrelated root constants and literal or binder-only
applications remain available.
Import loading still uses source-path candidates, including enclosing prefixes
for a module's declarations; it does
not scan or parse a package-wide source inventory to discover arbitrary files.

The settled [foreign-domain import contract](../../../wiki/spec/language/modules.md#import-scope-and-exposure)
requires file-local broad/narrow exposure, no transitive activation, and exact
declaring-owner-first attached paths. Loading a source cannot expose its sibling
domains. The current module-domain fence remains until namespace-aware operator
homes and declaration selection preserve those identities. The
[establishment contract](../../../wiki/spec/resources/authority.md#requirement-and-exact-machine-routes)
also admits exact machine targets; current requirement-only route normalization
needs a distinct exact-machine identity, not a conversion to a selected satisfier.
`MODULE-NAMESPACE-RESOLUTION` and `DOMAIN-ISSUER-ROUTES` on the
[execution board](../../../TASKS.md) track this work, not an unresolved owner decision.

The focused source/Terminal probes run from the repository root:

```sh
cargo run -p omega -- --check tests/omega/pass/modules/qualified_declarations/main.omg
cargo run -p omega -- inspect-terminal --machine combat::damage tests/omega/pass/modules/qualified_declarations/main.omg
cargo run -p omega -- --check tests/omega/pass/modules/qualified_constants/main.omg
cargo run -p omega -- inspect-terminal --machine combat::damage tests/omega/pass/modules/qualified_constants/main.omg
cargo run -p omega -- --check tests/omega/pass/modules/qualified_constant_indices/main.omg
cargo run -p omega -- --check tests/omega/pass/modules/compound_constant_indices/main.omg
cargo run -p omega -- --check tests/omega/pass/modules/domain_constant_indices/main.omg
cargo run -p omega -- --check tests/omega/pass/modules/machine_constant_indices/main.omg
cargo run -p omega -- --check tests/omega/pass/modules/boolean_machine_indices/main.omg
cargo run -p omega -- --check tests/omega/pass/modules/comparison_machine_indices/main.omg
cargo run -p omega -- --check tests/omega/pass/modules/boolean_equality_indices/main.omg
cargo run -p omega -- --check tests/omega/pass/modules/aggregate_machine_indices/main.omg
```

[Resolution](syntax-trees-to-symbol-resolved-trees/src/lib.rs) owns declaration
identity and exact lexical lookup, not type compatibility, borrow legality,
proof discharge or runtime support. Its private
[generic-data normalization](syntax-trees-to-symbol-resolved-trees/src/generic_data/mod.rs)
closes eligible data applications before stamping names. Templates, pending
substitutions and evaluation scratch are not new public representations.
Known-builtin anonymous remainder formation is checked before reduction; an
authored operator spelling makes that narrow pre-resolution check decline,
since it has no exact selected-operator authority.

Each rewritten application retains its original base/arguments at the exact use.
Copies remap these origins; they are not new authored declarations or roots.
Cloned attached methods retain the exact authored template and closed owner.
Missing or ambiguous templates cannot acquire derivation custody.
[Build-time evaluation](../semantics/build-time-evaluation/README.md) owns the
single-use pre/post-typing continuation and its ordered const/layout/wire work;
orchestration cannot recombine its retained rows or exchange its authority.

Receiver identity has two coordinates: the lexical storage root and the final
projected member. Preserve both. Lookup sees current-state parameters and only
the already-declared local prefix; self-initializers and later declarations
cannot select a same-spelled field, free machine or type. Projected lookup
consumes one declared collection layer per index and retains exact case-payload
and nominal-owner identity. Filter eligible owners before ordinary source
precedence; a shadowing type must not hide an inherited payload method.
Unresolved projected or call-result receivers cannot fall back to a free
same-named callable. Bounds, case reachability, index effects and receiver access
remain later obligations.

Operator contracts resolve in their own formal telescope, including domain-owned
operators. Nested conformance applications retain their own argument packs rather
than borrowing the enclosing telescope or inferring omitted arguments. Exact
requirement lifetime names settle binder membership/ordinals during typing.
Measures retain their own declaration and parameter symbols in full and seeded
resolution. A body name cannot become its parameter merely through matching
spelling or a missing symbol; ranking classification consumes the resolved binding.
Proof-only `zero_value<T>()` retains the resolved type graph without deciding
all-zero validity. Named-transition evidence stays separate from runtime arguments.

Signature-free requirement compatibility is checked before normalization consumes
authored paths: overloaded exact families receive one declaration diagnostic and
source-ordered use diagnostics. An explicit boundary requirement retains exact
package, attached owner/path, telescope, signature, visibility and contract;
bodylessness, reach or catalog membership cannot synthesize that declaration.

## Typing and source custody

[Typing](symbol-resolved-trees-to-typed-trees/src/lib.rs) owns type identity,
compatibility, signature and typed contract surfaces. Roots and tables preserve
enough information for checking without reverse-engineering syntax. Typed
references are not active loans; type-derived cleanup requirements are not a
drop schedule. Proof discharge, flow invalidation and concrete ABI placement
remain outside this stage.

Missing measure-body field selectors bind through the exact measure parameter's
declared nominal type. Existing nonzero selectors remain unchanged for checking;
typing does not replace conflicting identities with a same-spelled field.

[Call-result typing](symbol-resolved-trees-to-typed-trees/src/call_results.rs)
selects computed-receiver methods from the producer's exact declared return type
after receiver children lower, never from its body or a returned-place proof.
Preserve already-resolved pattern-bound call targets and root/member identity.
Binary-expression lowering keeps a thin recursive path in both resolution and
typing, rather than retaining large aggregate/call scratch at every binary node.

Typed reach and operational custody retain exact owner, keyword and target spans
independently of normalized semantic rows. Copies and specialization preserve
that separation. Nested evidence applications retain every non-lifetime slot;
an expected binder supplies a compatibility target, not omitted arguments.
Erased arrival/bundle subjects do not acquire runtime storage.

[Authored selections](typed-trees-to-checked-trees/authored_selections.md) owns
the shared capture/finalization contract. Capture source statement calls before
table rebuilding and recursively retain static declaration paths; unresolved
paths remain explicit late obligations. Partition trait-default copies by exact
conformance application for compiler joins, never canonical package identity.
Use-site exposure survives generic normalization: generated fields/signatures
must not invent public selections, and suppressing derived signatures requires
the exact template/closed-owner association with unchanged visibility and supply.
Original template contracts and body selections remain checked.

Resolution and typing preserve source-authored selections; checking settles late
calls/operators and inferred evidence. Public declaration contracts remain
interface exposure while executable bodies and internal states remain private.
Lexical locals and source-free synthesis do not receive fictional package owners.
See [generated continuation](../../omega/compiler/compiler/generated_source.md)
for append-only source execution and [checking](typed-trees-to-checked-trees/README.md)
for the proof, flow and ownership boundary.
