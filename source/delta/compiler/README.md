# Selected staged Delta compiler

`delta_compiler.gamma` is the first selected Gamma-authored Delta stage. It is a
source transformer executed by the selected Beta-authored Gamma evaluator.
`delta_compiler.composed` binds those exact source and evaluator-tape identities
under `GammaComposedV1`.

The current stage accepts the Gamma-shaped scalar core, immutable `Bytes`, and
finite data whose constructors carry any finite number of `Int`, `Bytes`, or
known nominal fields, plus declaration-order exhaustive matches. It assigns constructor tags in declaration
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

A type-checking pass builds each function's local environment from its typed
parameters, then extends the immutable exact-name trie for `let` bodies and
individual match arms. It rejects unknown value atoms, self-reference from a
`let` initializer, and any parameter, `let`, or pattern binder that duplicates
an active local. Immutable roots give lexical pop without mutation: sibling
expressions, branches, and disjoint match arms may reuse the same spelling.
The pass checks every currently emitted scalar, `Bytes`, and nominal constructor field,
pattern binder, call argument, `let` initializer, operator, conditional, match
arm, and declared result. Function and local names remain
grammar-distinguished namespaces.

The global function trie carries each exact declaration owner as its terminal
payload. Application heads resolve through that checked table, including
forward and mutual calls; arity, parameter types, and result type are recovered
from the retained declaration. Every user call, operator, and `if` has an exact
argument count. Undeclared Gamma effects such as `input`, `read`, and `pair`
therefore cannot leak through as Delta calls; an ordinary Delta function may
still deliberately use one of those spellings after declaring it. Every
non-`main` function definition and call receives the injective `__d_` Gamma
prefix, preventing such a declaration from being captured by Gamma's builtin
dispatch. `main` alone retains the name required by the evaluator.

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

This is a meaningful early stage, not the complete Delta compiler. It does not
yet provide production application profiles or canonical compiler-boundary
failures. Calls emitted in tail position remain in Gamma tail position through
`if`, `let`, and lowered `match`; the selected evaluator executes a 100,000-node
construction and traversal in bounded call context. Static acceptance of the
scalar/nominal slice is not full-language admission.

The executable gate is
[`../../../tests/delta/staged-compiler/`](../../../tests/delta/staged-compiler/).
The downgraded full compiler remains separate under
[`../bootstrap/concatenative-compiler/`](../bootstrap/concatenative-compiler/).

## Measurements

```text
2,020-line / 80,194-byte Gamma source
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
3,001-function / 66,266-byte scale fixture
  -> 78,271-byte Gamma receipt
  -> selected Gamma evaluation produces byte 199; staged transformation is
     about 12 seconds on the development host
```
