# Selected staged Delta compiler

`delta_compiler.gamma` is the first selected Gamma-authored Delta stage. It is a
source transformer executed by the selected Beta-authored Gamma evaluator.
`delta_compiler.composed` binds those exact source and evaluator-tape identities
under `GammaComposedV1`.

The current stage accepts the Gamma-shaped scalar core plus finite data whose
constructors carry any finite number of `Int` or known nominal fields, and
declaration-order exhaustive matches. It assigns constructor tags in declaration
order. Payload-bearing nominal values become immutable `(pair tag product)`
nodes whose products are right-nested pairs. Nullary constructors in a payload
type carry zero padding. Matches project the tag and product once and recover
binders with ordinary generated Gamma lets.

```text
(data Choice (Left) (Right))
```

becomes the scalar tag representation `Left = 0`, `Right = 1`. `Option` becomes
`Some 9 = (pair 1 9)` and `None = (pair 0 0)`. A recursive `List` constructor
with head and tail fields becomes `(pair tag (pair head tail))`. A match must contain
exactly one arm for every constructor in declaration order. Generated local
names use `$m`, `$p`, and `$v` prefixes. `$` is outside Delta's identifier
alphabet but inside Gamma's, so generated binders cannot capture or be captured
by an authored Delta name.

Before tokenization or emission, the stage rejects every source byte except HT,
LF, CR, and printable ASCII, exactly matching Delta's textual envelope. It then
scans the complete declaration sequence.
It requires all nonempty `data` declarations before one or more functions,
exactly one `main`, and unique type, constructor, and function declarations in
their separate namespaces. Exact source-byte names are retained in persistent
bitwise tries, so this check has neither hash collisions nor repeated
whole-source lookup. Type and constructor names may still share a spelling, as
required by Delta's grammar-distinguished namespaces. Name recursion advances
once per complete byte; a 200-byte identifier witness guards practical Gamma
call-context headroom, while the Epsilon customer currently tops out at 56.

The emitted slice also validates identifier spelling at declarations, types,
parameters, local binders, constructor patterns, atoms, and application heads.
Keywords, `Int`, `Bytes`, and the five closed `bytes_*` builtin names cannot be
redeclared. Decimal literals are scanned without overflow and admit exactly
`INT64_MIN..INT64_MAX`. The global census also rejects repeated parameter names
within a function before expression emission begins.

A scope-validation pass builds each function's local environment from its
parameters, then extends the immutable exact-name trie for `let` bodies and
individual match arms. It rejects unknown value atoms, self-reference from a
`let` initializer, and any parameter, `let`, or pattern binder that duplicates
an active local. Immutable roots give lexical pop without mutation: sibling
expressions, branches, and disjoint match arms may reuse the same spelling.
Function and local names remain grammar-distinguished namespaces.

The global function trie carries declared arity as its terminal payload.
Application heads resolve through that exact checked table, including forward
and mutual calls, and every user call, operator, `if`, and closed Bytes builtin
has an exact argument count. Undeclared Gamma effects such as `input`, `read`,
and `pair` therefore cannot leak through as Delta calls; an ordinary Delta
function may still deliberately use one of those spellings after declaring it.
Every non-`main` function definition and call receives the injective `__d_`
Gamma prefix, preventing such a declaration from being captured by Gamma's
builtin dispatch. `main` alone retains the name required by the evaluator.

This is a meaningful early stage, not the complete Delta compiler. It does not
yet provide normative `Bytes`, checked arithmetic, complete type checking, production
application profiles, or proper-tail guarantees beyond those available in
Gamma. Scope resolution does not establish expression, argument, arm, or result
types; staged acceptance is not language admission.

The executable gate is
[`../../../tests/delta/staged-compiler/`](../../../tests/delta/staged-compiler/).
The downgraded full compiler remains separate under
[`../bootstrap/concatenative-compiler/`](../bootstrap/concatenative-compiler/).

## Measurements

```text
1,457-line / 56,262-byte Gamma source
7-line / 195-byte nullary-ADT Delta fixture
  -> 3-line / 165-byte Gamma receipt
  -> selected Gamma evaluation produces byte 9
7-line / 186-byte payload-ADT Delta fixture
  -> 3-line / 230-byte Gamma receipt
  -> selected Gamma evaluation produces byte 9
7-line / 187-byte recursive-ADT Delta fixture
  -> 3-line / 251-byte Gamma receipt
  -> selected Gamma evaluation produces byte 3
8-line / 221-byte two-field recursive List fixture
  -> 3-line / 328-byte Gamma receipt
  -> selected Gamma evaluation produces byte 9
24-line / 767-byte three-field recursive rope fixture
  -> 7-line / 1,056-byte Gamma receipt
  -> indexing produces byte 0x42; indexing empty traps
3,001-function / 66,266-byte scale fixture
  -> 78,271-byte Gamma receipt
  -> selected Gamma evaluation produces byte 199; staged transformation is
     about 10 seconds on the development host
```
