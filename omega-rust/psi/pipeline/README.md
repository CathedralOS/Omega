# Source frontend ownership

The [pipeline map](../../pipeline.md) connects the source stages to checked Psi
and Terminal publication. These stages own increasingly resolved source meaning;
source acquisition and package closure remain compiler/package responsibilities.
They do not select providers, physical storage, native ABI plans, or installed
authority.

## Lexing and parsing

Value-position `match` currently expands into arithmetic in
[`primary.rs`](tokens-to-syntax-trees/src/parser/expression/primary.rs).
This repeats subject/default nodes and computes nonselected arm terms;
it does not implement the specified single-evaluation, first-match selective
behavior for general values/effects. Replace that expansion with retained
dispatch before claiming general match support. Parse acceptance and numeric
examples are not coverage for the [dispatch contract](../../../wiki/spec/language/patterns.md).

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
logical paths to the exact loaded source. Package aliases do not rename nominal
identities or grant transitive selection authority. Static namespace calls retain
the authored `::` distinction from value-member `.` calls through parsing.

Nominal data references, scalar free-machine calls, and primitive literal
constants used in bodies have namespace coverage, including qualified Terminal
selection and independently executable constant artifacts. Constant substitution
uses exact module/package selection after lexical name assignment and retains
the selected declaration at the original use. This is not completion of the
[module/name contract](../../../wiki/spec/language/modules.md):
aggregate/type-scoped constant and template normalization, trait defaults,
operator homes, qualified constructors, and the remaining declaration forms
still need exact namespace-aware resolution. Module-owned aggregate/type-scoped
constants, generic templates, traits, conformances, domains, and operators
currently reject before their bare-name transforms; so do generic
carrier/argument collisions across module scopes.
Closed data applications select named constant indices through the shared
source-aware resolver before folding. Loader import bindings reach evaluation
probes too. The normalizer checks the declared carrier, retains exact declaration
and initializer custody at each argument, and rejoins its canonical value to the
final symbol. Shared instance derivations do not inherit a caller's occurrence
exposure; equal values may deduplicate without losing distinct selections.
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
paths before legacy materialization: a runtime-qualified root cannot acquire
a same-spelled static aggregate. The selection prepass retains declaration
custody without materializing aggregate values. Module-owned aggregates still
require namespace-aware initializer normalization and materialization.
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
Open templates, aggregate evaluation, comparisons between two wholly anonymous
numeric operands, constrained destinations, authored operators and module-owned domain
families remain outside this probe.
Domain indices retain the declared family's identity; equal results share canonical
type identity without discarding the original constant or operator occurrences.
Unrelated root constants and literal or binder-only
applications remain available. Later
syntax extensions cannot yet consume a retained base constant whose initializer
was discarded at the previous resolution boundary.
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
