# Omega bootstrap checked IR schema major 14

[`CKIR12`](OMEGA_BOOTSTRAP_CHECKED_IR_V12.md) |
[`OMGLOWF`](OMEGA_BOOTSTRAP_LOWERING_V15.md) |
[conservative backend evidence](OMEGA_BOOTSTRAP_CHECKED_IR_V14_BACKEND.md)

CKIR schema major 14 is the private recursive full-width
`u32 in Trapping` arithmetic successor. It directly inherits CKIR12's header,
table order, row widths, canonical ordering, type kind 7, block flag bit 0,
resources, statuses, publication rules, and opcodes 1 through 25. In
particular, CKIR12's `StaticByteView`, `SliceNonEmpty`, `SliceHead`,
`SliceTailOne`, and synthetic nonempty-edge relations remain available and
retain their exact meanings.

CKIR14 widens the selected opcode-8 `Add` relation to full `u32` and adds
opcode 26 `Subtract` and opcode 27 `Multiply`. For major 14, inherited opcode
21 `IntegerWiden` may produce the selected full-width type and that result may
be an arithmetic leaf; CKIR10's historical target remains frozen. Every CKIR14
program contains at least one selected arithmetic operation from the 8/26/27
set. It need not contain all three, a widening, or any view operation. This
optionality is the one deliberate override to CKIR12's carrier-selection
requirement. If the CKIR12 view family is present, its complete type,
literal-root, opcode, and control-safety relation is mandatory; arithmetic does
not weaken or bypass it.

This is checked bridge IR, not a public Omega IR or ABI, an optimizer license,
or final admission to `Ωself`.

## 1. Identity and full-width semantic words

The magic and inherited fixed header select schema major 14, schema minor 0,
and flags zero. Earlier CKIR identities and bytes remain frozen. CKIR13 is a
retired identity: its magic, major, opcode claims, and bytes are invalid here.
Changing only a CKIR12 or retired CKIR13 major cannot create CKIR14, and a
CKIR14 payload relabeled as another major is not a valid older carrier.

In CKIR14, scalar type range endpoints and scalar `Const` immediate 0 are
exact unsigned 32-bit semantic words. The selected type is the unique row:

```text
(kind u32, policy Trapping, low 0x00000000, high 0xffffffff)
```

A checker may retain these bits in a signed implementation cell only if it
decodes the word by field position and preserves all 32 bits. Structural IDs,
counts, offsets, ordinals, constant-child spans, and `NO_ID` retain the CKIR12
structural decoder and bounds. A semantic `0xffffffff` is not interchangeable
with structural `NO_ID`.

## 2. Selected arithmetic operations

The three selected operations use the inherited 40-byte operation row, have
zero flags and immediate words, consume exactly two visible operands, produce
the next dense value ID, and preserve authored left/right operand order:

| Opcode | Name | Successful mathematical result | Trap predicate |
| ---: | --- | --- | --- |
| 8 | `Add` | `left + right` | sum is greater than `0xffffffff` |
| 26 | `Subtract` | `left - right` | `left` is less than `right` |
| 27 | `Multiply` | `left * right` | product is greater than `0xffffffff` |

For each row, both operands and the result have the exact selected full-range
`u32 in Trapping` type. Mathematics is over unbounded nonnegative integers for
the trap decision, then the successful result is encoded as its exact 32-bit
word. Signed overflow, signed comparison, host-language wraparound, or a low-
word-only product is not the reference meaning.

Each trap is per node. A successful child publishes its CKIR value for use by
its parent; if a node traps, that node publishes no value, its parents do not
execute, and no dependent store, call, transition, or selected machine result
is published. An implementation may use a wider temporary or exact carry,
borrow, and high-half predicates, but may not reassociate a recursive tree or
move a trap across an authored node.

## 3. Recursive same-carrier relation

A selected arithmetic row may consume a visible leaf value or the successful
result of any earlier selected Add, Subtract, or Multiply row in the same
machine and exact carrier. Thus mixed recursive trees such as
`(a + 1) * (b - 2)` are structural CKIR14 relations, subject to ordinary dense
value, visibility, block, and dominance rules.

Leaves are constants or inherited total loads of the exact selected type, or
an opcode-21 exact widening of a visible pure exact-unqualified-`u8` load into
that type. The widening preserves the operand's `0..=255` mathematical payload,
uses zero flags and immediates, and is itself total and nontrapping.
Calls, indexing, mutation, construction, allocation, user dispatch, mixed
carriers, inferred conversion, and a result with another policy/domain are not
arithmetic operands in this relation. This restriction applies recursively;
wrapping an excluded expression in a selected node does not admit it.

The operation sequence is canonical postorder for producer-backed source:
widenings and arithmetic children precede parents, siblings retain authored
left/right order, and no constant folding, reassociation, commutation,
distribution, or synthetic operator substitution occurs. A general CKIR14
validator does not need source bytes to check operation meaning, but the
OMGLOWF and OMGRFN16 relations own this source-to-row correspondence.

## 4. CKIR12 composition

All inherited CKIR12 view operations are optional as a family in CKIR14. The
presence of a kind-7 type, kind-7 literal root, opcode 22 through 25, or the
bit-0 synthetic block flag selects that family. Once selected, its exact type
and constant-DAG requirements apply, and the four-operation/synthetic-edge
closure required by CKIR12 must be complete. The partial head and tail
operations still execute only in the validated nonempty true-edge block and
still trap without a result on empty input.

Arithmetic values cannot stand in for view descriptors, slice lengths, block
flags, structural IDs, or constant roots. Conversely, view values cannot be
arithmetic operands. The two families may coexist and exchange control only
through already-valid blocks and terminators; neither family grants authority
to relax the other's validation.

## 5. Resources, status, and publication

Each selected arithmetic operation consumes one expression-depth level, one
operation row, two operand words, one dense value, and one four-byte scalar
slot. Each exact widening consumes its inherited one operation, operand word,
dense value, scalar slot, and expression-depth level. CKIR12 ceilings remain
normative: expression depth 8, 32,768 operations,
94,208 operand words, 36,864 values, a 262,144-byte machine frame, the 1-MiB
text bound, constant-DAG and view-literal limits, and the complete CKIR byte
bound. Depth 9 and any other declared resource exhaustion select 252.

Malformed identity, type, semantic word, operation, arity, immediate,
visibility, recursive custody, optional-view closure, or version/cross-pair
relation selects 251. Resource exhaustion selects 252. A runtime arithmetic or
partial-view trap publishes no operation result or dependent observable; it
is distinct from validation status 251. Validation failure and exhaustion
publish no CKIR result, ELF, or partial output.

CKIR14 admits no division, remainder, shifts, signed arithmetic, wrapping or
saturating policy, arbitrary precision, dependent arithmetic, vectorization,
constant folding, or public representation claim.
