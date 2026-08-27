# Omega bootstrap resolved-source lowering, outer version 15

[`OMGRSW7`](OMEGA_BOOTSTRAP_RESOLUTION_V7.md) |
[`CKIR14`](OMEGA_BOOTSTRAP_CHECKED_IR_V14.md)

`OMGLOWF` is the bounded producer relation from exact OMGCOMP1 and canonical
OMGRSW7 custody to CKIR14 recursive full-width arithmetic. It inherits the
established 32-byte lowerer frame and component ceilings. The exact header is:

```text
offset  width  field
0       8      magic: ASCII "OMGLOWF\0"
8       u16    outer version: 15
10      u16    minor: 0
12      u16    flags: zero
14      u16    header size: 32
16      u32    exact total frame length
20      u32    exact OMGCOMP1 length
24      u32    exact OMGRSW7 length
28      u32    resolution selector: 7
32      ...    exact OMGCOMP1 || exact OMGRSW7 || exact EOF
```

The component ceilings remain 267,280 and 524,288 bytes and the complete
frame ceiling remains 791,600 bytes. The selector is framing, not authority;
the lowerer validates the complete canonical source/witness relation before
using either component.

Outer version 14 and the corresponding `OMGLOWE` identity are retired.
OMGLOWF does not accept, emit, relabel, or reinterpret that identity. No
change to magic, version, selector, or CKIR major can manufacture a valid
cross-pair.

## 1. Recursive source boundary

The selected expression grammar is the following bounded, pure, same-carrier
subset, where every `expression` has the one exact normalized full-range
`u32 in Trapping` type:

```text
expression      := additive
additive        := multiplicative (("+" | "-") multiplicative)*
multiplicative  := primary ("*" primary)*
primary         := typed-leaf | exact-widening | decimal-literal | "(" expression ")"
exact-widening  := exact-u8-leaf "as" "u32" "in" "Trapping"
```

`*` binds more tightly than `+` and `-`; operators at each level associate
left. Authored operand order is preserved. A typed leaf is a direct field,
named-state, machine-parameter, or block-parameter load with that exact type.
A widening operand is the already-settled pure, total, nontrapping exact-`u8`
direct leaf conversion; it preserves the unsigned payload and produces the
selected full-width carrier before the parent arithmetic node executes. A
decimal literal contains only decimal digits, is anonymously contextualized to
the same type, and represents a value in `0..=4294967295`. Parentheses may occur
at any admitted depth and do not add an IR node.

Every operand subtree is pure: it contains no call, indexing, mutation,
assignment, allocation, constructor, boundary crossing, user dispatch, or
observable identity. Arithmetic nodes may trap according to their operator;
leaves themselves are total and nontrapping. Mixed or constrained carriers,
other policies or domains, unary arithmetic, coercion, division, remainder,
shifts, user operators, and literal overflow are outside this relation.

The expression may occur on an admitted assignment right-hand side, in a
guard position whose surrounding relation accepts `u32`, or in an already-
admitted call or state-transition argument. An argument list contains at most
one potentially trapping expression; every sibling argument and the receiver
are pure, total, and nontrapping. This is an admission boundary, not an
assertion of portable relative argument evaluation order.

At least one admitted arithmetic node is required. A CKIR14 program need not
contain all three operators and need not contain a CKIR12 view operation.
Recursive composition means that either operand of any selected node may be
another admitted arithmetic node, including a node using a different selected
operator.

## 2. Canonical lowering

Canonical lowering walks the parsed expression in postorder and emits one
operation per authored conversion or operator token. Exact widening emits
opcode 21 with the exact unqualified `u8` source and selected full-width result;
`+` emits opcode 8 `Add`, `-` emits opcode 26 `Subtract`, and `*` emits opcode
27 `Multiply`. Every arithmetic row has the same exact operand/result type,
zero flags and immediates, and operands in authored left/right order. The
lowerer does not fold constants, reassociate, commute, distribute, combine
traps, or replace one operator with another.

Each node's successful result becomes visible only after that node's exact
full-width trap predicate succeeds. A parent is not evaluated when a child
traps. An assignment, argument, transition, machine result, CKIR output, or
other externally observable publication depending on the tree occurs only
after every node on the evaluated path succeeds. Static intervals remain the
complete `0..=0xffffffff` carrier; the compiler does not perform overflowing
endpoint arithmetic to narrow them.

The producer may also emit CKIR12 type kind 7, opcodes 22 through 25, and the
exact synthetic nonempty-edge relation when those forms are independently
present in the OMGRSW7 source closure. Their CKIR12 validation and meaning are
unchanged. They are optional and cannot satisfy OMGLOWF's required arithmetic
node.

## 3. Resource and failure discipline

Every arithmetic node consumes one expression-depth level, one operation row,
two operand words, one value, and one four-byte scalar slot. Each exact widening
consumes its inherited one operation, operand word, value, scalar slot, and
expression-depth level. CKIR12 ceilings remain normative, including total
expression depth 8, 32,768 operations,
94,208 operand words, 36,864 values, the 262,144-byte machine frame, the 1-MiB
text bound, and the complete CKIR byte bound. Total depth 8 succeeds; depth 9
selects 252.

Malformed or excluded syntax, a source/witness mismatch, type/purity/context/
argument drift, missing arithmetic, noncanonical lowering, or any identity or
cross-pair failure selects 251. Resource exhaustion selects 252. Neither
status publishes partial CKIR bytes, a result, or an artifact. Publication
begins only after complete source, witness, tree, CKIR sizing, and exact-EOF
preflight.

This relation defines no public evaluation-order ABI, optimizer permission,
general arithmetic, wrapping behavior, or facility to final `Ωself`.
