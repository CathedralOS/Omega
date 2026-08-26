# Omega bootstrap checked IR schema major 7

[`CKIR6`](OMEGA_BOOTSTRAP_CHECKED_IR_V6.md) |
[`OMGRSW1`](OMEGA_BOOTSTRAP_RESOLUTION.md) |
[`OMGRSW2`](OMEGA_BOOTSTRAP_RESOLUTION_V2.md) |
[`OMGRSW3`](OMEGA_BOOTSTRAP_RESOLUTION_V3.md) |
[conservative backend evidence](OMEGA_BOOTSTRAP_CHECKED_IR_V7_BACKEND.md)

CKIR schema major 7 is the private successor for the selected Boolean
`&&`/`||` relation. It adds opcode 16 `LogicalAnd` and opcode 17 `LogicalOr`
and otherwise inherits CKIR6's header, table order, row widths, canonical
ordering, meanings, resources, statuses, and publication rules. Opcode 15
`LogicalNot` remains available but is no longer required. Earlier OMGLOW and
CKIR identities and bytes remain frozen.

This is bounded bridge-cost evidence for forms observed 44 times in product
source checkpoint 000001. It is neither a public Omega IR nor general
short-circuit-expression coverage or final admission to `Ωself`.

## 1. Independent lowering and resolution versions

Compiler-owned Boolean operator syntax creates no declaration, type, field,
case, call target, or other resolution identity. There is therefore no
`OMGRSW4`. Resolution continues to publish the least canonical OMGRSW1, 2, or
3 required by the exact source, independently of logical syntax.

The resolved-source lowerer consumes the distinct `OMGLOW8` frame:

```text
offset  width  field
0       8      magic: ASCII "OMGLOW8\0"
8       u16    schema major: 8
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

`OMGLOW8` requires at least one admitted `&&` or `||` and always emits CKIR7.
A source needing only `!` retains OMGLOW7 and CKIR6. A source needing none of
these operators retains its earlier least lowering relation. OMGLOW8 accepts
inherited `!` but rejects a source without a binary logical token, a
selector/witness mismatch, and every old/new frame cross-pair.

## 2. Selected source relation

Omega `&&` and `||` have their ordinary short-circuit meaning: evaluate the
left operand first; evaluate the right operand only when its value is needed.
`&&` binds tighter than `||`, both associate left, and prefix `!` binds tighter
than either:

```omega
transition self.ready || self.low <= value && value <= self.high { ... }
```

This relation admits only a finite tree whose leaves and internal inherited
expressions are statically proved:

- terminating;
- pure, with no mutation, allocation, call, boundary, or other observable
  effect;
- nontrapping; and
- exact canonical `bool`, without truthiness or coercion.

The current admitted leaf closure consists of Boolean literals, parameters,
ordinary receiver-field loads, inherited Boolean `!`, and already-admitted
primitive comparisons whose own operands satisfy the same boundary. Array or
slice indexing, calls, arithmetic marked Trapping, mutation, constructors,
user-defined operator dispatch, bitwise `&`/`|`, and diverging or effectful
forms are rejected inside either logical operand.

Because every admitted operand is total and observationally inert, evaluating
both operands and applying the Boolean truth function is observationally equal
to source short-circuiting. CKIR7 uses that eager private representation. This
equivalence is a condition of admission, not a change to Omega semantics and
not permission to eagerly lower broader operands.

Canonical lowering emits one operation for every authored `&&` and `||` token
pair, in precedence and left-associative construction order. It does not fold
literal expressions, reassociate trees, delete a right operand, or exchange
the operators. Results are values and carry no place, mutability, receiver,
path, or transition-fact identity.

## 3. Opcodes 16 and 17

Both opcodes use the inherited 40-byte operation row:

- schema major is exactly 7;
- result kind is value and result ID is the next dense value ID;
- result type is the exact canonical Boolean type;
- operand count is exactly two;
- operands are two visible values of that same exact Boolean type, in authored
  left/right order; and
- both immediate words and inherited flags/reserved fields are zero.

Opcode 16 computes canonical Boolean conjunction and opcode 17 computes
canonical Boolean disjunction. Validation already proves each operand is zero
or one. CKIR7 requires at least one opcode-16 or opcode-17 row. CKIR6 rejects
both opcodes, and changing only a CKIR6 major cannot create canonical CKIR7.

## 4. Resources, status, and non-expansion

Each logical binary node consumes one expression-depth level, operation row,
two operand-vector words, value, and four-byte scalar slot. It introduces no
new table, arena, allocator, or ceiling. Inherited ceilings remain normative,
including expression depth 8, 32,768 operations, 94,208 operand words, 36,864
values, the 262,144-byte machine frame, the 1-MiB text bound, and the complete
CKIR byte bound. Total expression depth 8 succeeds and depth 9 selects 252.

Malformed syntax, single `&`/`|`, non-Boolean or non-pure operands,
identity/type/visibility/arity/order drift, nonzero immediates or reserved
fields, a missing required logical-binary operation, and version cross-pairs
select 251. Resource exhaustion selects 252. Neither status publishes CKIR or
ELF bytes.

## 5. Focused and lower-rooted evidence

The focused evidence is:

- `../gates/delta-resolved-to-ckir7.sh`: Delta-native and Delta-self-built
  OMGLOW8 production over least OMGRSW1/2/3, exact precedence and token
  correspondence, purity negatives, inherited composition, and depth 8/9;
- `../gates/delta-resolved-to-ckir7-meaning.sh`: persisted-Beta translation of
  the actual Delta lowerer plus canonical Gamma observations of result,
  semantic rejection, resource exhaustion, and exact publication;
- `../gates/delta-checked-ir-v7-reference.sh`: independent decoding,
  validation, all truth rows, interpretation, result reconstruction, and
  isolated mutations;
- `../gates/delta-checked-ir-v7-backend.sh`: Delta-native/self artifact
  identity, pinned instruction templates, and artifact mutations; and
- `../../../source/assurance/refinement/omega-bootstrap/omgrfn9-same-frame-composite.sh`:
  persisted-Beta R1–R5 reconstruction over one immutable carrier with compact
  least-OMGRSW1/2 controls and exact result/ELF identity.

Together these gates close the selected pure/nontrapping Boolean relation. They
do not admit effectful short-circuit operands, primitive equality or remaining
comparison directions, general expression control flow, or the feature to
final `Ωself`.
