# Selected staged Delta compiler

`delta_compiler.gamma` is the canonical request entry of the selected
Gamma-authored Delta stage. It admits DCREQ, runs the shared compiler pipeline,
and publishes either the successful Gamma receipt or an owned failure
frame through the selected Beta-authored Gamma evaluator.
`delta_compiler.composed` binds the complete entry-plus-implementation bytes and
evaluator-tape identity under `GammaComposedV1`, not the entry file alone.

## Source organization

Start at `delta_compiler.gamma`, then follow `implementation/pipeline.gamma`.
The latter sequences source checking, catalogs, typing, profile validation,
complete Gamma lowering, body-height normalization, and emission. Concept-owned
members are grouped below it:

- `implementation/boundary/`: bounded request admission and DCOUT publication;
- `implementation/checking/`: source tokens, retained syntax and grammar roles,
  exact-name catalogs, declarations, expression types, and application schema;
- `implementation/representation/`: the expanded Gamma program, expressions,
  binding identities, and generated expression heights;
- `implementation/lowering/`: checked Delta declarations and expressions to
  that Gamma plan, including calls, constructors, arithmetic, and matches;
- `implementation/normalization/`: scoped Gamma body-height budgets and
  capture-safe extraction of over-height fragments into generated functions;
- `implementation/emission/`: generic Gamma serialization and the unchanged
  fixed byte helpers/profile adapters.

[`checking/syntax/storage.gamma`](implementation/checking/syntax/storage.gamma)
owns the cumulative syntax-byte provision shared by parsing and grammar.
Each syntax allocation group is admitted before construction, at the selected
Gamma pair size, while phase carriers and later compiler objects stay outside
that ledger. The [syntax guide](implementation/checking/syntax/README.md#syntax-storage)
defines exact groups, coordinates, and refusal fields.

Declaration resolution starts at
[`checking/declarations.gamma`](implementation/checking/declarations.gamma).
Its subordinate `checking/declarations/` members separate
[`functions.gamma`](implementation/checking/declarations/functions.gamma)
(parameters and signatures),
[`data.gamma`](implementation/checking/declarations/data.gamma)
(constructors and fields), and
[`types.gamma`](implementation/checking/declarations/types.gamma)
(retained type-node resolution).

Body typing starts at
[`checking/types.gamma`](implementation/checking/types.gamma), then follows
[`checking/types/`](implementation/checking/types/README.md) for identities,
expression dispatch, branches, bindings, calls, and matches. Continuation
frames belong to the expression dispatcher and each concept's payload owner.

Global collection starts at
[`checking/collection.gamma`](implementation/checking/collection.gamma).
[`checking/names/`](implementation/checking/names/README.md) separates
the private identity cursor, exact prefix navigation, and admitted insertion.
Global census and parameter cataloging share these construction cursors;
downstream phases receive ordinary completed tries and counted environments.
Declaration resolution also starts cursors from the completed census roots,
reusing final nullary metadata and replacing only unresolved typed payloads.

[`checking/environments.gamma`](implementation/checking/environments.gamma)
owns counted local environments and their shared active-row provision.
Parameter cataloging and body binding keep their role-specific conflict and
annotation order; saved environments carry names and counts together through
initializer, branch, argument, and match-arm continuations.

Lowering starts at
[`lowering/program.gamma`](implementation/lowering/program.gamma) and completes
every authored function body before publication. The resulting program is defined in
[`representation/gamma.gamma`](implementation/representation/gamma.gamma).
[`normalization/`](implementation/normalization/README.md) then extracts
over-height fragments while retaining evaluation order and binding identity.
[`emission/program.gamma`](implementation/emission/program.gamma) serializes
the resulting Gamma plan; it does not select Delta lowering rules.

`implementation/implementation.gamma.sources` selects all 62 shared members
with exact lengths, digests, and ordered identities. The byte-only source
materializer validates that closed inventory and prefixes the explicitly
selected entry. For the canonical entry, its application marker is therefore
the first declaration. Callers use the role registry's
`OMEGA_PATH_DELTA_COMPILER_SOURCES` and `OMEGA_PATH_DELTA_COMPILER_SOURCE`.
Reading the entrance alone does not reconstruct the compiler.

The separate `tests/delta/staged-compiler/development_driver.gamma` entry
invokes these same shared bytes as an unmarked raw-source transformer. It
exists only for frontend/lowering diagnostics and does not select a compiler
application profile. The canonical entry never detects raw source, guesses a
profile from its first byte, or falls back to that diagnostic entry.

## Implemented semantics

The current stage accepts the Gamma-shaped scalar core, immutable `Bytes`, and
finite data whose constructors carry any finite number of `Int`, `Bytes`, or
known nominal fields, plus exhaustive matches in authored arm order. It assigns
constructor tags in declaration order. Payload-bearing nominal values become immutable `(pair tag product)`
nodes whose products are right-nested pairs. Nullary constructors in a payload
type carry zero padding. Matches project the tag and product once and recover
binders with ordinary generated Gamma lets.

```text
(data Choice (Left) (Right))
```

becomes the scalar tag representation `Left = 0`, `Right = 1`. `Option` becomes
`Some 9 = (pair 1 9)` and `None = (pair 0 0)`. A recursive `List` constructor
with head and tail fields becomes `(pair tag (pair head tail))`. A match must contain
exactly one arm for every constructor, but arms may use any authored order and
nullary patterns may be bare or parenthesized. Generated local
names use `$m`, `$p`, and `$v` prefixes. `$` is outside Delta's identifier
alphabet but inside Gamma's, so generated binders cannot capture or be captured
by an authored Delta name.

Before tokenization or emission, the stage rejects every source byte except HT,
LF, CR, and printable ASCII, exactly matching Delta's textual envelope. A
complete lexical pass then admits identifiers, parentheses, arithmetic operator
tokens, and signed decimal literals. It checks an entire numeric spelling
before its range, so an overflowing prefix with a nondigit suffix is invalid
syntax, not an out-of-range literal. Comments retain CR, LF, and EOF boundaries.
Identifier admission finds the token end and validates its tail in one scan,
leaving whitespace, parentheses, and semicolons for the next scanner step.
Malformed tails still reject at the token start after full envelope admission;
nonidentifier spelling and integer-range checking retain their existing order.
Later name-role checks consume that lexical admission: they check initial
category and exact reserved spelling without scanning identifier tails again.
Only `Int` and `Bytes` can be reserved uppercase names; type positions admit
both builtins while nominal declaration positions exclude them. Prefixes such
as `IntX` remain ordinary identifiers. Nominal existence is still resolved only
after the complete global census.
After lexical admission, a total parser retains exact atom/list spans and ordered
children. An explicit frame stack handles nesting without recursive return
contexts. The complete balanced tree precedes grammar-role checking, so a later
unmatched delimiter wins over an earlier malformed declaration role. Missing
closing parentheses anchor at exact EOF; unmatched closing parentheses anchor
at their own byte. A separate iterative worklist checks declaration, binder,
type-name, expression, and pattern roles. Offending children anchor at their
start, and missing children at the containing closing parenthesis.
Pending grammar batches share parser-owned ordered child spines. Successful
roles tail-dispatch directly, without reversed child copies or per-role success
wrappers; only the complete declaration returns an outcome to the coordinator.
Grammar work entries also retain expression level. Function bodies start at 1;
expression children, including atoms and match arm bodies, advance by one.
Declarations, parameters, and patterns do not add levels. Before a level-1,025
expression's own grammar judgment, D30's selected 1,024-level `parse_depth`
profile produces an exact `Incomplete` frame: code 8, source coordinate at that
node's start, limit 1,024, requested 1,025. Complete balanced parsing remains
earlier, and the refusal is not a Delta syntax error.
Only after the complete structural grammar passes does collection consume the
retained program. Collection records every type,
constructor, and function identity, constructor counts, representation shape,
and source coordinates before resolving any declaration type. Duplicate
identities therefore precede declaration-type and body failures. Resolution
then checks fields and signatures against the complete catalogs, followed by
body checking. Census also provisions D30's 32,768 authored `function_rows`:
duplicate lookup precedes each fresh row, and a fresh 32,769th declaration
returns `Incomplete` resource code 4 at its name, before row insertion or any
declaration-type resolution. This count excludes metadata copies and generated
helpers; it is not Gamma's separate 4,096-function generated-program limit.
The census also counts fresh authored constructors across all data declarations.
D30 admits 65,536 rows; a fresh 65,537th name returns `Incomplete` resource
code 3 at that name, with limit/requested 65,536/65,537. Duplicate lookup
precedes provision, and a refused constructor allocates no row metadata.
Payload fields and the later declaration-resolution metadata rebuild do not
advance this count. Constructor tags still restart within each data owner;
the global resource count does not change their representation.
Type-row accounting starts at two for the builtin `Int` and `Bytes` identities
and advances once per fresh nominal declaration. The 65,536 total admits
65,534 nominal declarations, independent of the builtin identities' physical
representation outside the nominal trie. A duplicate type retains code 6;
a fresh over-capacity type returns `Incomplete` resource code 2 at its name,
with limit/requested 65,536/65,537, before its payload-summary traversal,
metadata allocation, or constructor collection. Type annotation occurrences
and resolved metadata copies do not consume new rows.
Declaration resolution visits declarations, constructors, and
fields in authored order. Each function parameter's conflict check precedes its
own type annotation, parameters precede the result type, and the whole
declaration phase precedes all bodies. Failures retain their exact source node
and propagate without converting evaluator failures into compiler judgments.
Entry existence and application schema are checked only after
the complete ordinary frontend succeeds. The entry lookup follows the four
exact `main` bytes in the resolved function trie, including its terminal
presence; a prefix or longer spelling is not an entry. It does not rescan the
source to rediscover that identity.
It requires all nonempty `data` declarations before one or more functions,
exactly one `main`, and unique type, constructor, and function declarations in
their separate namespaces. Exact source-byte names are retained in persistent
bytewise tries whose nodes store only present child edges, so this check has
neither hash collisions, absent-edge trees, nor repeated whole-source lookup.
Type and constructor names may still share a spelling, as required by Delta's
grammar-distinguished namespaces. Insertion builds a missing suffix from its
end toward its start with tail calls. Existing edges are traversed with a
counted ancestor spine, then rebuilt from the inserted terminal outward.
Exact terminal options distinguish a complete key from its prefixes; unchanged
children and prior roots remain persistent. Identifier length no longer adds
one Gamma return context per byte during insertion.

During the census, separate immutable cursors retain each namespace's current
prefix focus and ancestors across insertions. Exact seek rebuilds departed
prefixes, but a nearby name can reuse its shared prefix without rebuilding the
whole root. Duplicate lookup still precedes row provision, and insertion
occurs only after admission. Successful census completion finishes all three
ordinary roots. Parameter cataloging uses the same cursor helpers for local
names, then finishes the trie retained by its counted environment. Names remain
in authored order; there is no sorting pass,
lookahead past a refusal, mutation, or alternate downstream representation.
Empty child lists and absent trie options reuse their identical immutable
absence carrier rather than allocating a replacement on each miss.
Fresh suffixes likewise share a known-empty carrier across their absent
terminals and empty child lists, rather than allocating repeated empty pairs.

Declaration resolution retains the census constructor and function indexes
instead of rebuilding both from empty tries. It validates each raw row against
its declaration in authored order. A nullary constructor already carries the
final empty field-type spine, so no replacement is needed. Payload fields and
function signatures still resolve completely before their existing terminal
is replaced through a cursor. Only successful completion finishes the typed
roots; original census roots remain immutable custody evidence throughout.

The ancestor spine costs one additional immutable pair per traversed existing
edge. It shares prior nodes rather than copying names or introducing a dense
alphabet or hash table. Gamma's cumulative pair arena still bounds physical
construction, and its exhaustion is not yet a compiler-owned resource frame.
This change adds no identifier-length limit or new language/resource code and
does not establish complete compiler resource conformance.

The frontend validates identifier spelling at declarations, types, parameters,
local binders, constructor patterns, atoms, and application heads.
Keywords, `Int`, `Bytes`, and the five closed `bytes_*` builtin names cannot be
redeclared. Lexical admission scans decimal literals without overflow and admits
exactly `INT64_MIN..INT64_MAX`. Atom typing consumes that completed check;
bare `-` is a valid operator token but remains invalid as an expression atom.
Declaration resolution also rejects repeated parameter
names within a function before body checking begins.

A retained-node type-checking pass begins each function from the typed parameter
environment retained by the global catalog, then extends the immutable
exact-name trie for `let` bodies and individual match arms. It rejects unknown value atoms,
self-reference from a `let` initializer, and any parameter, `let`, or pattern
binder that duplicates an active local. Immutable roots give lexical pop
without mutation: sibling expressions, branches, and disjoint match arms may
reuse the same spelling.
The pass checks every currently emitted scalar, `Bytes`, and nominal constructor field,
pattern binder, call argument, `let` initializer, operator, conditional, match
arm, and declared result. Function and local names remain
grammar-distinguished namespaces.

Body checking consumes retained expression and pattern children rather than
reparsing their source structure. Explicit visit/resume continuations retain
pending children and environments without one Gamma call frame per source
nesting level. Ordered resolved constructor field types feed calls and pattern
binders without rescanning annotations. Each completed expression judgment
carries its type or the unchanged canonical failure; program checking succeeds
only after all bodies. No failed child produces a guessed type. The retained
source spans remain the coordinates for names, binders,
argument/type disagreement, and match coverage diagnostics.
The [boundary documentation](implementation/boundary/README.md#body-traversal-and-coordinates)
specifies the traversal order: head before expected arguments, body-let
annotation before conflict and outer initializer, and each complete match arm
before final coverage. These checks do not select a globally smallest offset.

The complete type-check pass finishes before the first output byte. Emission
therefore consumes that established preflight instead of revalidating data
declarations, parameter annotations, function results, or `let` annotations.
It copies admitted numeric spans without repeating their spelling/range checks.
It consumes retained expression and pattern children through explicit
visit/resume continuations. Source spans supply names and literal bytes, not
reconstructed subtree boundaries. Resume handlers receive the already-popped
depth and previous stack; pending closes and remaining children have separate
concept-owned payloads.
The raw-source development gate requires every rejected program to leave output
empty, including defects after otherwise emit-capable declarations. The
canonical entry instead publishes a complete DCOUT frame for owned failures;
request admission precedes source inspection, and no emitter runs before every
frontend and schema check succeeds.

Each match check retains its seen constructors in another immutable exact-name
trie. Same-owner validation plus duplicate rejection and exact constructor-count
agreement prove coverage without imposing declaration order. Emission compares
the cached scrutinee tag with each authored arm's actual tag and uses the final
exhaustive arm as the fallback, preserving the existing ordered receipts.

The global function trie carries each exact declaration's owner, arity, ordered
resolved parameter types, result type, typed parameter environment, and body
coordinate. Its preceding raw census rows retain the parsed declaration node.
Resolution checks the retained owner and name span, resolves the signature from
its child nodes, and advances through the counted declarations without rescanning
the body.
Application heads resolve through the checked table, including
forward and mutual calls, without reparsing the callee signature. Type and
constructor references likewise resolve through metadata catalogs rather than
rescanning the whole source. Every user call, operator, and `if` has an exact
argument count. Undeclared Gamma effects such as
`input`, `read`, and `pair` therefore cannot leak through as Delta calls; an
ordinary Delta function may still deliberately use one of those spellings after
declaring it. Every non-`main` function definition and call receives the
injective `__d_` Gamma prefix, preventing such a declaration from being
captured by Gamma's builtin dispatch. `main` alone retains the name required by
the evaluator.

Authored addition, subtraction, and multiplication lower to hygienic nested
Gamma lets that evaluate operands once, left-to-right, compute the wrapping
result, and trap if its sign relation or inverse-product check proves signed
overflow. Division and remainder use Gamma's already-identical zero-divisor
and `INT64_MIN / -1` traps. Compiler-generated tag arithmetic is structurally
bounded and does not acquire redundant runtime checks.

`Bytes` lowers to a private immutable Gamma-pair rope whose outer descriptor
stores the exact logical length. The five closed builtins are statically typed
and call generated helpers named with the capture-proof `$` prefix. Singleton
construction checks `0..255`; lookup checks the complete half-open range and
then traverses in proper tail position; concatenation computes and checks the
logical-length sum before allocating its new rope descriptor. Programs that
mention only the `Bytes` type receive no unused runtime helper.

`ConformanceBytesV1` now accepts canonical DCREQ framing, validates exact
`main : Bytes -> Bytes`, emits a marked nullary Gamma application adapter, and
owns empty/nonempty publication plus authored-trap, input-extent, and
output-extent statuses. Strict request admission publishes canonical DCOUT for
malformed framing, unknown profiles, and source-length refusal; see
[`implementation/boundary/README.md`](implementation/boundary/README.md).
Owned source failures now include forbidden source byte (code 3), invalid token
spelling and structural grammar (code 4), out-of-range integer literal (5),
duplicate type/constructor/function identity (6/7/8), local and pattern conflicts
(9/10), unknown types and names (11–14), type and arity disagreement (15/16),
duplicate and nonexhaustive match cases (17/18), missing `main` (19), and
application schema mismatch (20).
Missing `main` has no source coordinate; a schema mismatch
anchors at the entry name. Source-byte, total `type_rows`, authored
`function_rows` and `constructor_rows`, and expression `parse_depth` refusals
have owned resource frames; other compiler-owned resource accounting and
internal failure publication remain open. Lowering records expanded expression
heights, including generated wrappers. Normalization uses the selected Gamma
evaluator's 255-list body budget, reusing fitting subtrees and extracting whole
over-height fragments into helpers with free established values as arguments.
Fresh capture-parameter identities preserve lexical bindings; calls remain at
the original evaluation positions. Programs whose bodies already fit retain
their exact receipts. This does not guarantee full Gamma-profile admission
throughout Delta's 1,024-level profile: generated helpers, non-tail calls, and
immutable allocations still face separate function, context, and storage limits.
Underlying evaluator failures on those paths are not DCOUT. These
frontend diagnostics do not establish final edge closure.
Calls emitted in tail position remain in Gamma tail position through
`if`, `let`, and lowered `match`; the selected evaluator executes a 100,000-node
construction and traversal in bounded call context. Static acceptance of the
scalar/nominal slice is not full-language admission.

Run `sh tests/delta/staged-compiler/run.sh` for lowering and generated execution,
`sh tests/delta/normalization/run.sh` for body-height and capture behavior,
`sh tests/delta/request-boundary/run.sh` for exact request outcomes,
`sh tests/delta/resource-boundary/run.sh` for authored row-provision boundaries,
`sh tests/delta/frontend-boundary/run.sh` for exact frontend outcomes, and
`sh tests/gamma/composed-artifact.sh` for composed identity and publication.
The downgraded full compiler remains separate under
[`../bootstrap/concatenative-compiler/`](../bootstrap/concatenative-compiler/).

## Measurements

```text
3,229-line / 143,690-byte canonical entry plus shared Gamma implementation
7-line / 195-byte nullary-ADT Delta fixture
  -> 3-line / 165-byte Gamma receipt
  -> selected Gamma evaluation produces byte 9
7-line / 186-byte payload-ADT Delta fixture
  -> 3-line / 230-byte Gamma receipt
  -> selected Gamma evaluation produces byte 9
7-line / 187-byte recursive-ADT Delta fixture
  -> 3-line / 425-byte Gamma receipt
  -> selected Gamma evaluation produces byte 3
8-line / 221-byte two-field recursive List fixture
  -> 3-line / 502-byte Gamma receipt
  -> selected Gamma evaluation produces byte 9
24-line / 767-byte three-field recursive rope fixture
  -> 7-line / 1,404-byte Gamma receipt
  -> indexing produces byte 0x42; indexing empty traps
7-line / 277-byte proper-tail List fixture
  -> 4-line / 568-byte Gamma receipt
  -> constructs and traverses 100,000 nodes through if, let, and match
10-line / 379-byte typed Bytes fixture
  -> 10-line / 1,184-byte Gamma receipt
  -> all five builtins produce byte 0x42
5-line / 209-byte skewed Bytes fixture
  -> 9-line / 1,000-byte Gamma receipt
  -> 100,000-node lookup produces byte 0x5a in bounded call context
11-line / 397-byte forward/mutual nominal fixture
  -> 3-line / 956-byte byte-identical Gamma receipt
  -> all nullary, unary, and three-field constructor shapes produce byte 7
922-line / 34,804-byte current Epsilon declaration prefix plus scalar entry
  -> exact 21-byte scalar Gamma receipt within the evaluator watchdog
11,752-line / 598,889-byte current Epsilon source plus checking entry
  -> measured 697,820-byte Gamma receipt
  -> checking gate requires 48 exact judgments
3,001-function / 66,266-byte scale fixture
  -> 78,271-byte Gamma receipt
  -> selected Gamma evaluation produces byte 199
  -> transforms within the staged gate's unchanged 30-second watchdog
```
