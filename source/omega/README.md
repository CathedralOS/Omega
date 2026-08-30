# Omega product compiler source

This package is the product root and Terminal-Psi consumer. The
sibling [`../psi/`](../psi/) package owns target-neutral source processing,
checking, and Terminal Psi; this package owns optimization, target realization,
artifact emission, and the product entrypoint.

The product compiler has two exact source implementations. `D` is written in
Delta; `C` is written in Omega using a deliberately conservative,
compositional subset of ordinary Omega.

```text
delta compiler + Delta source D → omega₀
omega₀ + Omega source C          → omega
```

`omega₀` may be conservatively generated and slow. It is already a full Omega
compiler because `D` implements the product language. The second build closes
the self-hosting edge and may improve the compiler executable; it does not add
language functionality.

Both compiler outputs are platform-independent Alpha tapes. Native target
realization belongs to this product phase only for user-program artifacts; it
does not turn any compiler rung into a native bootstrap artifact.

## Ownership

- [`../psi/`](../psi/) — target-neutral source, proof, and terminal semantics;
- this root — target realization, optimization, artifact emission, the
  Delta-written source closure `D`, and Omega-written source closure `C`;
- [`../omega-rust/`](../omega-rust/) — maintained Rust implementation and
  differential comparator, never bootstrap authority;
- [`../delta/`](../delta/) — final lower-rung compiler and direct first-build
  producer.

That source choice does not define a dialect or restrict programs the resulting
compiler accepts. Standalone viewers, interpreters, REPLs, and proof
explorers remain outside `C` unless the compiler executable imports them.

Implementation work is tracked in [`../../TASKS.md`](../../TASKS.md); bootstrap
closure is tracked in [`../../TASKS_BOOTSTRAP.md`](../../TASKS_BOOTSTRAP.md).

## Retention inventory

| Retained file | Canonical role | Deletion condition |
| --- | --- | --- |
| `build.omg`, `main.omg` | Current roots of Omega-written compiler closure `C`; the closure is incomplete but is extended in place. | Delete or replace only when an exact package-root ruling changes `C`; do not preserve alternate hosted roots. |
| `omega_compiler.delta` | Incomplete Delta-written compiler closure `D`; currently owns strict source-view UTF-8 framing, the complete source-neutral lexical scanner, invocation-local source-shaped parser slices, the final exact Alpha tape encoder, bind-once label/fixup ownership, structural replay before sealing, and no invented application boundary. | Extend in place as `D`; replace a completed component only atomically with an equally complete final Delta implementation. |

The four empty target declarations in `build.omg` are temporary compatibility
scaffolding, not product architecture. Delete them as soon as immutable target
activation/reach closure lands, and normalize `windows_x64` to
`windows_x86_64` in that same migration.

`omega_compiler.delta` (`D`) now exists but is intentionally incomplete, and
both descriptive compiler tapes remain absent. D18 fixes their logical sealed
Omega request as a resolved package subject plus a bound invocation, complete
deterministic build-visible snapshots, and the `OCOUT` compiler boundary. D25
fixes their `OCREQ` v1 byte framing, committed subject binding, structural graph
and closed-tree snapshot encoding, validation order, and shared 40-byte/
source-coordinate-extended 48-byte `OCOUT` realization. No raw-single-file
stdin convention may stand in for it. Each compiler derives and retains the
admitted typed build checkpoint internally, evaluates the selected root build
once, adds its generated source as a later one-way-visible stratum, and
continues ordinary compilation. Dependencies contribute durable generated-
source bundles rather than recursively executing their builds or retaining live
partial compiler checkpoints.

Its retained Alpha encoder targets `AlphaBootstrapV2` directly: at most
1,048,572 raw tape bytes, targets below that exact extent, and at most 116,508
nine-byte-dominated fixups. These are outer compiler-resource bounds, not Omega
language limits, and move only with the common seed/checker profile.

Delta cannot safely express a reusable validate-once source cursor: machines
and fields are public, while immutable views cannot be stored in data. `D`'s
parser therefore validates once and streams the same source through private
states of one canonical invocation. Its retained slices sequence empty,
trivia-only, ordinary `use path::member;`, basic `[pub] data`, and ordinary
machine roots whose current bodies contain neutral path calls and simple
assignments.
One mixed root table preserves authored use/data/machine order. Optionally
public ordinary machines retain an arbitrary name-like path, optional
parentheses containing a comma-separated state-parameter list, and a body from
the retained statement slice. An optional leading target selector is
retained as an exact span on the same machine row; selection and activation
remain later phases. A source-ordered machine-clause ledger retains exact
non-generic `satisfies Trait::requirement` bindings and nonempty `reaches`
ceilings over comma- or plus-separated service identifiers. A satisfying clause
may additionally own the exact `via`, `Binding`, and `CompilerIntrinsic` spans
for the payload-free closed external binding. That form terminates one bodyless
external-leaf machine with its canonical empty implicit entry. The ledger also
retains the private ranking witness `terminates by <path> -> <View::Path>;` as
separate source-ordered subject and view paths plus exact `by` and arrow spans;
it does not confuse that form with bare `terminates;`, which authors the public
eventual-terminal guarantee. Tuple or non-path subjects, an omitted or
argumented view, ranking ranges, the bare guarantee, generic satisfies
arguments, aliases, other external binding cases, and empty or
installation-bound reach rows remain incomplete. The parameter list
retains canonical optional `const` and leading
`mut`, consuming or borrowed `self`, and shared/mutable/write-only
binding-reference forms.
Non-receiver parameters retain `name: Type`. Non-receiver parameter types, data
fields, case-payload fields, and immediate machine return types share one engine
for bare named/Self/Unit, outer references, unqualified domains, inclusive
literal ranges, and nested fixed arrays and slices. References may retain an
exact explicit lifetime, and general `Self` uses the same `SelfType` base as
receivers. Every parameter row records its canonical const/mutable/self flags;
the owning implicit or explicit state owns the optional return node.

The retained nonempty-body slices include ordinary semicolon-terminated call
statements without acknowledgements, evidence arguments, or result discard,
and terminal expression statements over the retained primary path/literal slice,
plus a restricted canonical transition core.
Each call owns a flattened receiver-member span, an exact target
member, and a contiguous span of argument expression handles. Its current
value arguments are exact name/self paths, booleans, unsuffixed nonnegative
decimal integers, or string tokens represented by tagged expression nodes;
the same lane accepts shallow named struct literals over those primary field
values. Zero arguments and trailing commas are supported. Calls may also own a
separate nonempty `<...>` lane of path-only static machine arguments. Qualified
paths are retained exactly; const arguments, evidence projections, nested
static applications, lifetime arguments, and empty or trailing-comma static
lanes remain incomplete. Ordinary assignments retain
separate target and value handles. Their current target grammar is a self/name
place path or one path-indexed place with a path, decimal, or canonical range
over the retained path/decimal bound slice; current values are
self-member paths, qualified-name paths, booleans, unsuffixed nonnegative
decimal integer literals, or strings. Integer
spelling is retained by source span and is not evaluated during parsing;
strings retain the exact token span and scanner-proven decoded byte length
without a decoded-byte mirror. Assignment values, local initializers, ordinary
call arguments, and transition subjects share one bounded precedence reducer
over `||`, `&&`, equality, comparison, `+`, `-`, and `*`. Every operator
materializes a source-ordered binary node without evaluation or type guessing.
Parentheses delimit reduction frames and preserve their exact transient source
extent without manufacturing a group syntax node. Recursive logical `!` and
fixed-width integer complement `~` prefixes wrap a completed operand from the
admitted primary, group, and cast slice before that value enters the binary
frame; unary call operands, borrows, negative values, and contextual unary
forms remain incomplete.
Shallow struct literals
accept an exact one-member record or two-member case type path and a comma-
separated named field list with an optional trailing comma. Canonical adjacent
fields without commas, nested struct literals, and richer field expressions
remain incomplete until the shared postfix and field-expression paths admit
them. The implicit entry state and every
authored `state` own independent parameter, return-type, and contiguous mixed-
statement spans. A retained transition has either no subject or one precedence-
correct subject over path, boolean, and unsuffixed nonnegative decimal integer
primaries, and expands each
boolean, integer, or wildcard arm into an ordinary statement targeting one
named state. Arms also retain bare name paths and qualified two-member case
paths with optional braces. Braced case patterns own a contiguous field ledger
over shorthand bindings and fixed path-expression matches, preserving the
distinction between `Type::Case` and `Type::Case {}` plus exact binding and
field spelling for later resolution. Current named targets own a contiguous
argument-expression span
over self/name/member paths, booleans, unsuffixed nonnegative decimal integers,
strings, shallow struct literals, and path bases indexed by a path or decimal
integer. An index may instead use any canonical bound-presence/inclusivity
shape (`..`, `..end`, `..=end`, `start..`, `start..end`, or `start..=end`)
over those retained bounds. It is represented by a separate range expression
rather than folded into the indexed node. Target argument lists retain an
optional trailing comma. A
subjectless block retains absence explicitly and does not manufacture a unit
expression. Computed or multiple subjects, record/tuple patterns, renamed or
waived fields, non-path fixed values, proof selectors, richer guards, richer
target arguments,
terminal/value/self targets, continuations, and `match` remain implementation-
incomplete. Richer expressions and statements likewise stop the root as
incomplete, and no body is skipped as opaque text.
Local-data statements retain canonical `let [mut] name: Type [= expression];`
syntax in a dedicated row. `mut` is contextual, so `let mut: T;` still binds an
immutable local named `mut`. Locals share the borrow-capable type engine and the
same bounded expression reducer as assignments, including one path-indexed
expression with a path or decimal index and the same canonical range shapes;
absent initializers are represented by an explicit presence bit rather than a
valid-looking handle. Ordinary call
initializers share the statement-call target, static-argument, runtime-argument,
and delimiter parser but own a distinct expression row with an explicit
optional receiver handle. Call arguments retain the existing primary slice and
reuse the same reducer.
Path-valued casts reuse the same expression and type engines in assignments,
locals, ordinary call arguments, and transition-target arguments. They retain
the value and target-type handles, exact cast span, and an optional single-name
`in Domain` suffix. Ordinary assignment, local, and call-argument casts may
also wrap a completed grouped primary and then continue through the shared
binary tail. Richer postfix values, recasts, domain arguments, arbitrary bound
expressions, nested/chained indexing, and other richer initializers remain
implementation-incomplete.
One terminal expression statement may close a state directly. Its current
values are a self/name/member path, boolean, unsuffixed nonnegative decimal
integer, string, or one path-indexed expression with a path, decimal, or
canonical range index. The statement points directly to the shared
expression node; indexing has no assignment-only syntax representation.

Each completed machine owns zero or one implicit entry followed by its explicit
states in source order, matching the canonical parser. Parameters, a return,
implicit statements, or an otherwise empty machine require the implicit entry;
an explicit-state-only machine without those forms does not manufacture one. A
free machine uses the generated `entry` identity, while an attached machine
names its entry with the final authored declaration-path member. Machine,
termination-witness, and domain paths share the general path-member arena, but
a machine snapshots its
path extent before parameter types can append domain members. A trailing
parameter comma rejects as malformed. Constrained slice elements and the
`Slice<T>` spelling, return types placed after clauses, generics, remaining
machine-clause forms, state arrival contracts, and `boundary` forms; target
declarations and public target-scoped combinations, other public roots,
other bodyless declarations, and other body forms
remain incomplete. The parser never skips a body as opaque
syntax. In the current 73-root `C` closure, all 113
machine-header parameter occurrences, all 73 root parameter lists, and all 73
root headers are representable. All 73 roots are complete: all 57 bodies that
reach parsing, plus sixteen target-scoped bodyless external leaves. The bodyful
set includes
four initial call-only roots, seven roots using the retained assignment slice,
four target-provider roots using path-only static call arguments, the string-argument
`psi` package build root, `Lexer::initialize`, the canonical Omega package build
root, `Lexer::{is_whitespace,push_decoded}` through explicit states and
transitions, and `ConsoleNativeProvider::{write,write_line}` through exact
satisfies and reach clauses, plus `Lexer::tokenize` through the same ordinary
call/assignment/transition statement grammar and `Lexer::emit_punctuation`
through retained addition. `Lexer::is_identifier_start`,
`Lexer::is_identifier_continue`, and
`Lexer::hex_digit_value` complete through precedence-aware transition subjects
and subtraction. `SourceUnit::append` and `TokenStream::push_decoded` complete
through retained indexed assignment places. `SourceUnit::byte_or_nul` completes
through the same indexed expression in terminal value position, and
`TokenStream::push` completes once its indexed place and named transition-target
argument are both retained. `Lexer::{retain_token,classify_keyword,
lex_identifier,lex_whitespace,consume_suffix}` complete through canonical local
bindings and the shared additive initializer path. `Lexer::{lex_line_comment,
lex_block_comment,copy_source_to_decoded,is_raw_string_candidate}` complete
through retained call-valued local initializers.
`Lexer::{validate_utf8,dot_starts_float,reject_raw_string_candidate}` complete
through retained additive ordinary call arguments; `Lexer::lex_next` advances
through the same syntax. Path-valued casts complete
`Lexer::{consume_digits,lex_punctuation,lex_next}` and
`Parser::load_current`. Reusing the indexed-expression builder for local
initializers completes `Lexer::span_equals`; the three Parser roots that use the
same form advance to qualified token-pattern guards. Retaining qualified case
paths, shorthand bindings, and fixed path matches then completes `Main::main`,
`Lexer::digit_in_base`, and `Parser::{parse_data,skip_trivia,parse_roots}`. The
shared grouping and multiplicative reducer then completes
`Lexer::{decode_at,lex_cooked_string}`. Recursive logical-not retention completes
`Lexer::lex_number`. The sixteen external leaves are the four
`ConsoleNativeProvider::{read_line,read_byte,write_byte,exit_process}`
realizations for each selected target. `console_write_bytes` completes through
shared indexed transition-target arguments: `bytes[0]` owns an integer index,
while `bytes[1..]` owns an open-ended range index. Closing the current root
census does not imply that the parser implements unexercised Omega syntax or
that `D` has semantic, lowering, or emission closure.

Data syntax retains an optional `[copy]` property, bare named fields,
payload-free cases, contextual `case: Type` fields, structured case payloads
over the same bare named type leaf, one unqualified `Base in Domain`
constraint, one inclusive unsuffixed decimal-literal range
`Base [minimum..=maximum]`, recursively nested fixed arrays `[Type; length]`
over bare named leaves with the same unsuffixed decimal length spelling and an
optional outer domain, optional final member/case semicolons, mixed field/case
order, and relative spans in separate live-prefix tables. A case reaches its
contiguous payload-field span in a separate arena; direct and payload fields
share one binding control path. Type references are postorder tagged nodes: a
constrained root points to its base and to one source-shaped constraint, while
an outer Reference points backward to its complete referee tree and retains
shared/mutable/write-only access plus an optional exact lifetime span.
FixedArray and Slice nodes point backward to their element; FixedArray also
retains the exact length span. SelfType and Unit need no payload.
Domain constraints point into the general path arena; literal ranges and array
lengths retain exact spans without interpreting their values. Bracket syntax
uses a bounded invocation-local frame stack and emits named, array, and slice
nodes in postorder, so every child index points backward.
Compact kind/index ledgers reach the use/data/machine rows and field/case child
spans instead of duplicating coordinates. Qualified, indexed, intersected,
combined, exclusive, expression-bound, or multiple constraints; constrained
slice elements, the `Slice<T>` spelling, generic types, rich array elements or
lengths; numbered identities;
field relevance; other public roots; and every other unimplemented valid form
stop as implementation-incomplete rather than becoming false Omega rejections.

The provisional backing tables hold 4,096 root/use/data/machine/state rows and
16,384 path-member/data-member/direct-field/payload-field/case/machine-parameter/
machine-clause/type-node/constraint/statement/call-statement/call-expression/
cast-expression/assignment/local-data/transition/expression/binary-expression/
unary-expression/
indexed-expression/
argument/transition-pattern-field/static-machine-
argument/struct-literal/struct-field rows,
plus 128 scratch type-array frames and 128 expression values, binary operators,
unary prefixes, and group boundaries.
Only rows below their corresponding count may be inspected after `Complete`;
every other status may leave unowned partial prefixes and authorizes no
syntax-tree consumer. A repeated invocation invalidates old rows by resetting
every count. Root capacity dominates use/data/machine capacity, while `States`
becomes independently exhaustible as soon as one machine may own multiple
states. Data-member capacity dominates direct-field/case capacity. Fields,
parameters, and state returns share the type-node table, making `TypeNodes`
independently exhaustible. Its equal ceiling dominates payload-field,
machine-parameter, and constraint capacity in
the current slice because every retained row of those kinds owns at least one
type node.
Import/domain paths, call receivers, and path expressions share the
independently exhaustible path-member arena after each machine snapshots its
declaration path. Every retained call or transition-target argument owns one
expression row, so `Expressions` dominates the equal argument table. Every
call, assignment, local-data statement, or
transition-arm row owns one statement, so `Statements` dominates all equal
statement-variant tables. Every local owns at least one type node, so
`TypeNodes` also dominates the equal local-data table. Assignments own two
expressions, making the
expression table independently exhaustible. Every retained struct field owns
one value expression
and every struct literal owns its expression node, so `Expressions` dominates
both equal struct tables. Every static machine argument owns at least one
same-capacity path-member row, so `PathMembers` dominates that table as well.
Every retained machine clause owns at least one path member, so `PathMembers`
also dominates the equal clause ledger.
Every retained transition-pattern field owns one path member for its spelled
field, so `PathMembers` also dominates the equal pattern-field ledger even when
a fixed match adds its own expression path.
Every binary row owns its expression node, so `Expressions` dominates that
equal table as well.
Every unary row likewise owns its expression node, so `Expressions` dominates
that equal table as well.
Every indexed row likewise owns its expression node, so it introduces no new
resource distinction.
Every range row likewise owns its expression node, so `Expressions` dominates
the equal range table.
Every call-expression row likewise owns its expression node, and every retained
receiver is another expression node, so `Expressions` dominates the equal call-
expression table.
Every cast-expression row likewise owns its expression node, so `Expressions`
dominates that equal table.
Every terminal expression statement points directly to an existing expression
handle and adds no variant table; the equal statement and expression ceilings
remain independently exhaustible across the other retained forms.
The meaningful
resource distinctions are therefore `Roots`, `States`, `PathMembers`,
`DataMembers`, `TypeNodes`, `TypeDepth`, `ExpressionDepth`, `Statements`, and
`Expressions`.
These are private compiler budgets to profile against the real compiler
closure, not Omega source limits; exhaustion is retained for the future outer
`Incomplete` mapping.

No source identity, package alias, token ledger, decoded mirror, or transferable
preflight fact is retained. D18 binds each relative tree to a package-owned
source unit and fixes public diagnostic/outcome framing at the outer compiler
edge. A
public validate/advance split would be false authority, while revalidating the
whole view at every token would be quadratic; neither belongs in the compiler.
