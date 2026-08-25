# OMGCOMP lower-rooted refinement witness, version 1

This document fixes the private witness and refinement frame for the first
two-unit `OMGCOMP` nominal-data artifact. `OMGRSW1` is also the bridge-private
normalized frontend/resolution handoff defined by
[`OMEGA_BOOTSTRAP_RESOLUTION.md`](../../../omega-bootstrap/compiler/OMEGA_BOOTSTRAP_RESOLUTION.md).
When embedded here it remains an untrusted witness: producer provenance grants
no authority, and independent persisted-Beta checkers reconstruct the same
exact rows. It is not Omega syntax, backend checked IR, a resolver receipt, a
stable product ABI, or a trust grant.
The canonical source/envelope/reference fixture lives under
[`gates/fixtures/two-unit-import/`](../../../omega-bootstrap/gates/fixtures/two-unit-import/).

The witness retains resolved source identity, declarations, and static-name
bindings only. Source-body operations are absent: lowering and selected-result
checkers reconstruct them from the exact source bytes. CKIR1 remains the only
bridge handoff to the backend.

## Version-2 refinement frame

The existing one-unit `OMGRFN1` frame is not widened implicitly. Multi-unit
refinement uses this distinct little-endian frame:

```text
offset  width  field
0       8      magic: ASCII "OMGRFN2\0"
8       u32    version: 2
12      u32    flags: 0 library, 1 entry-bearing compilation
16      u32    exact OMGCOMP byte length
20      u32    exact resolution-witness byte length
24      u32    exact CKIR1 byte length
28      u32    exact ELF byte length
32      u32    claimed full result
36      u32    claimed exit projection
40      ...    OMGCOMP || witness || CKIR1 || ELF || exact EOF
```

Library and entry result rules are unchanged from `OMGRFN1`. Component ceilings
are 267,280 OMGCOMP bytes, 524,288 witness bytes, 2,260,040 CKIR bytes, and
1,052,672 ELF bytes. Their simultaneous framed size is 4,104,320 bytes, below
the existing 4,194,304-byte backing by 89,984 bytes. Checked addition precedes
every offset. Excess is 252; malformed fields, relations, arithmetic, and EOF
are 251. Nothing is published.

The frame contains no digest and no resolver authority.

## `OMGRSW1` header — 72 bytes

All integers are little-endian and fit signed 32-bit bootstrap arithmetic.
`NO_ID` is `0xffffffff` only where explicitly permitted.

```text
offset  width  field
0       8      magic: ASCII "OMGRSW1\0"
8       u16    schema major: 1
10      u16    schema minor: 0
12      u16    flags: zero
14      u16    header size: 72
16      u32    exact encoded witness length
20      u32    unit count
24      u32    import count
28      u32    binding count
32      u32    declaration count
36      u32    type count
40      u32    record count
44      u32    field count
48      u32    machine count
52      u32    machine-parameter count
56      u32    block count
60      u32    block-parameter count
64      u32    selected machine ID, or NO_ID for a library
68      u32    reserved: zero
```

Tables follow in header order. Computed length equals both declared length and
exact witness extent. IDs are dense. Spans are canonical contiguous partitions;
a zero-count span names the next unconsumed row. Reserved fields are zero.

| Table | Ceiling | Stride |
| --- | ---: | ---: |
| unit | 16 | 36 |
| import | 64 | 48 |
| binding | 4,096 | 28 |
| declaration | 256 | 28 |
| type | 2,048 | 24 |
| record | 128 | 24 |
| field | 4,096 | 24 |
| machine | 128 | 40 |
| machine parameter | 2,048 | 24 |
| block | 2,048 | 40 |
| block parameter | 4,096 | 24 |

All maxima occupy 510,600 bytes. These are evidence-carrier limits, not new
compiler-language limits. Exceeding one means version-1 evidence exhaustion
(252), never partial compiler acceptance.

## Source references and order

Every source `start,length` pair is relative to one exact OMGCOMP source content
extent. It is in bounds, aligns to independently lexed token boundaries, and
never crosses a source boundary. Strings are not copied: module/alias string IDs
refer to OMGCOMP strings, and package/source/alias IDs refer to OMGCOMP rows.

Changing custody labels alone changes no identity. Changing package keys,
source ownership/placement, aliases, or source bytes does. Semantic declaration
order is package ID, source ID, and authored declaration order. Fields, machine
parameters, named states, block parameters, and body operations retain authored
order.

## Unit row — 36 bytes

```text
u32  dense source/unit ID
u32  owner package ID
u32  resolver-owned module-path string ID
u32  authored module-path start, or NO_ID when absent
u32  authored module-path length; zero exactly when absent
u32  import-row start
u32  import-row count
u32  declaration-row start
u32  declaration-row count
```

There is one row per OMGCOMP source in source-ID order. Owner and module equal
the source row. The authored `module`, when present, is unique and equals the
resolver-owned path byte-for-byte. Absence retains resolver placement and does
not infer from the bundle label. Import and declaration spans partition in unit
order.

## Import row — 48 bytes

```text
u32  dense import ID
u32  source ID
u32  authored `use` ordinal within that source
u32  complete authored path start
u32  complete authored path length
u8   origin: 0 same-package module, 1 requester-local direct alias
u8   target kind: 1 data, 2 machine
u16  reserved: zero
u32  OMGCOMP alias-row ID, or NO_ID for same-package origin
u32  resolved target package ID
u32  resolved target module-path string ID
u32  resolved declaration ID
u32  locally introduced final-name start
u32  locally introduced final-name length
```

Rows are in source/authored-use order. Origin 1 names an alias row whose
requester is the source package, raw alias is the first path component, and
target is the recorded package. Graph reach and another requester's alias do
not substitute. Origin 0 requires `NO_ID` and an existing same-package
top-level module. Matching both origins is ambiguous and rejects.

The remaining path resolves exactly one declaration in the recorded module.
Cross-package targets are public. Missing, duplicate, inaccessible, and
duplicate-local-name imports reject. Machine targets are reserved so later
CKIR call support need not replace name-resolution evidence; the first artifact
imports data only.

## Static binding row — 28 bytes

```text
u32  dense binding ID
u32  source ID
u8   role: 1 type name, 2 attached-machine owner, 3 machine target
u8   target kind: 1 data, 2 machine
u16  reserved: zero
u32  exact reference start
u32  exact reference length
u32  resolved declaration ID
u32  import-row ID, or NO_ID for same-package/local resolution
```

Rows are ordered by `(source ID,start,role)`, one for every non-builtin static
reference in the admitted surface. Imported bindings name their unique import
row. Qualified same-package bindings use `NO_ID`. The first CKIR1 artifact has
no role-3 rows; the versioned call successor consumes exact role-3 machine
targets without redoing name resolution.

## Declaration row — 28 bytes

```text
u32  dense declaration-order ID
u8   kind: 1 data, 2 machine
u8   visibility: 0 private, 1 public
u16  reserved: zero
u32  source ID
u32  authored declaration ordinal within the source
u32  declaration-name start
u32  declaration-name length
u32  kind-table ID: record ID or machine ID
```

`declaration_count == record_count + machine_count`. Record and machine IDs are
the dense filtered declaration order. Names are unique within the semantic
module and kind/owner namespace; multiple files in one module share it.

## Type row — 24 bytes

Type rows are byte-for-byte CKIR1 type rows and satisfy all CKIR1 relations.
Interning order is reconstructed rather than selected by the witness:

1. nominal types in record-ID order;
2. canonical `bool` and full admitted `u32`;
3. distinct non-array authored types on first encounter through fields,
   machine parameters/results, and block parameters in table order; then
4. distinct arrays in repeated source-type encounter order once their element
   type is interned, rejecting a no-progress cycle.

The source checker derives each descriptor from exact tokens and bindings. A
nominal payload is a resolved record ID, never a spelling or package-local ID.

## Record row — 24 bytes

```text
u32  dense record ID
u32  declaration ID
u32  nominal type ID
u32  field-row start
u32  field-row count
u8   flags: bit 0 is checked authored `[copy]`
u8   reserved[3]: zero
```

## Field row — 24 bytes

```text
u32  dense field ID
u32  owner record ID
u32  ordinal within owner
u32  type ID
u32  field-name start
u32  field-name length
```

Copyability, by-value acyclicity, layout, and offsets are recomputed; none is
asserted by the witness.

## Machine row — 40 bytes

```text
u32  dense machine ID
u32  declaration ID
u32  owner record ID
u8   receiver access: 1 shared, 2 mutable
u8   flags: zero
u16  reserved: zero
u32  result type ID, or NO_ID for Unit
u32  machine-parameter start
u32  machine-parameter count
u32  block start
u32  block count
u32  entry block ID
```

The owner uses an exact role-2 binding. The selected machine is derived by
matching OMGCOMP root package/source/module/owner/machine plus the conformance
signature. Libraries require `NO_ID`; entry frames require the exact ID.

## Machine-parameter row — 24 bytes

```text
u32  dense parameter ID
u32  owner machine ID
u32  ordinal within owner
u32  type ID
u32  parameter-name start
u32  parameter-name length
```

## Block row — 40 bytes

```text
u32  dense block ID
u32  owner machine ID
u32  ordinal: zero entry, then named states in authored order
u8   receiver access
u8   flags: zero
u16  reserved: zero
u32  body start
u32  body end, exclusive
u32  state-name start, or NO_ID for entry
u32  state-name length; zero for entry
u32  block-parameter start
u32  block-parameter count
```

Body spans stay within the owning source. The lowering checker reparses them;
this row supplies custody and identity, not operations.

## Block-parameter row — 24 bytes

```text
u32  dense block-parameter ID
u32  owner block ID
u32  ordinal within owner
u32  type ID
u32  parameter-name start
u32  parameter-name length
```

## Independent checker split

No checker imports process-local conclusions or shell flags. Each reads the
same `OMGRFN2` and locally performs enough checked framing to make accesses safe.

1. **Frame/OMGCOMP custody** validates the version-2 frame, complete OMGCOMP
   structure, nested bundle, exact source extents, graph, resources, and EOF.
2. **Source to witness** lexes each source independently, validates
   module/use/pub, builds namespaces, enforces direct requester-local reach and
   visibility, resolves every static reference, and compares every witness row.
   It reads no CKIR or ELF.
3. **Witness to CKIR tables** validates the witness, reconstructs
   copyability/layout/type interning, and compares type, record, field, machine,
   parameter, block, and selected-root rows.
4. **Resolved bodies to CKIR/result** reparses exact bodies using witness
   identities and compares all lowering rows. A companion source-only evaluator
   recomputes the full result without CKIR or ELF.
5. Existing complete CKIR and CKIR-to-ELF checkers are reused after changing
   only their frame offsets.

The split is required by Beta's 128-procedure ceiling: current one-unit table
and lowering compositions already use 123 and 115 procedures respectively.
Adding OMGCOMP resolution to either monolith is not credible modularization.

Implementation status:

- layer 1 closes framing and complete OMGCOMP structural/source-extent custody
  in
  [`omgrfn2-frame-omgcomp-custody.beta`](omgrfn2-frame-omgcomp-custody.beta);
- layer 2 independently reconstructs source resolution and exact witness bytes
  in
  [`omgrfn2-source-witness-independent.beta`](omgrfn2-source-witness-independent.beta);
- layer 3 reconstructs layout, interning, declarations, blocks, and the selected
  root in
  [`omgrfn2-witness-ckir-tables.beta`](omgrfn2-witness-ckir-tables.beta);
- layer 4 reconstructs resolved bodies and every operation/operand/terminator
  row, while a physically artifact-free companion independently computes the
  full source result, through
  [`omgrfn2-resolved-body-result.sh`](omgrfn2-resolved-body-result.sh); and
- layer 5 reuses the complete CKIR and CKIR-to-ELF relations at the v2 frame
  offsets through
  [`omgrfn2-ckir-elf-refinement.sh`](omgrfn2-ckir-elf-refinement.sh).

All five responsibilities are executable and joined by the lattice
orchestrator after the producer's native/self-built and Rust-free two-package
composition gates. The join closes the selected public two-package, finite,
acyclic, returning artifact; it does not grant resolver-receipt or digest
authority and does not generalize this fixture into `Ωself`. Mechanically
translating the Delta resolver into Beta would preserve the same common-mode
mistakes and would not satisfy source-to-witness reconstruction; layer 2 is
independently authored.

The exact-root and finite attached-call successor is separately versioned as
[`OMGCOMP_REFINEMENT_WITNESS_V3.md`](OMGCOMP_REFINEMENT_WITNESS_V3.md).
`OMGRFN2` and its CKIR1-specific checkers remain frozen.

## Authority separation

The semantic conjunction is:

```text
Structural(OMGCOMP)
and Resolves(OMGCOMP,witness)
and Tables(witness,CKIR)
and Lowers(OMGCOMP,witness,CKIR)
and Emits(CKIR,ELF)
```

Compilation acceptance separately requires:

```text
AcceptedResolverReceipt(receipt)
and SHA256(exact OMGCOMP) == receipt.expected_envelope_sha256
```

Receipt format and acceptance root are external dependencies. A digest copied
into this frame, witness, fixture manifest, or shell variable is self-asserted
and grants no authority. Until accepted receipt bytes and a lower-rooted
SHA-256 comparison both exist, a gate may establish semantic/artifact
refinement but must not call the compilation accepted.

## Required controls

Valid cross-pairs isolate custody-label rename, equivalent direct aliases,
package/source/declaration reorder, same-shaped nominal targets with different
record IDs, source/witness, witness/CKIR, source/result, and CKIR/ELF joins.

Rejection teeth cover module mismatch/duplication, alias-versus-module
ambiguity, undeclared and transitive-only aliases, wrong requester/target,
missing/duplicate imported names, private cross-package data, wrong binding
kind/ID, non-token spans, row/span/order drift, root drift, and all framing and
resource boundaries. Existing body, result, CKIR-schema, and ELF-byte mutation
inventories remain applicable.

The first fixture uses the nonempty modules `model` and `app` and one public
cross-package data import. The language guide does not yet define a finer
private-across-module visibility lattice; this slice does not need it and the
checker must not guess. Cross-unit machine calls remain unsupported by CKIR1
despite OMGRSW1 retaining their exact resolution identity for the versioned
successor.
