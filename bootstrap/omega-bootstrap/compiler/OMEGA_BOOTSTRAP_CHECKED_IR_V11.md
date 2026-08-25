# Omega bootstrap checked IR schema major 11

[`CKIR10`](OMEGA_BOOTSTRAP_CHECKED_IR_V10.md) |
[`OMGRSW1`](OMEGA_BOOTSTRAP_RESOLUTION.md) |
[`OMGRSW2`](OMEGA_BOOTSTRAP_RESOLUTION_V2.md) |
[`OMGRSW3`](OMEGA_BOOTSTRAP_RESOLUTION_V3.md)

CKIR schema major 11 is the private successor for one explicit trapping-
addition bridge relation. It introduces no opcode: the selected surface lowers
to inherited opcode 8 `Add`. It otherwise inherits CKIR10's header, tables, row
widths, canonical ordering, meanings, resources, statuses, and opcodes 1
through 21. Earlier OMGLOW and CKIR identities and bytes remain frozen.

This is bounded bridge-cost evidence for a common checkpoint source form. It
is not a public Omega IR, a general arithmetic admission, a full-width host
`u32` claim, or final admission to `Ωself`.

## 1. Independent lowering and resolution versions

The selected expression creates no declaration, type, field, case, call
target, or other resolution identity. There is no new OMGRSW schema. Resolution
continues to publish the least canonical OMGRSW1, 2, or 3 required by the exact
source.

The resolved-source lowerer consumes `OMGLOWC`; `C` is the single-byte
successor label and the numeric header version is 12:

```text
offset  width  field
0       8      magic: ASCII "OMGLOWC\0"
8       u16    schema major: 12
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
remain 267,280, 524,288, and 791,600 bytes. `OMGLOWC` requires at least one
selected trapping addition and always emits CKIR11. It accepts inherited
opcodes 1 through 21, but rejects a source without the selected relation, a
selector/witness mismatch, and every old/new frame cross-pair.

## 2. Selected source relation

The admitted form is exactly a direct field or parameter leaf of the unique
canonical `u32 in Trapping` type, the authored `+` token, and a nonnegative
anonymous integer literal representable by that same bridge carrier.
Parentheses around either leaf or literal are permitted. The result has the
same exact canonical type and `Trapping` policy.

The form is admitted on assignment right-hand sides, in guards, and in
already-admitted call and state-transition argument lists. Because Omega does
not yet give portable source an observable relative call-argument evaluation
order, a call argument list may contain at most one potentially trapping
argument. Every sibling call argument must be pure, total, and nontrapping. The
already-admitted direct call receiver is likewise pure, total, and
nontrapping. This rule is independent of arity: a qualifying selected addition
is admitted in any supported call-argument position of any supported arity.

The leaf operand itself is pure, total, and nontrapping. Literal-left, typed-
right, nested arithmetic, calls, indexing, mutation, constructors, and user
dispatch inside either operand are outside this relation. So are `u8`, `u64`,
`i32`, Boolean, structural, cross-carrier, constrained, other-policy, and
domain-qualified additions, and calls with two potentially trapping arguments.
No resolver fact, schema, or identity is introduced.

Canonical lowering emits one opcode-8 row for every selected authored form, in
source construction order. The operation operands retain authored left/right
order. The producer does not constant-fold the selected operation or reject it
merely because the leaf's static interval includes values that could overflow.

## 3. Selected opcode-8 meaning

The selected `Add` uses the inherited 40-byte operation row:

- schema major is exactly 11;
- result kind is value and result ID is the next dense value ID;
- result type and both visible operand types are the unique scalar row
  `(u32, Trapping, 0..2147483647)`;
- operand count is exactly two and operand order is authored left then right;
  and
- both immediate words and inherited flags/reserved fields are zero.

For mathematical operand payloads `left` and `right`, successful execution
returns `left + right` when that sum is at most 2147483647. A larger sum traps
before producing or storing a result. The producer conservatively assigns the
successful result the complete canonical target interval; it does not perform
the potentially overflowing endpoint addition inside the compiler.

CKIR11 requires at least one selected canonical trapping opcode-8 row. Other
inherited opcode-8 rows remain available but do not satisfy that requirement.
CKIR10 rejects a CKIR11 major, and merely changing a CKIR10 major cannot create
the selected source relation.

## 4. Resources, status, and non-expansion

Each selected addition consumes one expression-depth level, operation row,
two operand words, value, and four-byte scalar slot. It introduces no table,
arena, allocator, resolver fact, or ceiling. Inherited limits remain normative,
including expression depth 8, 32,768 operations, 94,208 operand words, 36,864
values, the 262,144-byte machine frame, 1-MiB text bound, and complete CKIR
byte bound.

Malformed or excluded syntax, carrier/policy/visibility/argument-order drift,
nonzero immediates or reserved fields, missing selected addition, and version
cross-pairs select 251. Resource exhaustion selects 252. Neither status
publishes CKIR bytes.

## 5. Focused producer evidence

- `../gates/delta-resolved-to-ckir11.sh` checks Delta-native/self OMGLOWC
  production over least OMGRSW1/2/3, exact token/operation correspondence,
  assignment, guard, multi-argument call, and multi-argument transition
  contexts, a source whose addition can overflow at runtime, and strong
  carrier/shape/order/version negatives.
- `../gates/checked_ir_v11_reference.py` independently decodes, validates, and
  interprets the published CKIR11 relation.

Backend execution, persisted-Beta/Gamma meaning, and same-frame refinement are
separate admission obligations.
