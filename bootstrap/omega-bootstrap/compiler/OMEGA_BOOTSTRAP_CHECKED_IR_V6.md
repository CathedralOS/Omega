# Omega bootstrap checked IR schema major 6

[`CKIR5`](OMEGA_BOOTSTRAP_CHECKED_IR_V5.md) |
[`OMGRSW1`](OMEGA_BOOTSTRAP_RESOLUTION.md) |
[`OMGRSW2`](OMEGA_BOOTSTRAP_RESOLUTION_V2.md) |
[`OMGRSW3`](OMEGA_BOOTSTRAP_RESOLUTION_V3.md) |
[conservative backend evidence](OMEGA_BOOTSTRAP_CHECKED_IR_V6_BACKEND.md)

CKIR schema major 6 is the private successor for bool-only prefix logical
negation. It adds opcode 15 `LogicalNot` and otherwise inherits the complete
CKIR5 header, table order, row widths, canonical ordering, meanings, resources,
statuses, and publication rules. Earlier OMGLOW and CKIR identities and bytes
remain frozen.

This is bounded bridge-cost evidence for a feature observed in product-source
checkpoint 000001. It is not a public Omega IR or final admission to `Ωself`.

## 1. Independent lowering and resolution versions

Logical negation creates no declaration, type, field, case, call target, or
other resolution identity. There is therefore no `OMGRSW4`. The resolver keeps
publishing the least canonical witness required by the exact source:

| Source closure | Resolution witness |
| --- | --- |
| no pure sum or direct field-receiver call | `OMGRSW1` |
| direct field-receiver call and no pure sum | `OMGRSW2` |
| at least one pure sum | `OMGRSW3` |

The resolved-source lowerer consumes the distinct `OMGLOW7` frame:

```text
offset  width  field
0       8      magic: ASCII "OMGLOW7\0"
8       u16    schema major: 7
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

`OMGLOW7` requires at least one admitted `!` and always emits CKIR6. A source
without logical negation retains OMGLOW4, 5, or 6 according to its least
resolution relation and emits its existing CKIR4 or CKIR5 bytes. OMGLOW4/5/6
reject `!`; OMGLOW7 rejects a source without `!`, a selector/witness mismatch,
and every old/new frame cross-pair. Resolution and checked-IR versions therefore
evolve independently rather than manufacturing an unrelated witness schema.

## 2. Source meaning and lowering

The admitted form is ordinary Omega prefix `!` over one already-admitted
Boolean expression. Prefix operators associate right and bind to the complete
postfix operand before binary operators:

```omega
self.empty = !self.saw_digit;
transition !!self.empty { ... }
```

The operand is evaluated and materialized exactly once. Its type and the result
type are exact canonical `bool`; `false` becomes `true` and `true` becomes
`false`. No integer truthiness, user-defined operator, domain-sensitive form,
or Boolean use of bitwise `~` is admitted.

Canonical lowering emits one opcode-15 operation for every authored `!`,
including literals and adjacent negations. It does not fold `!true`, `!false`,
or `!!value` away in this relation. The result is a value, never a place, and
does not carry lvalue, mutability, receiver, path, or transition-fact identity.

## 3. Opcode 15: `LogicalNot`

The inherited 40-byte operation row encodes opcode 15 as follows:

- schema major is exactly 6;
- result kind is value;
- result ID is the next dense value ID;
- result type is the exact canonical Boolean type;
- operand count is exactly one;
- the operand is one visible value of that same exact Boolean type; and
- both immediate words and inherited flags/reserved fields are zero.

CKIR6 requires at least one opcode-15 row. CKIR4/5 reject opcode 15, and merely
changing a CKIR5 major does not create canonical CKIR6.

The interpreter computes `1 - operand`. Validation already proves the operand
is canonical zero or one, so this is the exact truth function rather than a
truthiness conversion.

## 4. Resources, status, and non-expansion

Every `!` consumes one expression-depth level, operation row, operand-vector
word, value, and four-byte scalar value slot. It introduces no new public
table, arena, allocator, or ceiling. Inherited ceilings remain normative,
including expression depth 8, 32,768 operations, 94,208 operand words, 36,864
values, the 262,144-byte machine frame, the 1-MiB text bound, and the complete
CKIR byte bound. Total expression depth 8 succeeds and depth 9 selects 252.

Malformed syntax, non-Boolean operands, identity/type/visibility/arity drift,
nonzero immediate or reserved fields, missing required logical negation, and
version cross-pairs select 251. Resource exhaustion selects 252. Neither status
publishes CKIR or ELF bytes.

This tranche does not add general unary operators, Boolean/integer coercions,
short-circuit operators, constant folding, new resolution identities, or a
new Omega source dialect.

## 5. Focused and lower-rooted evidence

The focused evidence is:

- `../gates/delta-resolved-to-ckir6.sh`: Delta-native and Delta-self-built
  OMGLOW7 production over least OMGRSW1, 2, and 3, product-shaped field/call/sum
  composition, result 70, old/new cross-pairs, source negatives, and exact
  expression-depth 8/9;
- `../gates/delta-resolved-to-ckir6-meaning.sh`: persisted-Beta translation of
  the actual Delta lowerer plus canonical Gamma observations of least-OMGRSW1
  `false → true → false`, exact CKIR6 result 70, semantic 251, and resource 252;
- `../gates/delta-checked-ir-v6-reference.sh`: independent decoding, validation,
  interpretation, result 70, and isolated schema/arity/type/visibility/resource
  mutations; and
- `../gates/delta-checked-ir-v6-backend.sh`: Delta-native/self backend identity,
  the pinned logical-not instruction template, and artifact mutations.
- `../../assurance/refinement/omega-bootstrap/omgrfn8-same-frame-composite.sh`:
  persisted-Beta R1–R5 reconstruction over one immutable payload-sum carrier,
  compact least-OMGRSW1/2 controls, result 70, ownership mutations, version
  cross-pairs, and exact ELF identity.

These gates close the focused producer, Rust-free lowering meaning, checked-IR
meaning, conservative backend cost, and the selected lower-rooted OMGRFN8
source-to-artifact relation. This is a complete bounded vertical slice, not
general unary coverage or admission of the feature to final `Ωself`.
