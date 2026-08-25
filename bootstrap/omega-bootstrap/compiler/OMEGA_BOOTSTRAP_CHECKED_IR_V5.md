# Omega bootstrap checked IR schema major 5

[`CKIR4`](OMEGA_BOOTSTRAP_CHECKED_IR_V4.md) |
[`OMGRSW3`](OMEGA_BOOTSTRAP_RESOLUTION_V3.md) |
[`OMGRFN7`](../../assurance/refinement/omega-bootstrap/OMGCOMP_REFINEMENT_WITNESS_V7.md) |
[conservative backend evidence](OMEGA_BOOTSTRAP_CHECKED_IR_V5_BACKEND.md)

CKIR schema major 5 is the private successor for the first payload-bearing pure-
sum tranche. It adds declaration-owned sums, cases, payload fields, immutable
runtime case construction, exhaustive case dispatch, and selected-edge payload
binding. Except where this document overrides it, all CKIR1 through CKIR4
typing, call, constant, runtime-record, resource, status, and artifact rules
remain normative. Earlier schema identities and bytes remain frozen.

This is bridge-cost and refinement evidence. It is not an Omega ABI, a wire
schema, or final admission of sums to `Ωself`.

## 1. OMGLOW6 input and CKIR5 envelope

The resolved-source lowerer consumes only this frame:

```text
offset  width  field
0       8      magic: ASCII "OMGLOW6\0"
8       u16    schema major: 6
10      u16    schema minor: 0
12      u16    flags: zero
14      u16    header size: 32
16      u32    exact total frame length
20      u32    exact OMGCOMP length
24      u32    exact OMGRSW3 length
28      u32    reserved: zero
32      ...    exact OMGCOMP || exact OMGRSW3 || exact EOF
```

Component ceilings remain 267,280 OMGCOMP bytes and 524,288 OMGRSW3 bytes, so
the complete frame remains at most 791,600 bytes. `OMGLOW6` and every earlier
OMGLOW identity reject one another. The shared lowerer may dispatch versions,
but an OMGLOW4/5 input still produces byte-identical CKIR4.

CKIR5 retains magic `OMGCKIR\0`, target 1 (`linux_x86_64`), and schema minor 0.
Its 100-byte header is:

```text
offset  width  field
0       8      magic: ASCII "OMGCKIR\0"
8       u16    schema major: 5
10      u16    schema minor: 0
12      u16    target: 1
14      u16    flags
16      u32    selected entry machine ID, or NO_ID
20      u32    exact total CKIR length
24      u32    type count
28      u32    record count
32      u32    ordinary field count
36      u32    sum count
40      u32    case count
44      u32    case-payload-field count
48      u32    machine count
52      u32    machine-parameter count
56      u32    block count
60      u32    block-parameter count
64      u32    constant-node count
68      u32    constant-child-vector count
72      u32    operation count
76      u32    ordinary operand-vector count
80      u32    terminator count
84      u32    case-arm count
88      u32    case-arm-argument count
92      u32    value count
96      u32    place count
```

Flag and entry relations are inherited. Tables follow in this exact order:

1. types; 2. records; 3. ordinary fields; 4. sums; 5. cases; 6. case-payload
fields; 7. machines; 8. machine parameters; 9. blocks; 10. block parameters;
11. constants; 12. constant children; 13. operations; 14. ordinary operands;
15. terminators; 16. case arms; and 17. case-arm arguments.

The exact encoded length is:

```text
100
+ 24 * type_count
+ 20 * record_count
+ 16 * field_count
+ 20 * sum_count
+ 20 * case_count
+ 16 * case_payload_count
+ 36 * machine_count
+ 20 * machine_parameter_count
+ 32 * block_count
+ 20 * block_parameter_count
+ 24 * constant_node_count
+  4 * constant_child_count
+ 40 * operation_count
+  4 * operand_count
+ 52 * terminator_count
+ 24 * case_arm_count
+ 12 * case_arm_argument_count
```

Checked arithmetic precedes every access; the result equals the header length
and exact EOF.

## 2. Sum declarations, types, and private layout

Type kind 6 is `nominal sum`: payload 0 is a sum ID, payload 1 and both range
endpoints are zero, and flags are zero. Every sum has exactly one nominal type
and vice versa. Record and sum counts are separately encoded but their sum is
at most 128. Machines remain attached only to ordinary records in this tranche.
Nominal records occupy the first `record_count` type IDs in record-ID order;
nominal sums occupy the next `sum_count` IDs in sum-ID order. The canonical
`bool` and full admitted `u32` rows immediately follow that combined prefix.

### Sum row — 20 bytes

```text
u32  dense sum ID
u32  nominal type ID
u32  case start
u32  case count
u8   flags: bit 0 is checked `[copy]`
u8   reserved[3]: zero
```

### Case row — 20 bytes

```text
u32  dense case ID
u32  owner sum ID
u32  ordinal and private tag within owner
u32  payload-field start
u32  payload-field count
```

### Case-payload-field row — 16 bytes

```text
u32  dense payload-field ID
u32  owner case ID
u32  ordinal within owner
u32  type ID
```

Spans are canonical partitions in owner order. A sum has 1..64 cases; a case
has 0..4 payload fields. The first case has ordinal/tag zero. `[copy]` and the
by-value acyclicity graph recur through arrays, records, sums, and every case
payload. Copy and Call structural checks accept kind 6 by exact type ID when it
is recursively copyable.

CKIR5 derives one private layout and accepts no producer offsets:

- the tag is a 4-byte unsigned value at offset zero;
- each case lays out its payload fields in declaration order using inherited
  scalar/aggregate size and alignment rules, with a case-local cursor at zero;
- payload alignment is the maximum alignment of every case payload, or one;
- the payload overlay begins at `align_up(4, payload_alignment)`;
- payload size is the maximum rounded case-payload size;
- sum alignment is `max(4, payload_alignment)`; and
- sum size is `align_up(payload_offset + payload_size, sum_alignment)`.

The all-zero representation is case zero with a recursively zeroed payload.
Zero establishment therefore recurs only through case zero. No niche
optimization is permitted. Inactive payload and padding bytes have no semantic
value, are never inspected, and may remain stale. This chosen layout is solely
the checked bridge/backend relation; it is not the unique default Omega layout
or a public ABI.

## 3. Admitted source slice

The lowerer admits unnumbered, nongeneric, pure sums with payload-free and
payload-bearing cases. Construction uses canonical Omega spelling:

```omega
TokenKind::Identifier
TokenKind::Float {
    has_exponent: true,
    empty_exponent: false,
    has_suffix: false,
}
```

Every named payload field appears exactly once. Fields are bound by name and
then materialized in payload declaration order. Runtime payload expressions use
the same pure, nontrapping leaves admitted for CKIR4 runtime record fields:
scalar literals, machine/block parameters, scalar `self`/field places followed
by Load, structural parameters, and nested admitted record/case constructors.
Effectful or potentially trapping payload expressions reject. A source-valid
fifth payload field selects 252 after semantic validation; missing, duplicate,
unknown, extra, mistyped, or noncopyable payloads select 251.

Payload-free construction is still an operation rather than a scalar tag
constant: sum values remain exact structural values. CKIR3's constant graph is
unchanged and does not acquire sum nodes.

Case dispatch admits a structural parameter/value or a `self`-rooted sum place,
requires exhaustive exact-case coverage, and evaluates its subject once.
Patterns may bind named payload fields and pass a binding directly as the
corresponding target-state argument. Other admitted arm arguments are existing
machine/block parameters or scalar literals; no call, load, arithmetic, cast,
index, nested aggregate literal, trap, mutation, or order-observable expression
is evaluated conditionally in this first slice. Renaming/rest/data-field
patterns, domain arms, guards, and aggregate transition literals remain out.

Source arm order does not select CKIR order for disjoint exact cases. The
lowerer emits one arm per case in case declaration order. Missing, duplicate,
wrong-owner, nonexhaustive, or payload-incomplete arms reject before output.

## 4. Opcode 14: `ConstructCase`

The inherited 40-byte operation row encodes opcode 14 as follows:

- result kind is value and result type is exact kind-6 nominal sum;
- immediate 0 is the selected case ID and immediate 1 is zero;
- the case belongs to the result sum;
- operands are visible values in payload declaration order;
- operand count equals the selected case's payload count and is at most four;
- scalar operands are carrier-compatible and their encoded intervals are
  contained in the exact payload-field interval;
- structural operands have the exact recursively copyable payload type; and
- the result is one completed immutable address-backed object.

The result may feed inherited structural `Copy`, inherited structural `Call`, a
nested admitted constructor, or a CaseDispatch subject. It is not a place,
pointer, structural return, scalar-load source, or identity-bearing object.
Each result receives a distinct frame-owned object in value-ID order, with the
same complete-invocation lifetime and frame/live-stack accounting as opcode 13.
Construction stores the tag and active semantic payload leaves before
publishing the result address; it does not inspect or promise inactive bytes.

## 5. CaseDispatch and selected-edge arguments

The CKIR5 terminator row is 52 bytes: the inherited 44-byte row followed by:

```text
u32  case-arm start
u32  case-arm count
```

For inherited terminator kinds 1..4, flags/reserved remain zero, the arm count
is zero, and the arm start is the next unconsumed case-arm index. Their ordinary
operand spans retain the inherited partition. Terminator kind 5 is
`CaseDispatch`:

- flags are `1 = structural value subject` or `2 = place subject`; reserved is
  zero;
- the inherited value field contains the subject value/place ID;
- the subject has exact nominal-sum type;
- both inherited target IDs are `NO_ID`, and all four inherited ordinary-edge
  start/count fields denote empty spans at the current ordinary operand cursor;
- its case-arm span contains exactly the subject sum's case count; and
- arms occur in case ordinal order and cover every case exactly once.

### Case-arm row — 24 bytes

```text
u32  dense case-arm ID
u32  owner terminator ID
u32  exact case ID
u32  target block ID
u32  case-arm-argument start
u32  case-arm-argument count
```

The target is a non-entry block in the same machine. Argument count equals the
target block's parameter count. Arm spans partition the arm table in
terminator-ID order; argument spans partition their table in arm-ID order.

### Case-arm-argument row — 12 bytes

```text
u32  dense case-arm-argument ID
u8   source kind: 1 ordinary value, 2 selected-case payload field
u8   reserved[3]: zero
u32  reference ID
```

Kind 1 names a value visible at the owner block's end. A structural kind-1
value produced by opcode 13 or 14 remains prohibited as a direct state-edge
argument. Source lowering in this tranche emits kind 1 only for an existing
machine/block parameter or scalar `Const`.

Kind 2 names an exact payload-field ID belonging to that arm's case. It is
materialized only after the runtime tag selected that arm. A scalar payload is
range-checked and staged by value. A structural payload is copied by semantic
leaves into a frame-owned immutable binding snapshot associated with the arm-
argument row, and that snapshot address is staged. Snapshot extents are aligned,
distinct, live for the machine invocation, and count toward frame/live-stack
limits. This prevents inactive-payload reads and prevents later mutation of a
sum place from changing the bound value.

All case-edge arguments are staged before target parameters are committed, so
parallel transfer remains simultaneous. The backend checks the runtime tag is
within the declared case roster before selecting an arm; an impossible tag
traps rather than indexing an unchecked table.

## 6. Artifact and meaning relation

The backend independently reconstructs every tag, payload offset, semantic
leaf, constructor object, binding snapshot, frame displacement, edge transfer,
block displacement, instruction length, and ELF extent. No producer address or
layout byte is trusted. Case dispatch uses declaration-order unsigned tags and
emits one deterministic comparison/selected-edge sequence in arm order; exact
x86-64 templates and displacement sizing are part of the implementation gate
before CKIR5 artifact closure is claimed.

Semantic Copy of a sum retains the inherited snapshot rule: it first validates
and stages the tag and selected case's semantic payload leaves, then commits
the destination tag and active leaves. Overlap cannot expose a partially
changed case. Call arguments preserve the same exact structural value; a
callee parameter may dispatch it. Equality, hashing, public representation, and
inactive bytes do not enter this tranche. CKIR4 inputs continue to reconstruct
byte-identical CKIR4 ELF artifacts.

## 7. Resources, status, and publication

All inherited operation, ordinary operand, value, place, block, constant,
layout, image, text, ELF, frame, live-stack, call-depth, and dynamic block-entry
limits remain. New table limits are:

| Resource | Ceiling |
| --- | ---: |
| records plus sums | 128 |
| cases per sum / total cases | 64 / 4,096 |
| payload fields per case / total payload fields | 4 / 4,096 |
| ordinary fields plus payload fields | 8,192 |
| case arms per dispatch / total case arms | 64 / 4,096 |
| case-arm arguments | 94,208 minus ordinary operand words |

The complete encoded CKIR remains capped at 2,522,192 bytes. These row ceilings
are aggregate bounds within that byte cap, not permission to realize their
Cartesian product. Constructor and payload-binding objects are ordinary frame
and live-stack contributors rather than new storage budgets.

Malformed framing/tables, invalid identities/spans/order, a bad type/copy/call/
dispatch relation, unsupported source form, or inherited
semantic failure selects 251. A validated public extent above a stated source,
table, encoded-byte, layout, frame, live-stack, text, image, ELF, or evaluator
ceiling selects 252. Status is monotonic after 252. Neither lowerer nor backend
publishes bytes until complete semantic and resource preflight succeeds; 251 or
252 always has empty stdout.

## 8. Required evidence and non-expansion

The positive carrier must compose payload-free and payload-bearing cases,
one-to-four recursively copyable payload fields, nested copyable structure,
runtime construction, Copy, a single structural Call argument, dispatch of a
sum parameter and a nonzero-offset `self` field, and payload binding into exact
result 70. A product-shaped three-Boolean `TokenKind` case and a compact
`ByteRead::Byte { value }`-style dispatch must be observed without recognizing
their names or files.

Native, Delta-self-built, mixed producer/backend, independent CKIR interpreter,
persisted lower-rung meaning, and lower-rooted source-to-artifact routes must
agree. Negatives isolate every new count/span/owner/ordinal/type/reserved field,
cross-version pair, unknown/duplicate/missing construction or arm, inactive
payload observation, invalid tag, four/five and 64/65 resource teeth, and
0/251/252 publication silence.

CKIR5 does not add explicit discriminants, mixed/generic sums, stable/public sum
layout, sum constants, structural returns, sum equality, domains, guards,
renaming/rest patterns, effectful/trapping payload or arm expressions, imported
calls, or a ruling on call-argument evaluation order. It is one bounded general
bridge slice, not a source whitelist or final `Ωself` decision.
