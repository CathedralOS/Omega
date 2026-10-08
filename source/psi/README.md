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
`::` paths (every keyword but `self` is a name; a name keeps its identifier in
its own row, and only a path's members fill the shared member table),
`.field`, `[index]`, calls,
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
([`source/source.omg`](source/source.omg)): up to 64 packages, 256
requester-local dependency edges and 128 units, each unit belonging to a
package and to the module its path names there (`a::b`), parsed into the
shared syntax tables, with its position as its source id. Those tables are
whole-program, sized at about twice what the psi package fills: 4,096
roots, 1,024 uses, 2,048 machines, 16,384 parameters and statements, 8,192
states and transition arms, 32,768 type references and call arguments, and
131,072 expressions; resolution's and
typing's rows indexed by machine, type-reference and expression handles share
those bounds. A `use` path's first member selects a package
([`resolve/imports.omg`](resolve/imports.omg)): the toolchain mount
`omega::language::core` the toolchain's core package, one of the
requester's product dependency aliases that dependency, and the
requester's own name its own package; any other path is a module path of
the requester's own package. The rest must name a supplied module of the
selected package, and a first member spelling another package's name or a
build-scope alias, where the requester holds no such module, rejects as an
undeclared dependency. A reference sees its own module, whichever of the
module's units declares, and each module its unit imports, across packages
only `pub` declarations (a trait's or conformance's member by its
container's `pub`); an import is not transitive, one path in two packages
names two modules, and two visible declarations of one spelling reject as
ambiguous. Every root kind the parser accepts binds but
mathematical definitions (`let` roots) and transparent trait refinements,
which reject at their root as unsupported declarations before any binding,
as does a form whose bindings resolution does not record yet
([`resolve/forms.omg`](resolve/forms.omg)), at that form: a `where machine`
or `where proposition` contract, a proposition or evidence binder, or an
operator token anywhere but on an ordinary owned machine binding an
arithmetic or order token (a trait requirement's token, an unqualified
machine's, `==`, `!=`, `[]` and `[..]`), on any machine or declaration. An
ordinary owned machine's token binds at its semantic home, the data
declaration its owner names, which must belong to the machine's own
package; one token with the same parameters twice at one home rejects
([`resolve/operators.omg`](resolve/operators.omg)). A `satisfies` clause binds
its trait (a trait position), the trait's static arguments in the machine's
generic scope and the trait's member it names, recorded per clause in
`NameResolution::requirements`, and its `via` payload in the machine's
static scope, without its parameters; a trait reached through a module path
rejects as unsupported. A `terminates by` witness's subjects and view bind
in the body's parameter scope ([`resolve/clauses.omg`](resolve/clauses.omg)).
Data
declarations, traits and named conformances share one type namespace per
module, constants (by owner and name) and propositions their own; a later root
repeating an earlier one's path in its module rejects (as duplicate data when
both are data), and a type name spelling a builtin type, or a domain name a
compiler-known domain, rejects. It then rejects duplicate generic parameter
names of any kind within a declaration, duplicate proposition parameters, and
duplicate members: fields and cases share one namespace; each case's payload
fields share another with the common fields. Stable identities
([`resolve/identities.omg`](resolve/identities.omg)) number all or none of a
declaration's direct fields, of its cases, and of each case's payload fields,
with distinct `u64` values compared by value. Trait requirements and defaults
and conformance members are machines named through their container
(`Trait::requirement`, `Conformance::member`; [`resolve/containers.omg`](resolve/containers.omg)),
a `MachineOwner` per machine, and the container's generic parameters cover
the member's signature and body, the member's own shadowing them. Domains
([`resolve/domains.omg`](resolve/domains.omg)) bind their carriers and index
arguments in their own scope first, and a module declares one domain name over
one carrier type once. Every type reference then binds, each in its
declaration's scope: data declarations', then trait parents and satisfied
traits (which must name traits), constant types, proposition parameter and
evidence types, machine signatures (members included), then body types (`let`
types, cast targets and static arguments; a cast in a domain's, proposition's
or data declaration's fact takes that declaration's scope). A name binds an
enclosing generic parameter by spelling (never a lifetime), then a builtin
type, then the one visible data, trait or conformance declaration; a trait
binds only at the root of a trait position, a conformance only as a static
argument's root or an applied name's argument, and anything else there
rejects (`NotAType`, `NotATrait`). A type binds row by row over its run:
reference, array, slice and integer rows bind by form, and named rows carry
the names. An `in` qualifier binds a compiler-known domain (`Wrapping`,
`Saturating`, `Trapping`, `Finite`) by spelling, else the one visible domain of
that name whose carrier is the qualified type, a domain's own generic
parameters standing for any type; two such domains reject as ambiguous. Last,
it rejects duplicate parameter names within a signature, duplicate state names
within a machine, and a machine that repeats an earlier one of its module's
path (owner and name, a member's owner being its container), generic parameter
kinds and parameter types, compared run by run with generic parameters by
position (machines may otherwise overload); target-scoped bodies for distinct
targets form one family under one path, while a second body for one target or
an untargeted body beside them repeats it. A bodyless machine binds only its
signature; whether it has a supply is checking's question. The binding tables
are indexed by the syntax table's type-reference handles. Name paths bind
next: a reference row's left side to a trait requirement (`NotARequirement`
otherwise) and its right side to a machine, a domain's establishment routes to
a requirement or machine, and an alias's constituents to domains, bare ones
over the alias's own carrier. Then every name and path in a machine body or a
constant's value binds ([`resolve/names.omg`](resolve/names.omg)) in the
language's resolution order: an earlier `let` of the same body, the enclosing
arm's pattern binders, the body's own parameters (a state never sees the
entry's), then the machine's value binders (`Count: u32`, `const N: u64`), a
member's own before its container's, then a constant. A callee names a machine
binder (`machine F`), then a fellow member of the enclosing trait or
conformance, then a machine; a callee path `Owner::name` names the machine of
that path, so `Trait::requirement` and `Conformance::member`, or, for a
conformance declaring no such member, the machine its reference row or its
trait supplies for that requirement. A call-shaped transition target names a
state of its machine before any callee, a construction or case pattern a data
declaration or `Type::Case`, and a value path a case, an owned constant or an
owned machine; a terminal value target binds as any value, as do a
value-position match's subject, value patterns and values, and an unknown name
rejects. A pattern field renamed `field as name` binds as `name`, and a waived
`field as _` binds nothing. A membership `value in Type::Case` binds the case
whose implied domain it names; `value in Carrier::Name` binds the domain the
carrier type selects, and a bare `value in Name` the one visible domain of
that name, in the path bindings (a case and a domain of one path, or several
bare candidates, or several domains at once reject). These value bindings are
indexed by expression handle. A machine's `requires` and `ensures` facts and
a state's `requires` facts bind in their body's parameter scope, `result` in
an `ensures` fact naming the returned value unless a parameter spells it, and
a data declaration's `where` facts bind its fields bare
([`resolve/facts.omg`](resolve/facts.omg)). A case's own `where` facts,
domain predicates and proposition bodies bind no names here; which
requirement a member or row fills, row completeness and a domain's
establishment obligations are checking's.

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
`-> T` takes its machine's). An unsuffixed decimal literal is an anonymous
float: anonymous arithmetic involving one stays anonymous, and the tree lands
once, by one rounding, at an `f32` or `f64` destination, where an anonymous
integer may land too. Float arithmetic, negation and IEEE comparisons take
one format, so formats never mix, and `as` never converts between floats and
integers (named conversion machines do). A call to a generic machine binds
its generic parameters ([`typing/generics.omg`](typing/generics.omg)) from
explicit static arguments `path<arguments>(...)`, one per non-lifetime
parameter, and by unifying each parameter's type with its argument's type;
every parameter must be bound, each argument is admitted at its substituted
parameter type, and the result is substituted the same way. A construction
of generic data infers its arguments from its fields' values. Value-position
matches, memberships and equality needing a declared conformance reject as
unsupported rather than typing partially. Carrier facts beyond the qualifier
(unbounded `UInt`/`Int` storage, ranges) belong to checking. A constant is a
value of its declared type, and each constant's value types against that type
like a `let` initializer. A trait application's row takes no type. A call
selects among the callee's family by machine path; a trait requirement (which
runs only through a selected conformance), a generic conformance's member, a
target-scoped family's body (a realization's choice), a conformance named as
a static argument and a declared domain qualifier reject as unsupported.
Facts type too ([`typing/facts.omg`](typing/facts.omg)): each machine's
`requires` and `ensures` fact and each state's `requires` fact over its
parameters (`result` has the machine's result type) and each data
declaration's `where` fact over its fields is a Boolean. A case's own
`where` facts are not bound or typed yet. A machine that `satisfies` a
requirement must have the requirement's signature with the trait's type
parameters replaced by the clause's arguments, and its `via` payload and a
`terminates by` witness type like expressions
([`typing/clauses.omg`](typing/clauses.omg)); a requirement with generic
parameters of its own rejects as unsupported. An arithmetic or order
operator with a data operand selects, by its operands, the one visible
non-generic machine binding its token at an operand's data, admitting both
operands as a call admits arguments, and records the selection
([`typing/operators.omg`](typing/operators.omg)). Exact integer values and
their arithmetic, which literal folding uses, live in
[`integers/integers.omg`](integers/integers.omg).

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
without an observation when resolution rejects. It types the whole psi
package as one source set, and a source set filling the expression table.

[`tests/psi/resolver/test-resolver.sh`](../../tests/psi/resolver/test-resolver.sh)
does the same for name resolution. Its input is a source set framed like
the request's subject tables: the packages (a toolchain flag and a name),
the dependency edges (requester, scope, alias and target), then each unit's
package, module path and source, `u32`-framed; the decoder frames a single
source as the root module of one package. Its harness lexes and parses each
unit, resolves the set, and observes the diagnostic and every type, domain
and value binding. Lexical or parse rejection exits without an observation. Capacity
edges one unit's 65,536 tokens cannot reach (expressions, parameters, type
references and call arguments) run here as multi-unit source sets.

On macOS ARM64 the three gates build their harnesses natively from a fresh
CLI: all 456 parser cases, all 229 resolver cases and all 61 typing cases
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
| `integers/` | Owns exact integer values and arithmetic for literal folding. | Absorb only into a successor exact-arithmetic owner its consumers share. |

Generated data belongs under the semantic phase that consumes it. No retained
lexical contract consumes the current Unicode identifier table, so it must be
deleted rather than moved into a generic `generated/` owner.
