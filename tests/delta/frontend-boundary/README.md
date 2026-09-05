# Delta frontend-boundary gate

Run `sh tests/delta/frontend-boundary/run.sh` from the repository root. The gate
materializes the complete canonical Gamma-authored Delta compiler through its
role manifest and runs it with the selected Beta-authored Gamma evaluator.
Python frames requests, invokes those source-owned stages, and compares exact
observations. It neither parses Delta nor selects diagnostic reasons or source
coordinates.

The exact 40-byte DCOUT controls cover the frontend phases:

- Source-byte rejection uses code 3 and Delta-source coordinate space 1. Invalid
  bytes, including bytes inside comments and a Unicode BOM, precede syntax and
  global collection. The first invalid byte wins even when a duplicate appears
  earlier in source.
- Lexical rejection distinguishes malformed tokens (code 4) from complete
  decimal integer tokens outside signed 64-bit range (code 5), at the token's
  first byte. A suffix after an overflowing digit prefix is still malformed.
  The first lexical defect wins before global collection, declaration types,
  and entry schema checking; even a later forbidden source byte precedes it.
- Balanced-tree parsing precedes grammar-role checking. Unmatched closing
  parentheses anchor at that byte; an unfinished list anchors at exact EOF.
  A later unmatched delimiter therefore precedes an earlier malformed name or
  declaration role. Empty source and a data-only program reject at EOF because
  the program requires at least one function.
- Structural grammar uses code 4 for declaration order, data and constructor
  shape, parameter lists and pairs, name/type roles, function body count,
  `if`/`let` shape, match arms and patterns, and expression heads/atoms.
  An offending child anchors at its first byte; a missing child anchors at the
  containing closing parenthesis. A 1,000-level nested-call fixture reaches the
  exact inner bare-minus atom through both iterative traversals. A later role
  defect precedes earlier duplicate collection or unknown declaration types.
  [`name_roles.py`](name_roles.py) checks malformed identifier tails before
  role checking, exact `Int`/`Bytes` exclusions in nominal declarations, and
  reserved value names. Existing exact `if` controls are reused. Accepted
  controls distinguish `IntX`, `Bytes_`, `if_`, and `bytes_get_extra` from their
  reserved prefixes and exercise underscore function, parameter, and local names.
  Identifier scanning also stops at EOF, adjacent parentheses, and an immediate
  semicolon introducing LF/CR/CRLF comments. A later forbidden byte still
  outranks an earlier malformed identifier; the existing oversized numeric
  suffix control continues to require spelling code 4 rather than range code 5.
- Global collection rejects later duplicate types, constructors, and functions
  with codes 6, 7, and 8 at their exact declaration names. Unknown constructor
  and signature types and body defects do not preempt this earlier phase.
  Duplicate `main` is code 8, not a selected-profile schema failure.
- Declaration resolution reports repeated parameter names with code 9 at the
  later name and unknown field/signature types with code 11 at the type name.
  The whole global census precedes this phase. Declarations, constructors, and
  fields are visited in authored order; each parameter's conflict precedes its
  own annotation, parameters precede results, and all declarations precede all
  bodies. An unknown earlier annotation can therefore precede a later parameter
  conflict, while a later declaration defect precedes an earlier body defect.
- After frontend acceptance, missing `main` is code 19 in coordinate space 0
  at coordinate zero; `mai` and `main_suffix` do not supply that exact entry.
  A present but incompatible `main` is code 20 in
  Delta-source space at its declaration name, including after earlier
  declarations and comments between `def` and `main`.

Body judgments use codes 9 through 18: active local conflict, repeated pattern
binder, unknown type/constructor/function/local, type mismatch, arity mismatch,
duplicate match case, and nonexhaustive match. Names anchor at their exact
tokens; conflicts at the later binder; type mismatches at the offending
argument, initializer, condition, false branch, body result, or scrutinee.
Wrong pattern ownership anchors at its constructor name. Arity uses the
application, bare-constructor, or pattern start; duplicate cases use the later
constructor name; missing coverage uses the match start.

Competing-error controls pin the semantic sequence. Calls resolve their head,
then each expected argument and its type in order, before discovering a missing
or extra argument. A `let` resolves its annotation, checks its binder against
the outer environment, and checks its initializer in that unchanged environment.
A match checks its subject, then each arm's constructor identity, owner, arity,
case uniqueness, binders, body, and result agreement before complete coverage.
An invalid frontend cannot become missing-entry or entry-schema rejection.
These authored semantic failures now compare exact DCOUT frames, not generic
evaluator status 249. Resource/internal failures remain separate obligations.

Two additional 1,000-level semantic controls exercise retained continuation
worklists: nested calls reject the exact innermost unknown local, and nested
valid `if` expressions complete checking before missing-entry code 19. Neither
requires deeply nested Gamma emission; successful emission at that depth is
not claimed.

The 22 controls in `depth_fixtures.py` pin D30's expression `parse_depth`:
function bodies begin at level 1, and expression children, including atoms,
advance one level. At level 1,025 the compiler returns `Incomplete` code 8 at
the expression start, with limit 1,024 and requested 1,025. Exact-limit valid
programs complete frontend checking and report missing-entry code 19 without
requiring deep Gamma emission. Calls, let initializers and bodies, match
scrutinees and arm bodies, and sibling depth restoration are covered. Nested
single-case matches have additional pattern and arm parentheses that must not
count toward expression depth. Source bytes, lexical defects, and complete
balanced parsing precede depth accounting; earlier grammar defects retain
priority, while depth refusal precedes grammar judgment at the refused node
and the later global census and semantic phases.

Accepted programs exercise identity compilation, exact entry selection
after `main_suffix`, cross-namespace spelling reuse, forward and mutual data
visibility, forward and mutual function visibility, and the admitted ASCII
whitespace/comment boundaries. They include both exact signed integer limits,
negative zero, leading zeros, binary `+` and `-`, and malformed numeric text
ignored inside comments ending at LF, CR, CRLF, or EOF. A generated ordinary
function with 200 parameters and a matching 200-argument call preserves its
last argument. An outer let binder stays absent throughout its initializer, so
an inner initializer-local binder may reuse its spelling. Scope controls restore
environments between sibling expressions,
branches, and match arms, and nested matches retain independent coverage.
Mixed `Int`/`Bytes`/nominal payloads project each field and distinguish constructor
layouts; a 64-field alternating payload and its nullary sibling preserve their
exact types. Negative controls reject wrong first, middle, and last payload
arguments and wrongly used pattern binders. Each compiles
twice to identical bytes; its generated application preserves an exact binary
input including NUL and high bytes.

The sibling `name_fixtures.py` adds 1,024-byte type, constructor, function, and
local names. Shared-prefix controls insert terminals before and after their
extensions, observe distinct function payloads, and retain sibling nominal
owners. Scoped local roots preserve outer bindings while disjoint scopes and
match arms reuse long spellings. Exact long-name duplicates retain codes 6,
7, 8, 9, and 10 at the later name; unknown shorter prefixes, longer extensions,
and escaped locals retain exact unknown-name diagnostics. These are authored
byte constructions, not a host implementation of the exact-name trie.

[`census_cursors.py`](census_cursors.py) adds six exact rejection controls and
three accepted programs that leave and revisit catalog prefixes in authored
order. They distinguish a prefix from a complete name, retain independent type
and constructor owners, preserve every function payload, and revisit 1,024-byte
prefixes after unrelated insertions. Each accepted program is compiled twice
and must preserve the same binary application input as the other controls.

[`parameter_cursors.py`](parameter_cursors.py) adds four exact rejection
controls and two accepted programs for the separate parameter builder. They
check duplicate/annotation ordering, absent prefixes and extensions after
finishing the environment, and retained mixed `Int`/`Bytes`/nominal types after
leaving and revisiting short and 1,024-byte prefixes. Both accepted programs
compile twice and preserve the same binary application input.

The expected coordinates are literal authored fixture facts or lengths of
explicit fixture-construction prefixes, never source searches. Whole-frame
comparison checks the reason, halt/tag agreement, coordinate space, reserved
zeros, little-endian coordinate, and exact resource fields (zero when unused). Compiler
source identity is pinned before any observation. The gate implements the
bounded checks above, not full resource conformance, arbitrary emission-depth
closure, or closure of the Delta bootstrap edge.

The phase order is fixed by [D20](../../../wiki/architecture/bootstrap_chain/decisions.md#d20--delta-names-resolve-through-four-namespaces-without-active-shadowing)
and [D33](../../../wiki/architecture/bootstrap_chain/decisions.md#d33--dcout-admission-and-schema-diagnosis-are-bounded-and-total).
See also the [Delta language](../../../bootstrap/delta/LANGUAGE.md) and the
adjacent [request-boundary gate](../request-boundary/README.md).
