# Psi product source

This sibling package owns the Omega-written, target-neutral Psi half of the
production compiler. `source/omega/` consumes it through the `psi` package
identity; Omega does not own this source tree. The current live slice contains
source/span and token representations, a UTF-8-framed lexer that stores only
semantic tokens, and a fail-closed whole-file parser for ordinary
`use path::member;` roots,
`[pub] data Name [<generic parameters>] [properties] [where facts] { ... }`
declarations whose generic parameters, `[copy]`/`[linear]` property lists and
members share their grammars with the other declarations, with fields
`[#N] name: Type;` and payload-free `case [#N] Name;` or structured
`case [#N] Name([#N] field: Type, ...);` members that may carry their own
`where` facts (`#N` is a stable schema identity), and
`[pub] [target] [boundary] machine [token] [Owner::]name [<generic
parameters>] [(parameters)] [-> Type] contracts clauses ({ ... } | ;)`
declarations, where a target name before `machine` scopes one body of a
family keyed by target, `boundary` marks a crossing, and a fixed operator
token from the closed vocabulary `+ - * / % == != < <= > >= [] [..]` binds
operator syntax to the declaration (a conformance member binds none). Each
`where machine Name [<generic parameters>] [(parameters)] [-> Type] clauses`
contract, or nominal `where machine Name satisfies Trait::requirement`,
gives a static machine binder its signature; the contracts precede the
other clauses, end at their `;` or at the next `where machine`, and form one
flat list whose binders resolution matches, since a contract may name a
binder of an earlier contract. The clauses are `reaches`, `invokes`,
`suspends;`, `blocks;`, `crashes`, `requires`/`ensures` facts separated by
`,` or `;`, `satisfies Trait[<arguments>]::requirement [as Label] [via
value]` exact requirement edges (a bare trait rejects: whole-trait
satisfaction is a named conformance), and `terminates [by subjects [->
View]];`, whose subjects are one expression or a parenthesized list and whose
view is a path or an argumented view call. Bodies hold entry statements and
then `state name [(parameters)] [-> Type] [requires fact]* { ... }` members. A
bodyless machine ends at `;`; a `;` that ends a contract fact, a
satisfaction, a termination or an operational clause belongs to the clause
only while the declaration continues after it with `{`, another clause or
another fact, and otherwise ends the declaration; a trait member's `;` before
`requires` ends the member, whose trait reads `requires Parent;` as its body
parent ([`parse/clause.omg`](parse/clause.omg)).
Statements are `let [mut] name: Type [= value];`,
`crash Trap;`/`crash Abort;`, `place = value;`, `value;`,
`transition [subject] { arms }` and a final unterminated result expression.
Arms are guards in a subjectless block and `_`, literal or case patterns
(`Path { binder, field as name, field as _, field: value }`, renaming or
waiving a binder) over a subject, each with a call target
`-> target(arguments)`, possibly acknowledged (`-> suspend block f(x)`), any
other expression as a terminal value (`-> (gap)`, `-> View { items: items }`),
or the Unit completion `-> {}`. Expressions
([`parse/expression.omg`](parse/expression.omg)) cover integer, float, string
and Boolean literals, names and
`::` paths (every keyword but `self` is a name), `.field`, `[index]`, calls,
static applications `path<arguments>(...)`, `as Type` casts,
`Path { field: value }` constructions (always inside brackets, never before an
unbracketed body), prefix `!`, `-`, `~`, `&`, `&mut` and the call
acknowledgements `suspend`, `block` and `suspend block`, the binary
operators in the reference grammar's two precedence tiers, memberships
`value in Domain {& Domain}` or `{| Domain}` between those tiers (one
connective per membership; an indexed domain `AtMost<8>` rejects), and
value-position `match subject { pattern -> value, ... }` whose arms are `_` or
value patterns and keep their own table, all on explicit stacks rather than
recursion. Every type position (fields, payload fields,
generic binders, parameters, return types, `let` types and casts) shares one
grammar in [`parse/types.omg`](parse/types.omg): references `&[mut] T`, arrays
`[T; N]`, slices `[T]` and named types, possibly `::` paths, with static
arguments `Name<T, 4>`, nested on an explicit frame stack, each form that
admits one taking `in Domain`, a name or a path such as a case's implied
domain `Type::Case`. A type's rows follow their children, so the
type fills one contiguous run of the table ending at its root. Direct fields
and case-payload fields retain separate bounded custody while mixed
declarations preserve exact authored field/case order and shared named
type-reference identity. Empty payload lists and trailing payload commas are
accepted. The remaining root kinds share those grammars:
`[pub] boundary data Name [<generic parameters>] [properties];` leaves its
representation to a crossing, with no members or facts;
`[pub] [boundary] trait Name [<generic parameters>] [: Parent {+ Parent}]
{where proposition Name(parameters);} { ... }` holds machine requirements and
body parents `requires Parent;`, and a transparent refinement `trait Name =
Base { machine (* | [Base::]requirement) clauses }` narrows its base's
`reaches`, `suspends`, `blocks` and `terminates`, each clause ending at its
own `;`; a
named conformance `[pub] Name [<generic parameters>] satisfies Trait<arguments>
{ ... }` holds checked machines and reference rows `Trait::requirement = Path;`
([`parse/traits.omg`](parse/traits.omg), [`parse/conformance.omg`](parse/conformance.omg));
`[pub] domain [<generic parameters>] Carrier::Name [<indices>] [(subject)]`
is a transparent alias `= A & B;` or takes `requires` facts and
`established by Path, ...` routes, closed by `;` or an empty `{ }`
([`parse/domain.omg`](parse/domain.omg)); `[pub] const [Owner::]NAME: Type =
value;` names a compile-time value ([`parse/constant.omg`](parse/constant.omg));
`[pub] proposition Name [<generic parameters>] [(parameters)]` ends with
`;`, `evidence Type;` or `= fact;` ([`parse/proposition.omg`](parse/proposition.omg));
and a named mathematical definition `[pub] let Name [<generic parameters>]
[(parameters)]: Type = term;`, or an admitted assumption `boundary let ...:
Type;` without a term ([`parse/definition.omg`](parse/definition.omg)).
Generic parameter lists also take `proposition Name` binders, whose
signatures come from `where proposition` contracts, and conformance evidence
binders `Name satisfies Trait<arguments>`. A name-led root is a target-scoped machine when `machine` or `boundary` follows
the name and a conformance when `satisfies` or `<` does; after a bodyless
declaration's `;`, `Name<...> satisfies` likewise begins a conformance. Trait and conformance
members are machine rows marked with their container, held as one contiguous
range per declaration; a trait member without a body is a requirement, and a
conformance member must have one. `::` name paths (domain routes and
constituents, both sides of a reference row) share one table
([`parse/path.omg`](parse/path.omg)). The retired `boundary requirement` and
`operator` introducers reject at their word: a boundary operation is a
requirement of a boundary trait, and a token binds to an ordinary
`machine + Name(...)` declaration. Unsupported roots, retired inline
discriminants (`case A = 1`), `retired #N;` members, multi-domain unions, the
retired bracketed scalar range suffix (`u32 [0..=9]`; ranges are
`where`/`requires`/`ensures` propositions, `scalar-range-contract-spelling`),
and the `if`, `loop`, `while` and `return` statements reject instead of
becoming a private bridge tree.

Name resolution follows parsing over the compilation's source set
([`source/source.omg`](source/source.omg)): each unit belongs to the module
its path names (`a::b`), is parsed into the shared syntax tables, and has its
position as its source id. A `use` must name a supplied module
([`resolve/imports.omg`](resolve/imports.omg)). A reference sees its own
module, whichever of the module's units declares, and each module its unit
imports; an import is not transitive, and two visible declarations of one
spelling reject as ambiguous. Every unit belongs to one package here, so `pub`
does not yet narrow an import. It then rejects builtin-shadowing names and
duplicate data names within a module, duplicate generic parameter names of any
kind within a declaration, and duplicate members: fields and cases share one
namespace; each case's payload fields share another with the common fields.
Stable identities ([`resolve/identities.omg`](resolve/identities.omg)) number
all or none of a declaration's direct fields, of its cases, and of each case's
payload fields, with distinct `u64` values compared by value. It
binds every type reference, data declarations' first, machine signatures'
second and body types (`let` types, cast targets and static arguments) third,
each in its declaration's scope (a machine's generic parameters cover its
states and bodies): an enclosing generic parameter by spelling (never a
lifetime), then a builtin type, then a visible data declaration. It also
binds every `in` qualifier to a
compiler-known domain (`Wrapping`, `Saturating`, `Trapping`, `Finite`). A type
binds row by row over its run: reference, array, slice and integer rows bind
by form, and named rows carry the names. Last, it rejects duplicate parameter
names within a signature, duplicate state names within a machine, and a
machine that repeats an earlier one of its module's owner, name, generic
parameter kinds and parameter types, compared run by run with generic
parameters by position (machines may otherwise overload). A bodyless machine
binds only its signature; whether it has a supply is checking's question.
The binding tables are indexed by the syntax table's type-reference handles.
Then every name and path in a machine body binds
([`resolve/names.omg`](resolve/names.omg)) in the language's resolution order:
an earlier `let` of the same body, the enclosing arm's pattern binders, the
body's own parameters (a state never sees the entry's), then the machine's
value binders (`Count: u32`, `const N: u64`). A callee names a machine binder
(`machine F`) or a machine, a call-shaped transition target a state of its
machine before either, a construction or case pattern a data declaration or
`Type::Case`, and a value path a case or an owned machine; a terminal value
target binds as any value, as do a value-position match's subject, value
patterns and values, and an unknown name rejects. A pattern field renamed
`field as name` binds as `name`, and a waived `field as _` binds nothing. A
membership `value in Type::Case` binds the case whose implied domain it
names; another domain rejects as unknown, and several domains at once as an
unsupported expression. These value bindings are indexed by expression
handle. Resolution binds `use`, `data` (a `boundary data` declaration names
an opaque type like any other) and untargeted, non-boundary machine roots;
every other root kind (a trait, conformance, domain, constant, proposition
or mathematical definition, or a target-scoped or boundary machine) rejects
at its root as an unsupported declaration before any binding, rather than
resolving as if its names were absent. A machine root rejects in the same
pass, at the form, when it binds an operator token, gives a binder a `where
machine` contract, carries a `satisfies` or `terminates` clause, or declares
a proposition or evidence binder ([`resolve/forms.omg`](resolve/forms.omg)):
each names declarations (traits, conformances, propositions) resolution
does not bind yet.

Typing follows resolution ([`typing/typer.omg`](typing/typer.omg)) and
records its results in [`types/types.omg`](types/types.omg): an interned
type table, where equal types share one handle and a type's children precede
it, and annotations indexed by syntax handles (a type per type-reference row,
expression and pattern binder; each call's selected machine or state target
and each `.field`'s declaration; each expression's adaptation and anonymous
value). Every written type interns first, row by row
([`typing/references.omg`](typing/references.omg)): a builtin with its `in`
qualifier (an arithmetic policy on a fixed integer, `Finite` on a float), a
data declaration applied to as many arguments as its non-lifetime generic
parameters, a reference, an array with its literal length, a slice, or a
generic parameter. Then each body's statements type in authored order
([`typing/statements.omg`](typing/statements.omg)), each expression tree in
table order, children before parents
([`typing/expressions.omg`](typing/expressions.omg)). A name takes its
binding's declared type. `.field` projects a data type's common field
through any number of references, its generic arguments substituted;
`[index]` selects an array's or slice's element. An unsuffixed integer
literal is anonymous ([`typing/literals.omg`](typing/literals.omg)): wholly
anonymous `+`, `-`, `*` and exact `/` fold over exact integers, an anonymous
`%` or fraction rejects, and the tree lands once at a destination it fits: a
declared local, a parameter, a field, a typed operand, a cast or a result. A
suffix types its literal at that carrier. Arithmetic and bitwise operators take
one fixed-integer or float type, policy included, so differing policies never
mix; integer comparisons relate exact values of any carriers; `==` also
compares Booleans, addresses and sums whose every case is payload-free. A call
selects the one machine of its resolved overload family, visible from the
call's unit, whose parameters admit its arguments
([`typing/calls.omg`](typing/calls.omg)); none or several reject. Admission
([`typing/admission.omg`](typing/admission.omg)) is equal types, a landing, a
value of type `Never`, a shared-reference parameter implicitly lending its
referent or accepting an exclusive loan, or a payload-free case of generic data
meeting any application. A construction names each of its record's or case's
fields once; a case pattern names a case of its subject's data and types its
binders by the payload; a transition target supplies its state's parameters,
and a terminal call or value meets the body's result (a state without its own
`-> T` takes its machine's). Floats, static applications, value-position
matches, memberships, calls to generic machines and equality needing a
declared conformance reject as unsupported rather than typing partially. Carrier facts beyond the qualifier (unbounded
`UInt`/`Int` storage, ranges) belong to checking.

The source closure is authored against the ordinary-Omega surface that the
Delta-produced compiler must eventually accept; that compiler is not yet
published.

This authoring constraint governs features used by the compiler source, not the
Omega programs the resulting compiler accepts. Standalone terminal-Psi
interpreters, proof explorers, and other tools remain outside this closure
unless the compiler executable imports them.

The maintained Rust implementation lives at `omega-rust/psi/`. It
remains a differential comparator and may continue in parallel; no Rust
implementation belongs in this product subtree.

Each phase has a gate package under `tests/psi/`, driven by
[`tests/product/gate.sh`](../../tests/product/gate.sh), the driver the
product compiler gates share.
[`tests/psi/parser/test-parser.sh`](../../tests/psi/parser/test-parser.sh)
compiles the gate-owned Omega harness once and runs the parser's
acceptance, rejection, capacity-edge, lexical-handoff, and determinism cases
against that one native artifact. Its Python helper only decodes and compares
versioned parser observations; it implements no compiler semantics. No lexical
wire format or Rust observer is required. Rust's lexer has its own unit tests;
neither implementation must reproduce the other's internal representation.
Set `OMEGA_CLI` to the exact freshly built compiler and `OMEGA_TARGET` to the
selected target profile. Acceptance evidence prints the CLI and Omega artifact
SHA-256 identities beside that target. The gate has no cached-artifact or ambient
`target/debug` lookup; focused iteration may invoke the Python decoder directly
with the explicit Omega executable path. The gate is its own package beside this
root: under it, the gate's lock and build output would be `psi` source. Delete
the gate only when an equal or stronger product-source gate subsumes every
retained failure class, and the harness only when an equal or stronger
semantic-free gate preserves all of its cases.

[`tests/psi/typing/test-typing.sh`](../../tests/psi/typing/test-typing.sh)
does the same for typing over the resolver gate's framed source sets; its
harness observes the type table, every expression's span, type, selection,
adaptation and anonymous value, and every binder's type, and exits 248
without an observation when resolution rejects. Its last case types the whole
psi package as one source set.

[`tests/psi/resolver/test-resolver.sh`](../../tests/psi/resolver/test-resolver.sh)
does the same for name resolution. Its input is a source set: a unit count,
then each unit's module path and source, `u32`-framed; the decoder frames a
single source as the root module. Its harness lexes and parses each unit,
resolves the set, and observes the diagnostic and every type, domain and value
binding. Lexical or parse rejection exits without an observation.

On macOS ARM64 the three gates build their harnesses natively from a fresh
CLI: all 453 parser cases, all 144 resolver cases and all 43 typing cases
pass. Each gate counts each distinct source it runs as one case. No cached
executable may stand in for that run. Linux and Windows hosts have run none of
them.

The lexer keeps its one canonical `Token` stream of semantic tokens
(whitespace and comments separate tokens but are not stored); the parser
observes it through a shared borrow forwarded along its state edges. There is
no `TokenObservation`, numeric token array, per-token handoff, raw parser
ordinal, or scalar tag/span cache. Structural parser serialization lives only
in the gate-owned Omega harness; the exact product entrypoint retains phase
driving and exit diagnostics. The same parser cases and structural
observations remain mandatory, and the Python decoder stays semantic-free. The
former 32-case lexical parity matrix and its numeric token protocol are
removed; these parser cases do not replace that lexical coverage. Chapter 1
now fixes **LEXICAL-PROFILE-V1**: ASCII identifiers, space/tab/CR/LF
whitespace, byte-preserving literal bodies, and no codepoint escapes or raw
strings. Both maintained lexers reject all retired XID, `\u{...}`, raw-string,
Unicode-whitespace, and raw quoted-newline spellings through the same profile
diagnostic; no compiler source may depend on them.

The parser entrance is [`parse/parser.omg`](parse/parser.omg): it owns `Parser`,
initialization, and whole-file root dispatch. [`parse/input.omg`](parse/input.omg)
owns bounded token selection and trivia traversal;
[`parse/data.omg`](parse/data.omg) owns data-declaration grammar.

## Retention inventory

| Retained child | Product role | Deletion or absorption condition |
| --- | --- | --- |
| `build.omg` | Declares the target-neutral `psi` package consumed by Omega's product build. | Delete only if ordinary package ownership replaces this root atomically. |
| `source/` | Owns bounded source bytes and coordinates shared by the lexer and parser. | Absorb when a replacement representation preserves every live source/coordinate discriminator. |
| `tokens/` | Owns the sole typed lexical token stream the lexer fills and the parser borrows. | Absorb only into a successor representation that preserves the exact typed vocabulary and coordinates without parallel token truth. |
| `syntax/` | Owns the bounded structural syntax retained by the current parser slice, including distinct direct-field and case-payload-field tables. | Absorb into a later Psi representation only with equivalent accepted/rejected observations. |
| `lex/` | Owns the source-to-token implementation for the closed ASCII syntax profile and byte-preserving literal payloads. | Absorb only into a successor that preserves the exact V1 profile, diagnostics, coordinates, and payload bytes. |
| `parse/` | Owns token-to-structural parsing over a token stream the lexer keeps and the parser borrows: whole-file roots (`parser.omg`), token traversal and grammar-step helpers (`input.omg`), the shared type, generic-parameter, clause and name-path grammars (`types.omg`, `generics.omg`, `clause.omg`, `path.omg`), data, machine (with its operator token and `where machine` contracts), trait (with refinements and `where proposition` contracts), conformance, domain, constant, proposition and mathematical-definition declarations (`data.omg`, `machine.omg`, `traits.omg`, `conformance.omg`, `domain.omg`, `constant.omg`, `proposition.omg`, `definition.omg`), body statements (`statement.omg`) and expressions (`expression.omg`). | Absorb the parser only into its canonical successor. |
| `resolution/` | Owns the name bindings that annotate the syntax tables by handle, and resolution diagnostics. | Absorb into a later Psi representation only with equivalent bindings and diagnostics. |
| `resolve/` | Owns name resolution from syntax and source bytes to the bindings, and the named rejection of machine forms it does not bind yet (`forms.omg`). | Absorb only into the canonical multi-source resolver. |
| `types/` | Owns the interned type table and the typing annotations indexed by syntax handles, and typing diagnostics. | Absorb into a later Psi representation only with equivalent types, selections and diagnostics. |
| `typing/` | Owns typing from resolved syntax to the type table and annotations. | Absorb only into the canonical checker that subsumes its judgments. |

Generated data belongs under the semantic phase that consumes it. No retained
lexical contract consumes the current Unicode identifier table, so it must be
deleted rather than moved into a generic `generated/` owner.
