# Omega-bootstrap normalized resolution handoff, schema major 3

[`OMGRSW1`](OMEGA_BOOTSTRAP_RESOLUTION.md) |
[`OMGRSW2`](OMEGA_BOOTSTRAP_RESOLUTION_V2.md) |
[`OMGLOW6 and CKIR5`](OMEGA_BOOTSTRAP_CHECKED_IR_V5.md)

`OMGRSW3` is the minimal resolution successor for the bounded pure-sum bridge
tranche. It adds declaration-owned sum, case, and named case-payload identities.
Except where this document overrides it, every `OMGRSW1` source-custody,
resolution, ordering, type, status, and publication rule remains normative.
It also inherits the exact direct nominal field-receiver relation from
`OMGRSW2`.

This is a bridge-private normalized handoff. It is not an Omega ABI, a stable
schema format, a wire discriminant assignment, a proof of package acceptance,
or a final `Ωself` ruling.

## 1. Canonical version selection

The magic is `OMGRSW3\0`, schema major is 3, and schema minor is 0. A canonical
`OMGRSW3` contains at least one admitted pure sum. The shared resolver emits the
least relation required by the exact source closure:

| Source relation | Canonical witness |
| --- | --- |
| no sum and no direct field-receiver call | byte-identical `OMGRSW1` |
| no sum and at least one admitted direct field-receiver call | `OMGRSW2` |
| at least one admitted pure sum, with or without direct field-receiver calls | `OMGRSW3` |

Changing only a magic or major never creates another canonical witness. Every
consumer rejects cross-pairs among the three identities.

## 2. Header and table order

All integers use the inherited unsigned little-endian and signed-D0-carrier
rules. `NO_ID` remains `0xffffffff` only in positions that explicitly permit
it. The header grows from 72 to 84 bytes:

```text
offset  width  field
0       8      magic: ASCII "OMGRSW3\0"
8       u16    schema major: 3
10      u16    schema minor: 0
12      u16    flags: zero
14      u16    header size: 84
16      u32    exact total witness length
20      u32    source/unit count
24      u32    import count
28      u32    static-binding count
32      u32    declaration count
36      u32    type count
40      u32    record count
44      u32    field count
48      u32    machine count
52      u32    machine-parameter count
56      u32    block count
60      u32    block-parameter count
64      u32    sum count
68      u32    case count
72      u32    case-payload-field count
76      u32    selected machine ID
80      u32    reserved: zero
```

The inherited tables retain their exact row widths and internal order, with the
three new tables inserted after ordinary fields:

1. units;
2. imports;
3. static bindings;
4. declarations;
5. types;
6. records;
7. ordinary record fields;
8. sums;
9. cases;
10. case-payload fields;
11. machines;
12. machine parameters;
13. blocks; and
14. block parameters.

The exact encoded length is:

```text
84
+ 36 * source_count
+ 48 * import_count
+ 28 * binding_count
+ 28 * declaration_count
+ 24 * type_count
+ 24 * record_count
+ 24 * field_count
+ 24 * sum_count
+ 28 * case_count
+ 24 * case_payload_count
+ 40 * machine_count
+ 24 * machine_parameter_count
+ 40 * block_count
+ 24 * block_parameter_count
```

Checked arithmetic precedes every access. The computed length equals the
header length and exact EOF.

## 3. Pure-sum declarations and normalized types

An admitted declaration has only `case` members. A declaration containing both
ordinary fields and cases is a mixed shape and rejects in this tranche. Cases
are unnumbered; an authored `=` discriminant rejects rather than choosing a
meaning for the unresolved discriminant/zero-initialization interaction.
Generic sums, generic payloads, `repr`/foreign-layout sums, case domains,
machines attached to sums, and schema-numbered cases remain outside this
relation.

The inherited declaration row adds one kind without changing width:

| Kind | Meaning | kind-table ID |
| ---: | --- | --- |
| 1 | ordinary record data | record ID |
| 2 | machine | machine ID |
| 3 | pure-sum data | sum ID |

Imports and static bindings retain target kind `1 = data`; record-versus-sum is
derived from the target declaration row. The inherited normalized type row adds
kind `6 = nominal sum`, whose payload 0 is the exact sum ID, whose payload 1 and
range endpoints are zero, and whose trapping flag is zero. Every sum has
exactly one such row and every such row names exactly one sum.

Qualified constructor and arm spellings do not add a guessed body-name binding.
The expected construction type or transition-subject type selects the exact
sum; the exact case table then selects its unique same-named case. Imported and
qualified sum types still use the inherited role-1 type binding. The lowerer
validates these contextual relations from exact source plus the witness and
does not perform package or declaration lookup.

## 4. New rows

### 4.1 Sum row — 24 bytes

```text
u32  dense sum ID
u32  declaration ID
u32  nominal type ID
u32  case-row start
u32  case-row count
u8   flags: bit 0 is checked authored `[copy]`
u8   reserved[3]: zero
```

Sum rows use filtered declaration order. Case spans partition the case table in
sum-ID order. Every sum has at least one and at most 64 cases. The declaration
row is kind 3 and names this sum; the nominal type is kind 6 and names this sum.

`[copy]` is valid only when every case-payload field is recursively copyable.
Scalars are copyable; arrays, records, and sums recur through their declaration
rows. The combined by-value graph through record fields, case payloads, and
array elements must be acyclic for this tranche.

### 4.2 Case row — 28 bytes

```text
u32  dense case ID
u32  owner sum ID
u32  ordinal within owner
u32  payload-field start
u32  payload-field count
u32  case-name start
u32  case-name length
```

The owner is the sum whose span contains the row. Ordinals start at zero and
are dense. In this unnumbered relation, the ordinal is also the private runtime
tag: the first declared case is tag zero. Each case has at most four payload
fields. Names are unique within the sum member namespace.

### 4.3 Case-payload-field row — 24 bytes

```text
u32  dense payload-field ID
u32  owner case ID
u32  ordinal within owner
u32  normalized type ID
u32  payload-field-name start
u32  payload-field-name length
```

Case spans partition this table in case-ID order. Ordinals start at zero and
are dense. Name spans are relative to the exact source containing the owning
sum declaration, are exact identifier tokens, and are unique within the case.
No offset, size, alignment, tag width, payload address, or default/public layout
enters the witness.

## 5. Resources, status, and publication

The inherited maxima remain, including 16 sources, 4,096 bindings, 256 total
declarations, 2,048 normalized types, 128 machines, 2,048 blocks, and the
524,288-byte complete witness ceiling. The new maxima are:

| Resource | Ceiling |
| --- | ---: |
| ordinary records plus pure sums | 128 |
| cases in one sum | 64 |
| case rows in the witness | 4,096 |
| payload fields in one case | 4 |
| case-payload-field rows in the witness | 4,096 |

The exact encoded witness must fit 524,288 bytes; individual row ceilings are
not permission to realize an over-cap Cartesian product. Payload fields also
consume the inherited raw-type budget. A source-valid fifth payload field, 65th
case, or other validated public extent above a stated ceiling selects 252.
Malformed syntax, duplicate or unknown names, a mixed or explicitly numbered
shape, a type/copyability/cycle failure, bad relation, cross-version pair,
truncation, or trailing bytes selects 251. Semantic validity is established
before a four/five resource decision, so malformed five-field cases remain 251.

No byte is published until all source, resolution, type, copyability,
acyclicity, resource, exact-length, selected-root, and canonical-version checks
succeed. Status 251 or 252 has empty stdout, and an already selected 252 is not
downgraded by later inspection.

## 6. Explicit non-expansion

`OMGRSW3` does not settle explicit discriminants, mixed data, generics, domains,
proofs, stable/public sum layout, effectful expression order, or private access
between distinct logical modules. It carries identities required by the first
pure-sum bridge-cost slice and no compiler or package authority.
