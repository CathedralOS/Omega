# Omega bootstrap checked IR schema major 10

[`CKIR9`](OMEGA_BOOTSTRAP_CHECKED_IR_V9.md) |
[`OMGRSW1`](OMEGA_BOOTSTRAP_RESOLUTION.md) |
[`OMGRSW2`](OMEGA_BOOTSTRAP_RESOLUTION_V2.md) |
[`OMGRSW3`](OMEGA_BOOTSTRAP_RESOLUTION_V3.md) |
[conservative backend evidence](OMEGA_BOOTSTRAP_CHECKED_IR_V10_BACKEND.md)

CKIR schema major 10 is the private successor for one explicit integer-widen
bridge relation. It adds opcode 21 `IntegerWiden` and otherwise inherits
CKIR9's header, table order, row widths, canonical ordering, meanings,
resources, statuses, and publication rules. Opcodes 1 through 20 remain
available but are no longer required. Earlier OMGLOW and CKIR identities and
bytes remain frozen.

This is bounded bridge-cost evidence for exact `u8 as u32 in Trapping`. It is
not narrowing, coercion inference, a public Omega IR, general policy or domain
qualification, or final admission to `Ωself`.

## 1. Independent lowering and resolution versions

The cast surface creates no declaration, type, field, case, call target, or
other resolution identity. There is therefore no new OMGRSW schema. Resolution
continues to publish the least canonical OMGRSW1, 2, or 3 required by the exact
source.

The resolved-source lowerer consumes `OMGLOWB`; `B` is the single-byte
successor label and the numeric header version is 11:

```text
offset  width  field
0       8      magic: ASCII "OMGLOWB\0"
8       u16    schema major: 11
10      u16    schema minor: 0
12      u16    flags: zero
14      u16    header size: 32
16      u32    exact total frame length
20      u32    exact OMGCOMP length
24      u32    exact selected OMGRSW1/2/3 length
28      u32    exact selected OMGRSW major: 1, 2, or 3
32      ...    exact OMGCOMP || exact selected OMGRSW || exact EOF
```

The selector is framing, not authority. Component and aggregate ceilings
remain 267,280, 524,288, and 791,600 bytes. `OMGLOWB` requires at least one
admitted cast and always emits CKIR10. It accepts inherited opcodes 1 through
20 but rejects a source without opcode 21, a selector/witness mismatch, and
every old/new frame cross-pair.

## 2. Selected source relation

The only admitted surface is an explicit exact-u8 leaf followed by the exact
tokens `as u32 in Trapping`. Parentheses around the leaf are permitted. The
form is permitted on assignment right-hand sides and in already-admitted
single-argument call contexts. Its result has the exact canonical `u32` carrier
with `Trapping` policy and preserves the operand's mathematical payload.

The operand must be a direct field or parameter leaf, pure, total, and
nontrapping. Calls, indexing, arithmetic, mutation, constructors, user
dispatch, structural values, Boolean, `u32`, constrained or policy-qualified
`u8`, and other operand carriers are rejected. Narrowing, `u64`, `i32`, Boolean
targets, bare `as u32`, other policies or domains, and constrained target rows
are rejected. No resolver fact, schema, or identity is introduced.

Canonical lowering emits one opcode-21 row for every authored admitted form,
in source construction order. It does not fold the cast, reinterpret signed
bits, infer a conversion, or synthesize dispatch. Attaching the selected target
policy changes no payload and performs no user work.

## 3. Opcode 21

`IntegerWiden` uses the inherited 40-byte operation row:

- schema major is exactly 10;
- result kind is value and result ID is the next dense value ID;
- result type is the unique scalar row `(u32, Trapping, 0..2147483647)`;
- operand count is exactly one;
- the operand is a visible value whose type is the unique exact unqualified
  scalar row `(u8, 0..255)`; and
- both immediate words and inherited flags/reserved fields are zero.

Its result is the operand's unsigned mathematical value, unchanged. CKIR10
requires at least one opcode-21 row. CKIR9 rejects opcode 21. Merely changing a
CKIR9 major cannot create canonical CKIR10.

## 4. Resources, status, and non-expansion

Each widening consumes one expression-depth level, operation row, operand
word, value, and four-byte scalar slot. It introduces no table, arena,
allocator, resolver fact, or ceiling. Inherited limits remain normative,
including expression depth 8, 32,768 operations, 94,208 operand words, 36,864
values, the 262,144-byte machine frame, 1-MiB text bound, and complete CKIR
byte bound.

Malformed or excluded syntax, type/policy/visibility/arity drift, nonzero
immediates or reserved fields, missing opcode 21, and version cross-pairs
select 251. Resource exhaustion selects 252. Neither status publishes CKIR or
ELF bytes.

## 5. Focused evidence

- `../gates/delta-resolved-to-ckir10.sh` checks Delta-native/self OMGLOWB
  production over least OMGRSW1/2/3, exact token/operation correspondence,
  assignment, parentheses, single-argument calls, 0/70/255, negatives, and
  depth controls.
- `../gates/delta-resolved-to-ckir10-meaning.sh` observes canonical result,
  semantic rejection, resource exhaustion, and exact publication through the
  persisted-Beta/Gamma path.
- `../gates/delta-checked-ir-v10-reference.sh` independently decodes,
  validates, interprets, and mutates CKIR10.
- `../gates/delta-checked-ir-v10-backend.sh` checks Delta-native/self artifact
  identity, exact unsigned widening bytes, mutation rejection, and 0/70/255.

These gates establish the selected producer, independent CKIR meaning, and
conservative backend. Same-frame refinement remains a separate admission
obligation.
