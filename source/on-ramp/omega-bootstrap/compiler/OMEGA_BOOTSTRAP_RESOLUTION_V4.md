# Omega-bootstrap normalized resolution handoff, schema major 4

[`OMGRSW1`](OMEGA_BOOTSTRAP_RESOLUTION.md) |
[`OMGRSW2`](OMEGA_BOOTSTRAP_RESOLUTION_V2.md) |
[`OMGRSW3`](OMEGA_BOOTSTRAP_RESOLUTION_V3.md)

`OMGRSW4` is the minimal resolution successor for the program-static shared
byte-view bridge milestone. It adds one normalized source type, exact shared
`&[u8]`, and independently delimits the bounded plain byte literals used by the
selected source bodies. Except where this document overrides it, every
OMGRSW1/2/3 source-custody, resolution, ordering, type, status, and publication
rule remains normative.

This is a bridge-private normalized handoff. It is not an Omega ABI, a stable
wire format, a literal pool, a lifetime proof, package authority, or admission
to final `Ωself`.

## 1. Canonical version selection

The magic is `OMGRSW4\0`, schema major is 4, and schema minor is 0. A canonical
OMGRSW4 contains at least one admitted explicit machine-parameter or named-state
parameter whose exact normalized type is shared `&[u8]`.

The shared resolver publishes the least relation required by the complete exact
source closure:

| Source relation | Canonical witness |
| --- | --- |
| no admitted shared-byte parameter and no sum or direct field-receiver call | byte-identical OMGRSW1 |
| no admitted shared-byte parameter or sum, with an admitted direct field-receiver call | byte-identical OMGRSW2 |
| no admitted shared-byte parameter, with an admitted pure sum | byte-identical OMGRSW3 |
| at least one admitted shared-byte machine/state parameter, with or without inherited sums or field-receiver calls | OMGRSW4 |

A quoted literal alone does not select OMGRSW4 and adds no witness row; this
bounded literal spelling is admitted only when the completed closure separately
selects OMGRSW4 through an exact shared-byte parameter. A later sum declaration
cannot downgrade a closure already selected as OMGRSW4.
Changing only magic or major never creates another canonical witness.

## 2. Header, tables, and exact framing

OMGRSW4 inherits the complete OMGRSW3 84-byte header, table order, row widths,
count ceilings, checked-offset rules, and exact EOF. The only fixed-header
changes are magic and major:

```text
offset  width  field
0       8      magic: ASCII "OMGRSW4\0"
8       u16    schema major: 4
10      u16    schema minor: 0
12      u16    flags: zero
14      u16    header size: 84
16      ...    unchanged OMGRSW3 counts, selected machine, reserved zero,
              and unchanged V3 tables
```

No literal count, literal extent, provenance row, operator row, or body-token
table is added. The complete witness remains at most 524,288 bytes. Malformed
identity, framing, syntax, type, source span, ordering, or version relations
select status 251 with no output. A declared carrier or resource limit selects
252 with no output.

## 3. Normalized exact shared byte slice

The inherited 24-byte normalized type row adds exactly:

| Kind | Flags | Payload 0 | Payload 1 | Range |
| ---: | ---: | --- | ---: | --- |
| 7 | zero | canonical full-`u8` element type ID | zero | both zero |

The element row is exactly kind 1, flags zero, payloads zero, and range
`0..=255`. Canonicalization observes the element before the slice, interns
duplicate `&[u8]` occurrences to one row, and otherwise retains the inherited
first-encounter and array rules. A kind-7 row may appear only when an exact
admitted machine/state parameter uses it, and every such parameter names that
row.

The selected syntax is exactly `&[u8]`. `&mut [u8]`, other elements,
constraints, domains, arrays or slices of slices, record fields, case payloads,
and machine results remain outside this relation. OMGRSW4 records shared slice
type identity only. Program-static origin, descriptor construction, nonempty
facts, indexing, subslicing, lowering, and artifact meaning belong to later
versioned relations.

## 4. Plain quoted literal custody

In a closure independently selecting OMGRSW4, the resolver tokenizer admits a
quoted literal as one opaque body token when:

- the bytes between quotes are ASCII `0x20..0x7e`;
- neither `"` nor backslash occurs as payload;
- the closing quote occurs in the same source extent; and
- the payload contains at most 32 bytes.

The quote delimiters are not payload bytes. Braces, comment markers, and words
inside the literal do not affect body balancing, declaration scanning, or
role-3 resolution. Empty literals are valid. The 33rd payload byte selects 252
without publication.

Backslash escapes, codepoint escapes, raw-prefix spellings, controls and raw
newlines, non-ASCII bytes, character literals, and an absent closing quote
select 251. This selected tokenizer surface does not contradict the public raw-
byte meaning of ordinary Omega literals; it merely postpones every spelling
outside the checkpoint's bounded plain-ASCII need.

Literal tokens add no OMGRSW table, type, identity, address, encoding domain,
or interning claim. A body literal in a closure that otherwise selects
OMGRSW1/2/3 rejects rather than silently widening that frozen relation or
selecting OMGRSW4 by itself.

## 5. Focused evidence and non-expansion

The independent reference retains the finite multi-unit OMGRSW4 source with an
imported record, inherited pure sum, exact machine/state shared-byte
parameters, role-3 call, plain literals, V3-shaped envelope, kind-7 row,
canonical full-`u8` payload, parameter type IDs, and source spans. The former
handoff wrapper joined it to native/self resolver agreement; replay is
suspended until canonical Delta publication.

Controls retain byte-identical least OMGRSW1/2/3 identities, reject a literal
without independent V4 type selection, accept empty and 32-byte V4 literals, select 252 at 33
bytes, and reject mutable/other-element/aggregate/result slice shapes plus
escape/raw/control/non-ASCII/unterminated literals. Magic, numeric version,
kind, payload, flags, parameter type, length, and trailing-byte mutations reject
through the independent reference.

This milestone does not admit CKIR operations, descriptor ABI, indexing,
subsampling, mutable views, allocation, UTF-8 meaning, general raw strings,
general references, or heterogeneous `u32`/`u64` collection operations. The
current product checkpoint has since moved collection coordinates and counts to
same-carrier `u64`; V4 itself makes no claim about those operations.
