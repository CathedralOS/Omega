# Omega bootstrap resolved-source lowering, outer version 17

[`OMGRSW8`](OMEGA_BOOTSTRAP_RESOLUTION_V8.md) |
[`CKIR16`](OMEGA_BOOTSTRAP_CHECKED_IR_V16.md)

`OMGLOWH` version 17 is the bounded producer relation from exact OMGCOMP1 and
canonical OMGRSW8 custody to CKIR16. The exact 32-byte header is:

```text
offset  width  field
0       8      magic: ASCII "OMGLOWH\0"
8       u16    outer version: 17
10      u16    minor: 0
12      u16    flags: zero
14      u16    header size: 32
16      u32    exact total frame length
20      u32    exact OMGCOMP1 length
24      u32    exact OMGRSW8 length
28      u32    resolution selector: 8
32      ...    exact OMGCOMP1 || exact OMGRSW8 || exact EOF
```

Component ceilings remain 267,280 and 524,288 bytes; the complete frame
ceiling remains 791,600 bytes. Magic, version, selector, witness identity, and
CKIR major form one exact pair. Relabeling or cross-pairing cannot manufacture
a valid product.

## 1. Selected source relation

The selected expression is exactly one pure direct `<` between same-carrier
kind-10 operands. Each side is a direct named parameter, direct `self` field,
or a contextual decimal literal, and at least one side is typed. Literals are
bounded to the full unsigned 64-bit carrier. Calls, indexing, mutation,
assignment inside the expression, nested arithmetic, coercion, user dispatch,
other comparison operators, and mixed carriers are excluded.

At least one selected relation must be lowered. The source closure may also
contain inherited relations independently admitted by its exact witness;
those preserve their frozen producer meaning and cannot satisfy the u64 Less
requirement.

## 2. Canonical type and operation lowering

The source and checked-IR kind numbers are deliberately not inferred to be
identical. OMGLOWH explicitly maps OMGRSW8 kind 10, flags zero, to CKIR16 kind
8, flags zero. All four endpoint words are copied in positional order and
validated as an unsigned 64-bit interval. Full and constrained rows remain
distinct canonical types.

A contextual kind-8 constant emits opcode 1 `Const`; immediate 0 is the low
32 bits and immediate 1 is the high 32 bits. Every bit pattern is data,
including `0xffffffff`; neither immediate uses structural `NO_ID` semantics.
The selected comparison emits opcode 9 `Less` with two visible kind-8 values
in authored left/right order, canonical bool result, zero flags, and no other
immediate meaning. It is pure and nontrapping. No folding, commuting,
coercion, or alternative opcode is permitted.

Kind 8 has size and alignment eight. Both halves survive field storage and
loads, exact machine call parameters and results, block parameters and edges,
returns, and admitted constructor custody. Range checks compare endpoint pairs
unsigned and reject a value range not contained by the receiving type.

## 3. True-edge range custody

For a direct guard `left < right`, the producer derives a fact only for the
true predecessor and only when `left` has a direct named subject. Its inclusive
upper endpoint is the contextual right operand's inclusive upper endpoint
minus one. Subtraction is a two-word unsigned decrement, including borrow from
low word zero. The fact is paired `(low32, high32)` and is joined across all
predecessors; equality is not a fixed-point change, and a factless predecessor
removes fact custody.

That fact may narrow the direct left subject and justify a constrained u64
edge argument. The false edge receives no refinement. State-parameter interval
transport, subsequent calls, storage, and edge publication retain both halves.
CKIR16 serializes the constrained target type and ordinary edge check, not a
separate proof-fact table.

## 4. Failure and resources

Malformed framing, source/witness pairing, kind mapping, endpoint order,
literal width, impurity, carrier mismatch, excluded syntax, missing selected
Less, range-custody failure, or identity/cross-major drift selects 251.
Declared source, expression, table, operation, operand, value, output, or
fixed-point resource exhaustion selects 252. Failure publishes no partial
CKIR bytes.

Producer evidence is `../gates/delta-resolved-to-ckir16-fixture.py` and
`../gates/fixtures/ckir16-u64-less/general.omg`. The focused gate requires
native/self resolver and lowerer byte parity, exact OMGRSW8/OMGLOWH/CKIR16
identities, two-word constants, kind-10-to-kind-8 mapping, borrow-bound true
custody, storage/call/edge transport, and adjacent policy, carrier, literal,
boundary, width, false-edge, and cross-version negatives.
