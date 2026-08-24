# Omega-bootstrap private checked IR, version 1

`CKIR1` is the first private handoff from the Delta-written Omega source
frontend to its conservative native backend. It exists so the bridge can lower
ordinary records, fixed arrays, attached machines, and checked mutation without
adding those operations to Terminal Psi. It is not an Omega source format,
Terminal-Psi vocabulary, product IR, stable external ABI, object format, or
additional language/profile inventory.

This document is normative for every byte accepted as CKIR1. A producer may use
different internal tables, but it must publish exactly this encoding. A backend
must reconstruct and validate every relation below before publishing an artifact.
Possession of well-formed CKIR1 is not compiler authority: the source-to-CKIR and
CKIR-to-artifact refinements remain independently checked obligations.

The first producer is the existing source-custody checker in two disjoint input
modes. A raw Omega unit retains its checker-only contract: success exits 0 with
empty stdout. Input beginning with the canonical `OMG0BNDL` magic is decoded as
the exact bridge source-bundle version 1 contract, must contain exactly one
source unit, and publishes CKIR1 on success. The bundle label and exact source
bytes remain source custody; no label or source span is copied into CKIR1.

## 1. Scalar conventions and envelope

All multibyte integers are unsigned little-endian. `u8`, `u16`, and `u32` below
mean encoded widths, not authored Omega types. All counts, lengths, offsets, IDs,
range endpoints, and literal magnitudes must be at most `2^31 - 1`, so the D0
producer and backend can validate them with checked `i32` arithmetic. The
distinguished absent ID is `NO_ID = 0xffffffff`; it is permitted only where this
document says so and is never a dense table ID.

The fixed 72-byte header is:

```text
offset  width  field
0       8      magic: ASCII "OMGCKIR\0"
8       u16    schema major: 1
10      u16    schema minor: 0
12      u16    target: 1 = Linux x86-64 System V
14      u16    flags
16      u32    conformance entry machine ID, or NO_ID
20      u32    total CKIR byte length, including this header
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
```

Flag bit 0 is `HAS_CONFORMANCE_ENTRY`. Bits 1 through 15 are zero. Bit 0 is set
exactly when the entry field is not `NO_ID`. The target and both schema numbers
must match exactly; there is no compatible-minor-version rule in CKIR1.

The tables immediately follow the header in this order, with no padding:

1. type rows;
2. record rows;
3. field rows;
4. machine rows;
5. machine-parameter rows;
6. block rows;
7. block-parameter rows;
8. operation rows;
9. the operand vector; and
10. terminator rows.

The encoded length is therefore exactly:

```text
72
+ 24 * type_count
+ 20 * record_count
+ 16 * field_count
+ 36 * machine_count
+ 20 * machine_parameter_count
+ 32 * block_count
+ 20 * block_parameter_count
+ 40 * operation_count
+  4 * operand_vector_count
+ 44 * terminator_count
```

Every multiplication and addition is checked before use. The computed length
must equal the header length and exact EOF; truncation, overflow, padding, and
trailing bytes are malformed.

## 2. IDs, spans, and canonical order

Every table with an `id` field uses the dense IDs `0..count-1`, and row `i` must
encode ID `i`. All `start,count` spans are checked with subtraction-shaped bounds
before addition, lie within the named table, and are canonical contiguous
partitions in the order specified below. A zero-length span uses the next
unconsumed table index as its start; it may not point elsewhere merely because no
row would be read.

Source names and spans do not enter CKIR1. Their source-order-independent
resolution is already a frontend obligation. CKIR IDs are assigned from the
frontend's resolved semantic order:

- records and their fields use resolved data-declaration order and field order;
- machines use resolved machine order;
- blocks use each machine's entry block followed by its resolved named-state
  order;
- operations retain source evaluation order within a block.

The same accepted source bytes must therefore produce identical CKIR bytes.
Renaming or declaration permutation must preserve behavior but is not required
to preserve CKIR bytes.

## 3. Type, record, and field tables

### 3.1 Type row — 24 bytes

```text
u32  id
u8   kind
u8   flags
u16  reserved, zero
u32  payload 0
u32  payload 1
u32  inclusive range low
u32  inclusive range high
```

Type kinds and payloads are:

| Tag | Kind | Payload 0 | Payload 1 | Range |
| ---: | --- | --- | --- | --- |
| 1 | `u8` | zero | zero | valid scalar interval, within `0..=255` |
| 2 | `u32` | zero | zero | valid scalar interval, within `0..=2147483647` |
| 3 | `bool` | zero | zero | exactly `0..=1` |
| 4 | nominal record | record ID | zero | both zero |
| 5 | fixed array | element type ID | literal element count | both zero |

Flag bit 0 is `TRAPPING`; all other bits are zero. It is permitted on `u8`,
`u32`, and fixed-array types and must be zero on `bool` and nominal records.
Scalar range low must not exceed range high. Fixed-array length is at most
65,536; its element type is a scalar, record, or fixed array. CKIR1 does not
encode full-width authored `u32` constants or endpoints above `2^31 - 1`.

Type rows are interned: no two rows may have the same kind, flags, payloads, and
ranges. Every record has exactly one nominal type row and every nominal type row
names exactly one record.

### 3.2 Record row — 20 bytes

```text
u32  id
u32  nominal type ID
u32  field start
u32  field count
u8   flags
u8   reserved[3], all zero
```

Flag bit 0 is the authored and checked `[copy]` property; all other bits are
zero. The nominal type must be kind 4 and name this record. Field spans partition
the field table in record-ID order. Each record has at most 64 fields.

A record marked `[copy]` is valid only if every field is recursively copyable.
Scalars are copyable; a fixed array is copyable exactly when its element is; a
record is copyable exactly when its record row has bit 0 and all of its fields are
copyable. A record may remain unmarked even when its fields would permit copying.

### 3.3 Field row — 16 bytes

```text
u32  id
u32  owner record ID
u32  ordinal within owner
u32  type ID
```

The owner must be the record whose field span contains the row. Ordinals start at
zero and are dense within that span. Field identity is the pair `(owner,ordinal)`;
no byte offset is accepted from the producer.

The graph formed by record fields and fixed-array elements must be acyclic when
followed through by-value record/array edges. A recursive by-value layout is
unsupported rather than assigned an arbitrary size.

## 4. Machines, blocks, and parameters

### 4.1 Machine row — 36 bytes

```text
u32  id
u32  owner record ID
u8   receiver access
u8   flags, zero
u16  reserved, zero
u32  result type ID, or NO_ID for Unit
u32  machine-parameter start
u32  machine-parameter count
u32  block start
u32  block count
u32  entry block ID
```

Receiver access is `1 = shared` or `2 = mutable`. The result is Unit or a scalar
type; CKIR1 has no structural return. A machine has at most seven explicit
parameters: the implicit receiver is the eighth public source-profile parameter.
Machine-parameter spans partition that table in machine-ID order.

Block spans partition the block table in machine-ID order. Every machine has at
least one and at most 128 blocks. Its entry block is the first row in its block
span, has no block parameters, and uses the machine's receiver access.

### 4.2 Parameter row — 20 bytes

Machine-parameter and block-parameter tables use the same row encoding:

```text
u32  id within this parameter table
u32  owner machine ID (machine parameters) or owner block ID (block parameters)
u32  ordinal within owner
u32  type ID
u32  value ID
```

Each parameter table has its own dense row IDs. Owner spans and ordinals must
agree. A block has at most seven explicit parameters because its receiver is
implicit. Parameter types may be scalar or copyable structural types.

The value IDs of machine parameters are assigned first in machine-parameter row
order, followed by block parameters in block-parameter row order. Thus parameter
row `i` in the first table has value ID `i`, and row `i` in the second has value
ID `machine_parameter_count + i`.

Scalar values are the scalar bits. A structural parameter value is an immutable,
address-backed value of its exact type; CKIR1 permits it only as the source of a
`Copy` operation. It cannot be loaded as a scalar, returned, indexed directly, or
used as a place.

### 4.3 Block row — 32 bytes

```text
u32  id
u32  owner machine ID
u8   receiver access
u8   flags, zero
u16  reserved, zero
u32  block-parameter start
u32  block-parameter count
u32  operation start
u32  operation count
u32  terminator ID
```

Block receiver access is `1 = shared` or `2 = mutable` and may not exceed its
machine's access. Block-parameter spans partition that table in block-ID order.
Operation spans partition the operation table in block-ID order. A block has at
most 32 source statements; operation count is separately bounded because one
statement can lower to multiple place and value operations.

There is exactly one terminator per block. Consequently terminator count equals
block count, a block's terminator ID equals its block ID, and terminator row `i`
names block `i`.

## 5. Values, places, operations, and operands

CKIR1 is stackless. A value is immutable. A place is a typed address rooted only
in the current machine's implicit receiver. Places never escape a block and are
not edge arguments.

After parameter value IDs, value-producing operations receive the next dense
value IDs in operation-row order. Place-producing operations receive dense place
IDs from zero in operation-row order. The header counts must equal those
reconstructions; IDs may not be sparse or producer-selected.

A value operand is visible only when it is:

- a parameter of the current machine;
- a parameter of the current block; or
- the result of an earlier operation in the current block.

A place operand must be the result of an earlier operation in the same block.
These rules are checked even in unreachable blocks.

Two scalar types are carrier-compatible exactly when they have the same kind
tag; their interval endpoints need not be identical. Structural types agree
only by exact type ID. Whenever a carrier-compatible value crosses into a
narrower destination type, the backend checks the destination interval before
committing the value. This relation preserves the frontend's compositional
range judgment without pretending that distinct constrained scalar types are
identical.

### 5.1 Operation row — 40 bytes

```text
u32  id
u32  owner machine ID
u32  owner block ID
u8   opcode
u8   result kind
u16  flags, zero
u32  result ID, or NO_ID
u32  result type ID, or NO_ID
u32  operand start
u32  operand count
u32  immediate 0
u32  immediate 1
```

Result kind is `0 = none`, `1 = value`, or `2 = place`. A no-result operation
uses `NO_ID` for both result fields. A producing operation uses its reconstructed
dense ID and exact result type. Operation owner fields must agree with the block
span containing the row.

The operand vector is a flat table of `u32` references. Operation operand spans,
in operation-ID order, consume the first part of that vector without gaps or
overlap. Terminator edge-argument spans consume the remainder as defined in
section 6.

CKIR1 opcodes are:

| Tag | Operation | Result | Operands | Immediates and validation |
| ---: | --- | --- | --- | --- |
| 1 | `Const` | value | none | immediate 0 is the scalar magnitude; immediate 1 is zero. Result is scalar and magnitude lies in its range. |
| 2 | `SelfPlace` | place | none | both zero. Result is the current machine owner's nominal type. |
| 3 | `FieldPlace` | place | base place | immediate 0 is field ID; immediate 1 is zero. Base is that field's owner nominal type; result is its field type. |
| 4 | `IndexPlace` | place | array place, scalar index value | both zero. Base is fixed array; index is `u8` or `u32`; result is element type. |
| 5 | `Load` | value | scalar place | both zero. Result type equals place type. Structural loads are unsupported. |
| 6 | `Store` | none | destination scalar place, source scalar value | both zero. Types are carrier-compatible and destination is mutable. |
| 7 | `Copy` | none | destination place, source reference | immediate 0 is `1` when source is a structural value and `2` when source is a place; immediate 1 is zero. Exact types agree, type is copyable, and destination is mutable. |
| 8 | `Add` | value | left value, right value | both zero. All three types are carrier-compatible `u8` or carrier-compatible `u32`. Trapping addition and result-range rules apply. |
| 9 | `Less` | value | left value, right value | both zero. Operands are carrier-compatible `u8` or carrier-compatible `u32`; result is canonical `bool`. Comparison is unsigned. |

No other opcode, result shape, operand count, flag, or immediate is valid. Place
mutability is derived, never asserted in the wire format: `SelfPlace` is mutable
exactly in a mutable block, and `FieldPlace`/`IndexPlace` inherit their base
place's mutability.

`IndexPlace` has the Omega trapping-index meaning for this tranche. Native
lowering must compare the runtime index with the fixed length before address
arithmetic and trap when it is out of bounds, even when frontend range facts
prove the accepted source safe. `Add` must trap on carrier overflow or a result
outside its declared scalar interval. A `Store` must trap if the runtime scalar
is outside the destination interval. Static source proof does not authorize the
backend to omit these checks in CKIR1.

## 6. Terminators and edge arguments

### 6.1 Terminator row — 44 bytes

```text
u32  id
u32  owner machine ID
u32  owner block ID
u8   kind
u8   flags, zero
u16  reserved, zero
u32  value ID, or NO_ID
u32  target 0 block ID, or NO_ID
u32  target 0 argument start
u32  target 0 argument count
u32  target 1 block ID, or NO_ID
u32  target 1 argument start
u32  target 1 argument count
```

Terminator kinds are:

| Tag | Terminator | Value | Targets |
| ---: | --- | --- | --- |
| 1 | `Jump` | `NO_ID` | target 0 present; target 1 absent |
| 2 | `Branch` | visible `bool` value | both targets present |
| 3 | `ReturnUnit` | `NO_ID` | both absent; machine result is Unit |
| 4 | `ReturnValue` | visible scalar value carrier-compatible with the machine result type | both absent |

Every target belongs to the same machine. Each edge argument count equals its
target block's parameter count; arguments are visible values in source ordinal
order. A scalar argument is carrier-compatible with its target parameter and a
structural argument has its exact type ID. Entry blocks are not legal edge
targets. No-target argument counts are zero.

The operation spans first partition `0..operation_operand_end`. Then, in
terminator-ID order, target 0 arguments followed by target 1 arguments partition
the remainder of the operand vector. Empty argument spans use the current next
index. No operand-vector word is unused.

Edge argument transfer is simultaneous. A native implementation must stage all
source values before writing any target block-parameter slot, so a loop edge
cannot observe a partially overwritten parallel assignment.

## 7. Conservative layout and Linux x86-64 lowering

CKIR1 layout is private to this bridge tranche and is not an Omega ABI promise.
The backend computes it; the producer supplies no size, alignment, offset,
register, or frame information.

Scalar size/alignment is one byte for `u8` and `bool`, and four bytes for `u32`.
A fixed array has its element alignment, element stride equal to element size
rounded up to that alignment, and checked size `stride * length`. A record lays
out fields in ordinal order: each field begins at the smallest offset at or above
the cursor that is divisible by its alignment; record alignment is the maximum
field alignment or one for an empty record; final size is rounded up to record
alignment. Every calculation is checked and the by-value graph must be acyclic.

Fixed arrays are raw inline repetition: an array value is exactly `length`
consecutive element strides. It has no runtime length word, descriptor, pointer,
or capacity field.

The runnable backend lowers only the selected conformance-entry machine. CKIR1
contains no machine-call operation, exported symbol table, or external entry, so
other machines are semantically unreachable from that closed image and need not
be emitted. They must nevertheless pass all CKIR validation. A CKIR module with
no conformance entry is a valid library-shaped handoff: the backend exits 0 and
publishes empty stdout. An empty output in that case is not an ELF artifact.

The selected entry must:

- have a mutable or shared attached receiver;
- have zero explicit machine parameters;
- have a scalar `u8`, `u32`, or `bool` result; and
- have an entry block with zero block parameters.

The producer determines the private root compositionally. A candidate is an
attached machine with zero explicit machine parameters and a scalar `u8`, `u32`,
or `bool` result. Zero candidates produces library CKIR with header bit 0 clear
and `NO_ID`; exactly one sets bit 0 and its machine ID; more than one rejects 251
without CKIR. The producer does not recognize a candidate's name or declaration
position. The backend reconstructs this candidate set from the machine and type
tables and rejects 251 unless its cardinality and member agree exactly with the
header flag and entry field.

The backend allocates one zero-filled instance of the entry owner. Zero must be
within every scalar interval recursively reachable in that owner. The entry shim
passes its address as the implicit receiver, invokes the selected machine, and
passes the low eight result bits to Linux `exit_group`. It writes no stdout or
stderr. This is a conformance adapter, not authored `target::ProgramEntry`
selection. Unit entry, external authority, and more than one runtime instance are
outside CKIR1.

Within the selected machine, scalar values use four-byte frame slots with `u8`
and `bool` canonicalized on every definition. Structural parameter values are
immutable addresses and use eight-byte slots; places also use eight-byte address
slots. The receiver address has its own eight-byte slot. Slots are assigned
deterministically in value/place ID order after filtering to the selected
machine. Edge scratch has one eight-byte slot per argument in the largest
outgoing edge. The complete frame is rounded to 16-byte alignment. No red-zone,
ambient heap, dynamic allocation, or host import is used.

`SelfPlace` copies the receiver address. `FieldPlace` adds the recomputed field
offset. `IndexPlace` performs the unsigned bound check before multiplying by the
recomputed stride and adding the address. Scalar loads and stores use their exact
width. A scalar store, scalar edge transfer, and scalar return check the
destination or declared result interval before committing the value. Structural
edge transfer copies the immutable address. `Copy` snapshots and writes every
semantic scalar leaf of the structural value in declaration/index order.
Padding is not semantic data and is not copied; destination padding remains
unchanged. Source and destination may alias, so all source leaves are observed
as if before the first destination leaf is committed.
`Add`, interval checks, and index checks branch to one shared `ud2` trap stub.
`Less` uses unsigned comparison and produces exactly zero or one. Block
transfers use rel32 branches after all edge arguments are staged.

The emitted artifact is deterministic ELF64 little-endian `ET_EXEC`, machine 62,
with page size 4,096 and image base `0x400000`. It has exactly two `PT_LOAD`
segments:

- an RX segment at file offset 0 and virtual address `0x400000`; ELF header and
  two program headers occupy the beginning, bytes through offset 4,095 are zero,
  text starts at file offset and virtual offset `0x1000`, and the segment is
  zero-padded to a page boundary;
- an RW, non-executable zero-fill segment at the first byte after the padded RX
  segment, with virtual address `0x400000 + rx_file_size`, file size zero, and
  memory size equal to `max(entry_owner_size, 1)` rounded up to a page.

The ELF entry is `0x401000`, the first byte of the conformance shim. There are no
section headers, interpreter, dynamic entries, relocations, symbols, imports, or
bytes for the zero-fill segment. The output length equals the padded RX file
size. Machine blocks are emitted in block-ID order after the shim and shared trap
stub; all sizes and rel32 displacements are precomputed and rechecked while
emitting.

## 8. Resource, status, and publication contract

CKIR1 publishes these artifact ceilings in addition to the source-profile
ceilings:

| Resource | Ceiling |
| --- | ---: |
| encoded CKIR bytes | 4,194,304 |
| types / records / fields | 8,192 / 128 / 8,192 |
| machines / blocks | 128 / 2,048 |
| machine parameters | 4,096 |
| block parameters | 4,096 |
| both parameter tables combined | 4,096 |
| operations / operand-vector words | 32,768 / 131,072 |
| values / places | 40,960 / 32,768 |
| fixed-array length | 65,536 |
| `layout.data_bytes` (selected-owner layout) | 131,072 |
| selected-machine frame bytes | 262,144 |
| RX text bytes before page padding | 1,048,576 |

The operation ceiling is a backing bound, not permission to bypass the public
32-statements-per-block or expression-depth limits. A producer must justify that
its lowering expansion fits these aggregate tables for every admitted source;
it may not discard operations, blocks, fields, or declarations to stay within a
bound.

These aggregate ceilings are deliberately smaller than the Cartesian product
of the per-declaration source ceilings. The 131,072-byte source-unit bound is
the first aggregate limiter, and CKIR expansion has its own explicit checked
capacity. The chosen tables keep the complete Delta decoder/lowerer below the
current 16 MiB AArch64 self-field-addressing envelope and every single backing
array within the lower-rung elaborator's 524,288-cell bound. Crossing one of
these private capacities is declared exhaustion, not a restriction on Omega's
language semantics or on a later CKIR schema.

Status 0 means complete success. For the producer, raw-unit status 0 publishes
nothing and exactly-one-unit bundle status 0 publishes one CKIR module. For the
backend, entry-bearing CKIR status 0 publishes one ELF and library CKIR status 0
publishes nothing. Malformed/noncanonical CKIR, invalid IDs or relations,
unsupported CKIR1 forms, static type or mutability failure, recursive layout,
ambiguous conformance roots, or target mismatch returns 251. Crossing a
declared source, table, CKIR-byte, layout, frame, text, displacement, or image
ceiling returns 252. Arithmetic that would overflow while checking a purported
count is malformed 251 unless the already validated count simply exceeds a
declared ceiling, which is 252.

Status is monotonic: once 252 is selected it cannot be overwritten by 251.
Neither producer nor backend writes one stdout byte before complete decode,
semantic validation, exact EOF, layout, code-size, displacement, and output-size
preflight succeeds. On status 251 or 252 stdout is empty. Diagnostics, if any,
use stderr and are not part of the observation. On producer status 0 stdout is
either empty in raw mode or exactly one CKIR module in bundle mode; on backend
status 0 stdout is either empty for library CKIR or exactly one ELF image for an
entry-bearing module.

## 9. Explicit exclusions and non-authority

CKIR1 deliberately excludes free machines, machine calls, recursion, multiple
runtime objects, allocation, pointers as source values, structural returns,
structural scalar loads, sums, generics, domains, proofs, atomics, threads,
exceptions, boundary calls, packages/modules, target selection, optimization,
debug information, and every Terminal-Psi operation. It does not settle whether
any excluded source facility belongs in final `Ωself`.

The signed-D0 carrier restriction is retained: CKIR1 does not establish complete
Omega `u32` literal/range support. Its record layout is internal and cannot be
used for FFI, public ABI, wire identity, stable hashing, or product layout
decisions. The conformance entry and process-status observation are test adapters
only. A product compiler producing the same exit status or image bytes is useful
differential evidence, not bootstrap authority and not DDC.

## 10. Required evidence before use as an artifact tranche

The following are mandatory; a fixture and a successful run alone do not close
the contract:

1. The Delta source producer accepts the exact `compiler/psi/source/source.omg`
   library shape and a self-contained renamed/reordered conformance program
   through the same compositional path. Native and `lowermachine`-built producers
   emit identical CKIR for each exact input, and repeated production is
   byte-identical.
2. The exact fixture
   `bootstrap/omega-bootstrap/gates/fixtures/source-custody-artifact.omg`
   exercises nested record copy, mutable and shared
   attached receiver use, fixed-array store/read, scalar store/load, `Add`,
   `Less`, guarded named-state control, state arguments, and scalar return. Its
   Linux image exits 70 with empty stdout/stderr. A product-owned Omega path must
   agree on behavior without owning this private format.
3. Producer and backend each elaborate through the persisted Beta-written
   Delta-to-Gamma route. Canonical Gamma execution agrees with native Delta on
   complete status and every published byte for the positive, a semantic 251,
   and a resource 252 observation.
4. Exact-limit and adjacent-limit teeth cover every table and layout ceiling.
   Malformed/truncated/trailing inputs and mutations of every header flag, count,
   ID, span, owner, ordinal, type, receiver access, result, opcode, operand,
   immediate, target, edge arity, and terminator class reject before output.
   Distinct teeth cover use-before-definition, cross-machine/block references,
   shared-receiver mutation, noncopyable copy, recursive layout, range failure,
   bad index, code/displacement exhaustion, and a 252-to-251 overwrite attempt.
5. An independent artifact check reconstructs the two ELF segments, entry,
   zero-fill size, frame/field/array offsets, block displacements, instruction
   templates, padding, and exact EOF. Mutation of any reconstructed byte or
   relation rejects.
6. Lower-rooted refinement reconstructs the accepted source tables to CKIR
   relation and the CKIR to limited x86-64/ELF relation for each compilation.
   The checker, not an encoder or shell comparison, recomputes the selected
   source result and the emitted template's observation; a perturbed result,
   CKIR operation, field offset, branch target, or artifact byte must be
   unprovable.

Current evidence closes items 1–3. The focused artifact gate also closes
representative exact/adjacent resources and mutations from item 4 and
independently reconstructs the ELF segment, entry, and selected-owner BSS
envelope from item 5. It does not claim exhaustive table/relation teeth, exact
instruction/displacement reconstruction, or item 6. Those three remaining
obligations stay separately visible in `TASKS_BOOTSTRAP.md`; passing the current
fixture cannot silently promote this tranche to complete.

Only after these obligations and their negative controls pass may CKIR1 support
the bounded artifact tranche. Later widening must publish a new schema version
or a compatible rule explicitly defined by a later contract; unused flag bits,
tags, fields, padding, and reserved values cannot be interpreted by convention.
