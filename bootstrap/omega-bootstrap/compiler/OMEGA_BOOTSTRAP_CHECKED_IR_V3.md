# Omega bootstrap checked IR schema major 3

CKIR schema major 3 is the private, versioned successor for the first general
constant-aggregate tranche. It adds recursively constant scalar, nominal-record,
and fixed-array construction without turning authored aggregate elements into
thousands of place/store operations. It also adds aggregate installation into
mutable storage and unsigned scalar `<=`.

This is not an Omega ABI, a static-data ABI, or an admission of these facilities
to final `Ωself`. It is provisional bridge cost and correctness evidence, not
a source-profile ruling. Except for the overrides below, every CKIR1 rule and
every CKIR2 exact-root, opcode-10 `Call`, role-3-binding, and finite-call-graph
rule in
[`OMEGA_BOOTSTRAP_CHECKED_IR.md`](OMEGA_BOOTSTRAP_CHECKED_IR.md) and
[`OMEGA_BOOTSTRAP_CHECKED_IR_V2.md`](OMEGA_BOOTSTRAP_CHECKED_IR_V2.md) remains
normative. Schema-major-1 and schema-major-2 bytes and meanings remain frozen.

## 1. Versioned lowering frame and CKIR envelope

The resolved-source lowerer consumes `OMGLOW3`, not `OMGLOW1` or `OMGLOW2`:

```text
offset  width  field
0       8      magic: ASCII "OMGLOW3\0"
8       u16    schema major: 3
10      u16    schema minor: 0
12      u16    flags: zero
14      u16    header size: 32
16      u32    exact total frame length
20      u32    exact OMGCOMP length
24      u32    exact OMGRSW1 length
28      u32    reserved: zero
32      ...    exact OMGCOMP || exact OMGRSW1 || exact EOF
```

The component contracts and maximum frame length remain unchanged: at most
267,280 OMGCOMP bytes, 524,288 OMGRSW1 bytes, and 791,600 bytes for the complete
frame. `OMGRSW1` remains version 1. It already retains exact body spans, nominal
type identities, fields, machines, types, and static bindings; constant-body
meaning belongs in the lowerer rather than a producer-selected resolution row.
All three OMGLOW versions reject one another.

The nominal 791,600-byte component sum is not source-realizable. The greatest
canonical `OMGCOMP` is 267,224 bytes. Although the witness row ceilings alone
suggest a 482,784-byte `OMGRSW1`, the resolver's 8,192 raw-type ceiling also
binds every field, machine parameter, block parameter, and typed result. With
4,096 fields, the greatest 889 machine parameters (`127 * 7`, because the
selected entry has none), and one typed selected result, at most 3,206 block
parameters remain. The resulting greatest witness is 461,424 bytes, and all
of these relations are simultaneously realizable:

```text
32 + 267,224 + 461,424 = 728,680 bytes
```

The exact construction fills 16 packages/sources, 32 aliases, 50 used strings,
4,096 bindings and fields, 128 machines, 2,048 blocks, 2,048 normalized types,
and all 8,192 raw types. Adding one trailing source space produces the first
728,681-byte failure. It exceeds both the already-maximal nested bundle and
aggregate source content; canonical preflight selects the nested-bundle extent
first and returns 252 without CKIR publication.

The CKIR magic remains `OMGCKIR\0`; schema major is 3, schema minor is 0, and
target remains 1 (`linux_x86_64`). CKIR consumers reject every other major or
minor. The header grows from 72 to 80 bytes:

```text
offset  width  field
0       8      magic: ASCII "OMGCKIR\0"
8       u16    schema major: 3
10      u16    schema minor: 0
12      u16    target: 1
14      u16    flags
16      u32    exact selected entry machine ID, or NO_ID
20      u32    exact total CKIR byte length
24      u32    type count
28      u32    record count
32      u32    field count
36      u32    machine count
40      u32    machine-parameter count
44      u32    block count
48      u32    block-parameter count
52      u32    operation count
56      u32    operand-vector count
60      u32    terminator count
64      u32    value count
68      u32    place count
72      u32    constant-node count
76      u32    constant-child-vector count
```

Flag bit 0 and the exact selected-root relation are unchanged from CKIR2. All
other bits are zero. Tables follow in this exact order:

1. types;
2. records;
3. fields;
4. machines;
5. machine parameters;
6. blocks;
7. block parameters;
8. constant nodes;
9. the constant-child vector;
10. operations;
11. the operation/terminator operand vector; and
12. terminators.

All inherited row widths are unchanged. A constant node is 24 bytes and each
constant-child entry is one `u32`, so exact encoded length is:

```text
80
+ 24 * type_count
+ 20 * record_count
+ 16 * field_count
+ 36 * machine_count
+ 20 * machine_parameter_count
+ 32 * block_count
+ 20 * block_parameter_count
+ 24 * constant_node_count
+  4 * constant_child_vector_count
+ 40 * operation_count
+  4 * operand_vector_count
+ 44 * terminator_count
```

Checked multiplication and addition precede every access. The result equals
both the header length and exact EOF.

## 2. Admitted source construction

This version admits an aggregate constant only as the right side of assignment
to a mutable structural place. The complete expression is recursively one of:

- a scalar literal checked against its context-selected `u8`, `u32`, or `bool`
  type;
- a nominal-record literal naming every field exactly once, whose children are
  recursively constant for their declared field types; or
- a fixed-array literal with exactly the declared element count, whose children
  are recursively constant for the element type.

The destination supplies the exact root type. Record children are normalized to
declaration-field ordinal and array children to element index. Authored record-
field order therefore does not select wire order. Missing, duplicate, unknown,
or extra fields; wrong array arity; an out-of-range scalar; a wrong nominal or
element type; or any nonconstant child rejects 251 before CKIR publication.
Calls, loads, names, arithmetic, casts, transitions, and other runtime
expressions are not constant children in this version.

The structural root and every structural descendant must be recursively
copyable under the inherited CKIR rules. The completed constant is checked and
interned before an operation can install it. No destination place is partially
updated while source construction is being checked.

An array constant has at most 1,024 children and a record constant has at most
four children in this source tranche. The declared type system retains the
inherited 65,536-element fixed-array and 64-field record ceilings; only literal
construction has the narrower provisional bounds. Thus a larger structural
type may exist and be indexed or copied through inherited operations, but it
cannot be constructed by a CKIR3 constant node.

A type-correct, complete 1,025-element array literal or five-field record
literal selects resource exhaustion 252. A literal whose child count fails to
match its declared type is malformed/unsupported 251 instead. The two cases
must have distinct controls.

Ordinary nested place suffixes require no special Unicode-table operation.
`self.outer[i].inner[j].field` lowers left-to-right through the inherited
`SelfPlace`, `FieldPlace`, and trapping `IndexPlace` rules, followed by `Load`
when a scalar value is required. Every index retains its independent runtime
bounds check.

### 2.1 Guardless transitions and state-edge interval custody

This version admits the ordinary guardless form
`transition { _ -> target(arguments) }`. It has exactly one wildcard arm. For
canonical CKIR construction it emits the inherited `Jump`: value is `NO_ID`,
target 0 and its arguments are present, target 1 is absent, and no operation is
emitted for a synthetic guard. Authored `transition true` remains an authored
guard and follows the inherited `Const(1)` plus `Branch` construction. These
forms have the same Omega result but deliberately have different canonical
CKIR.

Range facts are frontend checking evidence and do not appear in CKIR. CKIR3
closes their state-edge meaning as follows:

- a true arm of `left < right` narrows a retained direct scalar subject on the
  left to at most `right_hi - 1`; if `right_hi` is zero, that arm is
  unreachable;
- a true arm of `left <= right` narrows that subject to at most `right_hi`;
- the applicable arm fact is installed before its argument expressions are
  checked or lowered, so guarded arithmetic and indexing use the narrowed
  interval;
- each scalar edge argument's resulting interval is intersected with the
  declared interval of the target state parameter at the same ordinal; and
- a reachable state parameter receives the least fixed-point convex hull of
  all such incoming intervals. Forwarding therefore changes the fact's subject
  identity to the target parameter instead of retaining the predecessor's
  parameter ID. Joins and cycles are independent of declaration order.

Machine parameters retain their declared intervals. A state with no reachable
incoming edge is checked at its declared parameter intervals, so unreachable
source is not exempt from static validation. Assignment to a retained mutable
subject invalidates its inherited fact before later expressions are checked.
After the monotone state-edge fixed point stabilizes, the lowerer reparses or
replays bodies in inherited canonical order and emits ordinary CKIR operations
and terminators. The exact Unicode loops must consequently preserve
`index <= 690` through `scan → check → upper`, admit `index + 1` on the back
edge, join `0` and the back-edge interval at `scan`, and reapply the `< 691`
true-arm fact before every indexed access.

The inherited block, parameter, terminator, and edge-operand ceilings bound the
finite transfer graph. An implementation must compute the fixed point for every
input within those published bounds; private worklist behavior is not a new
source-visible resource and may not turn traversal or declaration order into a
0/252 distinction.

## 3. Typed semantic constant graph

### 3.1 Constant-node row — 24 bytes

```text
u32  dense constant-node ID
u32  exact CKIR type ID
u32  child-vector start
u32  child-vector count
u32  scalar magnitude, or zero for a structural node
u32  reserved: zero
```

Node kind is derived exclusively from the referenced type:

- a scalar node has no children, uses the current next child-vector index as
  its empty-span start, and carries a magnitude in the exact scalar interval;
- a nominal-record node has one child per field in field-ordinal order, has a
  zero scalar field, and each child has that field's exact type; and
- a fixed-array node has one child per element in index order, has a zero scalar
  field, and every child has the exact element type.

No producer size, alignment, field offset, element stride, byte image, address,
relocation, or padding appears in either table. Constant-child spans partition
the complete child vector in node-ID order, including canonical empty spans.

The graph is an interned DAG, not an authored syntax tree. The backend and
independent checker recompute each node's height: scalars have height zero and a
structural node has one plus the maximum child height. Rows are strictly ordered
by the following key:

```text
(height, type ID, scalar magnitude)                 for a scalar
(height, type ID, child count, child-ID sequence)  for a structural node
```

Keys use ordinary unsigned and lexicographic order. Duplicate keys reject.
Every child consequently has smaller height and a smaller node ID; forward
edges and cycles reject. This order makes interning and bytes deterministic
without using filenames, declaration occurrence counts, or producer traversal
accidents.

Every node must be transitively reachable from at least one opcode-11 root.
Every opcode-11 root is structural. An unused node, child-vector word, or
disconnected subgraph rejects 251.

### 3.2 Canonical source lowering

The source lowerer first type-checks every admitted aggregate assignment,
collects its semantic nodes, interns identical typed nodes, computes the
canonical order above, then assigns IDs and operation references. This pass is
complete before the first CKIR byte is written. It does not emit a `Const`,
`FieldPlace`, `IndexPlace`, or `Store` for each literal leaf.

Within the inherited machine/block/statement order, assignment first lowers
the destination place and then emits one opcode-11 row referring to the already
numbered root. Global graph numbering does not reorder executable operations.

Renaming a declaration or permuting declarations may still change type IDs and
CKIR bytes under inherited semantic ordering. For one exact accepted OMGLOW3
frame, output is byte-identical across runs and implementation routes.

## 4. Operations

Opcodes 1 through 10 retain their complete CKIR2 meanings. In particular,
ordinary `Copy` still snapshots semantic leaves, and `Call` still consumes one
exact role-3 binding and participates in the complete finite acyclic machine
call graph.

### 4.1 Opcode 11: `CopyAggregateConst`

The existing 40-byte operation row encodes `CopyAggregateConst` as follows:

- result kind is 0 and both result fields are `NO_ID`;
- the sole operand is a destination place;
- immediate 0 is a constant-node ID and immediate 1 is zero;
- that node is a structural root with the destination place's exact type;
- the type is recursively copyable; and
- the destination place is mutable.

This operation installs one already-completed semantic value. It does not make
the constant graph addressable from Omega source and produces no structural
value, reference, place, or pointer.

### 4.2 Opcode 12: `LessEqual`

`LessEqual` has result kind 1, two value operands `(left,right)`, and zero
immediates. Operands are carrier-compatible `u8` or carrier-compatible `u32`;
the result is canonical `bool`; comparison is unsigned. Its type and visibility
rules are exactly those of inherited `Less` except for the inclusive relation.

Canonical source evaluation follows the inherited comparison order: lower the
left operand's place-producing suffixes, then the right operand's suffixes;
materialize the right before the left; then emit `LessEqual(left,right)`. It is
not normalized to a negated or operand-reversed `Less`, because doing so would
change canonical materialization and trap order.

No other opcode, operand count, result shape, flag, or immediate is valid.

## 5. Backend reconstruction and conservative artifact

### 5.1 Layout and constant image

The backend recomputes the inherited private scalar, array, and record layout
from types, fields, and ordinals. It rejects recursive layout or any mismatch
before publication. It then finds the distinct opcode-11 roots in increasing
constant-node ID and materializes one private image object for each root:

1. align the current image cursor to the root type's independently derived
   alignment;
2. initialize inter-object and internal padding to zero;
3. write each scalar leaf at its independently derived field/index offset in
   little-endian target representation; and
4. advance by `max(independently derived complete type size, 1)` so an empty
   structural root has a deterministic address anchor without acquiring a
   semantic byte.

Identical typed roots share one interned node and therefore one image object.
Child nodes that are not operation roots do not receive separate image objects.
The image offset of every root is derived by this algorithm and never supplied
by the producer. The source-to-CKIR and CKIR-to-artifact checkers independently
reconstruct the same bytes from semantic magnitudes and types.

`CopyAggregateConst` has the same observable result as assigning all semantic
leaves of the completed literal to the destination. CKIR has no operation that
observes padding, so the conservative backend may copy the root's complete
derived object image, including canonical zero padding. The constant source is
read-only and cannot alias mutable owner storage. Existing opcode-7 `Copy`
retains its older snapshot/semantic-leaf rule unchanged.

### 5.2 ELF segments

When there are no constants, `constant_node_count` and
`constant_child_vector_count` are both zero, opcode 11 is absent, and the
inherited two-segment CKIR2 ELF relation remains exact.

When an opcode-11 root exists, the resulting constant image is nonempty under
the address-anchor rule above and the deterministic ELF has three
`PT_LOAD` segments and no section headers, interpreter, dynamic entries,
relocations, symbols, imports, or debug information:

- RX begins at file offset and virtual address `0` relative to image base
  `0x400000`. The ELF header and three program headers occupy the beginning,
  bytes through offset 4,095 are zero, text begins at `0x401000`, and the
  segment is page-padded exactly as in CKIR2;
- R begins at file offset `rx_file_size` and virtual address
  `0x400000 + rx_file_size`. It contains the exact derived constant image
  followed only by zero page padding. Its file and memory sizes are the image
  length rounded up to 4,096, its flags are 4, and it is not writable or
  executable; and
- RW begins at the first page after R, has file size zero, and has the inherited
  page-rounded zero-fill size for the selected owner. Its flags are 6.

All segment offsets and virtual addresses are page-aligned and
`p_paddr == p_vaddr`. EOF follows the padded R segment. The selected entry,
reachable-machine closure, private call ABI, block order, frame rules, shared
trap, and exit projection remain those of CKIR2. Library CKIR still publishes
no ELF.

Let `CONST(c)` be the independently derived virtual address of root `c` and
`SIZE(t)` its independently derived layout size. Existing CKIR2 instruction
templates remain exact. The two new operation templates are:

| Operation | Canonical bytes, in order |
| --- | --- |
| `CopyAggregateConst` | `LP(destination); 49 89 C2; 48 8D 35 REL(CONST(root)); 4C 89 D7; B9 SIZE(type); F3 A4` |
| `LessEqual` | `LV(left); 3B 85 -V(right); 0F 96 C0; 0F B6 C0; SV(result)` |

The copy uses `rep movsb` only after the complete graph, layout, image, text,
segment, and output extents have passed preflight. CKIR3 has no threads,
exceptions, or observable intermediate aggregate-write state. The canonical
backend never sets the direction flag. Every RIP-relative constant reference
and inherited rel32 branch/call displacement is sized and rechecked in both
emission passes.

## 6. Resources, status, and publication

All inherited CKIR2 source, declaration, type, machine, control-flow, value,
place, operand, layout, frame, and text ceilings remain in force. CKIR3 adds:

| Resource | Ceiling |
| --- | ---: |
| constant nodes | 8,192 |
| constant-child-vector words | 16,384 |
| children in one array constant | 1,024 |
| children in one record constant | 4 |
| complete derived constant-image payload | 131,072 bytes |
| encoded CKIR3 bytes | 2,522,192 |
| entry-bearing ELF bytes | 1,183,744 |

The CKIR byte ceiling is the inherited 2,260,040-byte CKIR2 structural maximum,
with its 72-byte header replaced by 80 bytes, plus `8,192 * 24` constant-node
bytes and `16,384 * 4` child words. The ELF ceiling is the inherited maximum
1,052,672-byte padded RX file plus at most 131,072 bytes of page-aligned
read-only image. These are aggregate maxima, not permission to bypass the
narrower source-literal, source-byte, statement, expression-depth, type-layout,
frame, or text limits.

The selected entry-machine frame can reach 262,128 bytes: the mandatory
16-byte root live-stack allowance makes the next realizable 262,144-byte frame
exhaust the 262,144-byte live-stack ceiling. Canonical text can reach exactly
1,048,576 bytes; the next realizable operation mix is 1,048,587 bytes. A
canonical entry artifact can simultaneously reach the 1,183,744-byte ELF
ceiling with exact RX and read-only-image maxima. The isolated image and text
overages select 252 before a larger canonical ELF can be published; a fabricated
1,183,745-byte output is therefore not an adjacent positive candidate.

For refinement observations that execute cyclic state control, the source-only
evaluator retains at most 16 active machine frames and the CKIR-only evaluator
retains at most 64. Each independently permits at most 65,536 dynamic block
entries, including machine-entry blocks; attempting the 65,537th returns 252
without a claimed result. These are evidence-storage/work ceilings, not a fuel
counter inserted in the emitted ELF, a recursion admission, or permission to
accept a cyclic machine-call graph. Native execution retains ordinary Omega
loop meaning.

Status 0 means complete success. Malformed framing, noncanonical tables,
invalid graph order/reachability/type/arity, nonconstant source, static type or
mutability failure, unsupported construction, recursive layout, or target
mismatch returns 251. A validated extent above a source, graph, CKIR, constant
image, layout, frame, text, ELF, or evaluator ceiling returns 252. Arithmetic
overflow while decoding a purported encoding is 251 unless an already validated
public extent selects 252. Status is monotonic once 252 is selected.

The lowerer emits no CKIR byte until source checking, graph construction,
interning, canonical ordering, all resource checks, exact EOF, and complete
output sizing succeed. The backend emits no ELF byte until full CKIR validation,
reachable-call closure, layout, image, frame, text, displacement, segment, and
EOF preflight succeeds. Status 251 or 252 always has empty stdout.

## 7. Explicit exclusions and non-authority

CKIR3 does not add runtime aggregate constructors, aggregate locals, structural
returns, aggregate call results, aggregate transition literals, named constant
references, constant evaluation of calls/arithmetic/casts, array repetition or
spread forms, payload sums, slices, strings, allocation, pointers, recursion,
field receivers, imported machine calls, cross-package calls, or private access
between distinct logical modules. It does not change the inherited exclusion of
generics, domains, proofs, boundaries, atomics, threads, exceptions, or full-
width authored `u32` beyond the signed-D0 carrier.

The read-only image is private implementation data. Its offsets, padding,
deduplication, segment address, and bytes are not source-observable layout,
wire identity, hashing input, FFI, or a product ABI. This contract makes no
decision that records, arrays, generated data, or `<=` belong to final
`Ωself`; it supplies the general bridge implementation and assurance cost
needed for that later profile decision.

No new source-language ruling is made here. Declaration-order canonicalization
of constant record fields and ordinary fixed-integer `<=` are existing Omega
behavior. Whether final `Ωself` retains aggregate literals, wider aggregate
expression contexts, or noncopyable aggregate construction remains open at the
completed source/bridge join. Private access between distinct logical modules
also remains unresolved and rejected; the selected source and harness
contribute to one logical module and do not depend on that ruling.

## 8. Required evidence before use as an artifact tranche

All CKIR2 producer, self-build, Rust-free meaning, mutation, and lower-rooted
obligations remain mandatory. CKIR3 additionally requires:

1. The exact standalone
   `source/compiler/omega/psi/generated/unicode_tables.omg` bytes plus a same-logical-module
   result harness compile through OMGLOW3 and produce a deterministic ELF that
   exits 70 with empty stdout/stderr. The harness exercises initialization,
   first/last elements, present/absent start and continue lookups, looped state
   control, nested array/record indexing, Unit calls, and `<=`.
2. Renamed, declaration-reordered, authored-field-reordered, smaller, and
   recursively nested record/array fixtures take the same general path and
   preserve ordinary Omega meaning. No fixture or producer branch recognizes a
   filename, Unicode version, 1,497 literal count, or 691/806 table lengths.
3. Guardless-transition controls require canonical `Jump` bytes and reject a
   semantically equivalent synthetic-true `Branch` as noncanonical.
   An authored `transition true` remains an ordinary `Branch` and must not be
   collapsed into the guardless encoding. Focused cyclic controls require
   arm-local `<` and `<=` narrowing, ordinal argument-to-parameter interval
   transfer, predecessor joins, forwarding, declaration reordering, and the
   exact `scan → check → upper → scan(index + 1)` recurrence. A stale
   predecessor parameter ID, a fact applied only after argument checking, a
   missing predecessor, or a declaration-order-dependent result must reject or
   disagree with the canonical relation.
4. Phase-isolated source negatives cover missing/duplicate/unknown record
   fields, scalar and structural type mismatch, array arity, an out-of-range
   literal, a nonconstant child, a noncopyable root, shared-place mutation,
   recursive layout, and malformed `<=` operands. Each is 251 with no CKIR.
   In the admitted named-record grammar, an extra field is necessarily unknown
   or a duplicate and record-arity disagreement is witnessed by the isolated
   missing/duplicate/unknown cases; there is no separate positional-record
   arity form to manufacture. Oversized but otherwise valid layout is resource
   exhaustion and belongs to item 6, not this malformed-source matrix.
5. Independent CKIR negatives mutate every new header count, constant ID, type,
   span, child, scalar, reserved word, ordering key, duplicate, reachability,
   opcode, operand, immediate, result shape, root/type relation, image byte,
   image offset, segment field, RIP-relative displacement, and `setbe` byte.
   Valid-but-mismatched CKIR/image/result cross-pairs reject.
6. Exact and adjacent resource teeth cover 1,024/1,025 array children, four/five
   record children, source-unit and aggregate source bytes, node and
   child-vector capacities, derived image bytes, encoded CKIR bytes, the
   791,600-byte OMGLOW3 frame, selected-owner layout including its first
   oversized valid form, machine frame, text, ELF image, active evaluator
   frames, and 65,536/65,537 dynamic block entries.
   Where relations make a nominal aggregate maximum unrealizable, evidence
   proves the greatest realizable boundary and its first adjacent failure
   rather than manufacturing a noncanonical positive.
7. Native and persisted-Delta-self-built lowerers/backends publish identical
   CKIR and ELF bytes for positives and agree on complete 0/251/252
   observations. The Rust-free Delta-to-Gamma meaning route independently
   checks the same semantic graph, result, and exhaustion boundaries. A Rust
   product compiler may remain differential evidence only.
8. Lower-rooted CKIR3 refinement uses the distinct `OMGRFN4` version-4 carrier
   and exact 4,497,544-byte simultaneous ceiling defined by
   [`OMGCOMP_REFINEMENT_WITNESS_V4.md`](../../../source/assurance/refinement/omega-bootstrap/OMGCOMP_REFINEMENT_WITNESS_V4.md).
   The five logical responsibilities and the constant-table/source-root/
   artifact-root ownership boundary are normative there. Earlier OMGRFN
   carriers and checkers remain frozen and reject CKIR3.

Only that evidence closes the selected bounded, finite-call, returning
source-to-artifact relation. It does not grant compilation authority without
the separately accepted lock/closure and exact OMGCOMP commitment join.
