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
Compound generic expressions and domain constant indices still retain their
existing selection fences. Unrelated root constants and literal or binder-only
applications remain available. Later
syntax extensions cannot yet consume a retained base constant whose initializer
was discarded at the previous resolution boundary.
Import loading still uses source-path candidates, including enclosing prefixes
for a module's declarations; it does
not scan or parse a package-wide source inventory to discover arbitrary files.

The focused source/Terminal probes run from the repository root:

```sh
cargo run -p omega -- --check tests/omega/pass/modules/qualified_declarations/main.omg
cargo run -p omega -- inspect-terminal --machine combat::damage tests/omega/pass/modules/qualified_declarations/main.omg
cargo run -p omega -- --check tests/omega/pass/modules/qualified_constants/main.omg
cargo run -p omega -- inspect-terminal --machine combat::damage tests/omega/pass/modules/qualified_constants/main.omg
cargo run -p omega -- --check tests/omega/pass/modules/qualified_constant_indices/main.omg
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
