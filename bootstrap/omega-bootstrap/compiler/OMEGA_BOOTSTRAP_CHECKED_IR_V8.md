# Omega bootstrap checked IR schema major 8

[`CKIR7`](OMEGA_BOOTSTRAP_CHECKED_IR_V7.md) |
[`OMGRSW1`](OMEGA_BOOTSTRAP_RESOLUTION.md) |
[`OMGRSW2`](OMEGA_BOOTSTRAP_RESOLUTION_V2.md) |
[`OMGRSW3`](OMEGA_BOOTSTRAP_RESOLUTION_V3.md) |
[conservative backend evidence](OMEGA_BOOTSTRAP_CHECKED_IR_V8_BACKEND.md)

CKIR schema major 8 is the private successor for the selected primitive
scalar-equality relation. It adds opcode 18 `ScalarEqual` and otherwise
inherits CKIR7's header, table order, row widths, canonical ordering, meanings,
resources, statuses, and publication rules. Opcodes 15 through 17 remain
available but are no longer required. Earlier OMGLOW and CKIR identities and
bytes remain frozen.

This is bounded bridge-cost evidence for exact `bool`, `u8`, and `u32`
equality. It is neither a public Omega IR nor aggregate, sum, coercing,
user-defined, or general equality coverage, and it is not final admission to
`Ωself`.

## 1. Independent lowering and resolution versions

Compiler-owned primitive `==` syntax creates no declaration, type, field,
case, call target, or other resolution identity. There is therefore no
`OMGRSW4`. Resolution continues to publish the least canonical OMGRSW1, 2, or
3 required by the exact source, independently of equality syntax.

The resolved-source lowerer consumes the distinct `OMGLOW9` frame:

```text
offset  width  field
0       8      magic: ASCII "OMGLOW9\0"
8       u16    schema major: 9
10      u16    schema minor: 0
12      u16    flags: zero
14      u16    header size: 32
16      u32    exact total frame length
20      u32    exact OMGCOMP length
24      u32    exact selected OMGRSW1/2/3 length
28      u32    exact selected OMGRSW major: 1, 2, or 3
32      ...    exact OMGCOMP || exact selected OMGRSW || exact EOF
```

The selector is framing, not authority. The lowerer validates the complete
selected witness and its canonical least-version relation to the source.
Component and aggregate ceilings remain 267,280, 524,288, and 791,600 bytes.

`OMGLOW9` requires at least one admitted `==` and always emits CKIR8. A source
needing only `!`, `&&`, or `||` retains its earlier least lowering relation.
OMGLOW9 accepts those inherited logical forms but rejects a source without an
admitted equality, a selector/witness mismatch, and every old/new frame
cross-pair.

## 2. Selected source relation

Omega `==` compares two operands for equality and produces canonical `bool`.
Comparison operators bind tighter than equality; equality binds tighter than
`&&`; and equality associates left. Thus the selected construction order for
`1 < 2 == true && true || false` is `<`, `==`, `&&`, `||`, while
`true == false == false` is `(true == false) == false`.

The admitted operand pair is deliberately narrow:

- both operands have the same primitive carrier: exact `bool`, exact `u8`, or
  exact `u32`;
- an integer literal is contextualized by the other numeric operand, or is
  exact `u32` when both operands are integer literals;
- both operands terminate, are pure, and cannot trap; and
- each operand is a value with no observable mutation, allocation, call,
  boundary, or user-defined dispatch.

The current admitted operand closure consists of literals, parameters,
ordinary receiver-field loads, and inherited pure/nontrapping scalar or
Boolean operations. Calls, array or slice indexing, trapping arithmetic,
mutation, constructors, records, sums, mixed carriers, `u64`, `!=`, truthiness,
coercion, and user-defined equality are rejected. In particular, this relation
does not infer structural equality for records or discriminant/payload
equality for sums.

Canonical lowering emits one opcode-18 row for every authored admitted `==`
token pair, in precedence and left-associative construction order. It does not
fold literals, swap operands, widen values, synthesize aggregate traversal, or
turn assignment `=` into equality. Results are values and carry no place,
mutability, receiver, path, or transition-fact identity.

## 3. Opcode 18

`ScalarEqual` uses the inherited 40-byte operation row:

- schema major is exactly 8;
- result kind is value and result ID is the next dense value ID;
- result type is the exact canonical Boolean type;
- operand count is exactly two;
- operands are two visible values whose type rows have the same scalar kind,
  one of `u8`, `u32`, or `bool`, in authored left/right order; and
- both immediate words and inherited flags/reserved fields are zero.

The operation produces one exactly when the two carrier values are equal and
zero otherwise. Distinct compatible constrained type rows are permitted only
when their carrier kind is the same; no cross-carrier comparison is implied.
CKIR8 requires at least one opcode-18 row. CKIR7 rejects opcode 18, and changing
only a CKIR7 major cannot create canonical CKIR8.

## 4. Resources, status, and non-expansion

Each scalar-equality node consumes one expression-depth level, operation row,
two operand-vector words, value, and four-byte scalar slot. It introduces no
new table, arena, allocator, or ceiling. Inherited ceilings remain normative,
including expression depth 8, 32,768 operations, 94,208 operand words, 36,864
values, the 262,144-byte machine frame, the 1-MiB text bound, and the complete
CKIR byte bound. Total expression depth 8 succeeds and depth 9 selects 252.

Malformed syntax, `!=`, an excluded carrier or expression, identity/type/
visibility/arity/order drift, nonzero immediates or reserved fields, a missing
required scalar-equality operation, and version cross-pairs select 251.
Resource exhaustion selects 252. Neither status publishes CKIR or ELF bytes.

## 5. Focused evidence

The focused implementation evidence is:

- `../gates/delta-resolved-to-ckir8.sh`: Delta-native and Delta-self-built
  OMGLOW9 production over least OMGRSW1/2/3, exact precedence, association,
  authored-token correspondence, type and purity negatives, inherited
  composition, and depth 8/9;
- `../gates/delta-resolved-to-ckir8-meaning.sh`: persisted-Beta translation of
  the actual Delta lowerer plus canonical Gamma observations of result,
  semantic rejection, resource exhaustion, and exact publication;
- `../gates/delta-checked-ir-v8-reference.sh`: independent decoding,
  validation, equality meaning for all Boolean rows and selected `u8`/`u32`
  rows, result reconstruction, and isolated mutations; and
- `../gates/delta-checked-ir-v8-backend.sh`: Delta-native/self artifact
  identity, pinned instruction templates, and artifact mutations.

These gates establish the selected producer, independent CKIR meaning, and
conservative backend. Same-frame persisted-Beta refinement remains a separate
admission obligation. Nothing here admits aggregate or sum equality, `!=`,
`u64`, cross-carrier equality, effectful operands, or the feature to final
`Ωself`.
