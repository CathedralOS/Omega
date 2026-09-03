# Selected staged Delta compiler

`delta_compiler.gamma` is the first selected Gamma-authored Delta stage. It is a
source transformer executed by the selected Beta-authored Gamma evaluator.
`delta_compiler.composed` binds those exact source and evaluator-tape identities
under `GammaComposedV1`.

The current stage accepts ordinary scalar/effect Gamma plus finite data whose
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
names use the reserved `__m` prefix.

This is a meaningful early stage, not the complete Delta compiler. It does not
yet provide normative `Bytes`, checked arithmetic, complete type checking, production
application profiles, or proper-tail guarantees beyond those available in
Gamma.

The executable gate is
[`../../../tests/delta/staged-compiler/`](../../../tests/delta/staged-compiler/).
The downgraded full compiler remains separate under
[`../bootstrap/concatenative-compiler/`](../bootstrap/concatenative-compiler/).

## Measurements

```text
852-line / 34,043-byte Gamma source
7-line / 195-byte nullary-ADT Delta fixture
  -> 3-line / 159-byte Gamma receipt
  -> selected Gamma evaluation produces byte 9
7-line / 186-byte payload-ADT Delta fixture
  -> 3-line / 229-byte Gamma receipt
  -> selected Gamma evaluation produces byte 9
7-line / 187-byte recursive-ADT Delta fixture
  -> 3-line / 246-byte Gamma receipt
  -> selected Gamma evaluation produces byte 3
8-line / 221-byte two-field recursive List fixture
  -> 3-line / 324-byte Gamma receipt
  -> selected Gamma evaluation produces byte 9
24-line / 789-byte three-field recursive Bytes-rope fixture
  -> 7-line / 1,040-byte Gamma receipt
  -> indexing produces byte 0x42; indexing empty traps
3,001-function / 66,266-byte scale fixture
  -> 66,267-byte Gamma receipt
  -> selected Gamma evaluation produces byte 199
```
