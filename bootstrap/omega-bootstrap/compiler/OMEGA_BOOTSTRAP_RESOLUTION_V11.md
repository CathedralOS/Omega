# Omega-bootstrap guarded record-array witness, schema major 11

[`OMGRSWA`](OMEGA_BOOTSTRAP_RESOLUTION_V10.md) |
[`OMGLOWK`](OMEGA_BOOTSTRAP_LOWERING_V20.md)

`OMGRSWB` is the focused normalized source witness for direct full-width-u64
indexing into fixed arrays of copyable records followed by nested scalar field
mutation. It consumes one exact `OMGCOMP1` source unit. The producer is the
focused `omega-bootstrap-record-array-resolve.alp`; the historical resolver is
not extended.

The accepted source family is structural, name-independent, and permits
declaration/field reordering, comments, and unrelated inert ordinary fields.
It contains:

- one `[copy]` record with exactly four `u8`, one `u32 in Trapping`, and four
  `u64 in Trapping` fields;
- one noncopy owner with `[Element; 16384] in Trapping`, a
  `u64 [0..=16384]` count, and Boolean retained status;
- a mutable push machine with nine scalar parameters of those exact authored
  types, a direct `count < 16384` guard, a true block which assigns every
  `rows[count].field` from a distinct same-typed parameter, exact `count + 1`,
  and success status, plus a false/full status block;
- a shared `u64 in Trapping` guarded lookup returning one stored `u8` field or
  zero; and
- a zero-argument mutable root which calls push with nine pure nontrapping
  integer literals, then calls lookup at zero. The literal feeding the selected
  read field is exactly 70.

The resolver parses declarations and bodies by grammar and resolved identity.
Filenames, readable names, source labels, whole-source token counts/digests,
fixed token ordinals, and declaration order are not selectors.

## Identity and header

All words are little-endian. The identity is magic `OMGRSWB\0`, major 11,
minor zero, flags zero, and header size 160. The header is
`8s + 4*u16 + 36*u32`:

```text
16 total length                 20 exact OMGCOMP1 input length
24 unit count                   28 normalized type count
32 record count                 36 field count
40 machine count                44 machine-parameter count
48 block count                  52 ordinary-call count
56 selected-store count         60 call-argument count
64 selected root machine        68 observation record
72 stream record                76 root record
80 push machine                 84 lookup machine
88 fixed-array length N         92 u8 type
96 u32 type                     100 bool type
104 trapping full-u64 type      108 constrained count-u64 type
112 fixed-array type            116 observation nominal type
120 stream nominal type         124 root nominal type
128 relation flags (=1)         132..159 reserved zero
```

Canonical counts are `1,10,3,13,3,10,7,2,9,9`; selected identities are root
machine 2, records 0/1/2, machines 0/1, `N=16384`, and types 1..9. The exact
canonical witness is 2,172 bytes with SHA-256
`00727b9c80aec71054a20dbc7afe80d8b587d377ebf22e04c45e0c5a164ebe05`.

## Tables

Tables follow in this exact order. IDs are dense. Every source span is relative
to the sole unit content.

1. Unit, 20 bytes (`5I`): `id, source, source_start, source_length, flags`.
2. Type, 32 bytes (`I B B H 6I`): `id, kind, flags, reserved, payload0,
   payload1, lower_lo, lower_hi, upper_lo, upper_hi`. Kinds are Unit 0, u8 1,
   u32 2, bool 3, nominal record 4, fixed array 5, and u64 10. Flag bit zero
   retains authored `in Trapping`; it is required on u32, full u64, and array,
   and absent on the constrained count. Array payloads are element type and N.
3. Record, 32 bytes (`8I`): `id, source, nominal_type, field_start,
   field_count, name_start, name_length, flags`. Flag bit zero is exact authored
   `[copy]` and is required only on the observation record.
4. Field, 24 bytes (`6I`): `id, owner, ordinal, type, name_start, name_length`.
   Observation rows are normalized in selected-store order; source ordinals are
   not semantic authority.
5. Machine, 56 bytes (`14I`): `id, source, owner, receiver_access,
   result_type_or_NO_ID, parameter_start, parameter_count, block_start,
   block_count, name_start, name_length, body_start, body_length, flags`.
6. Machine parameter, 24 bytes (`6I`): `id, machine, ordinal, type,
   name_start, name_length`.
7. Block, 40 bytes (`10I`): `id, machine, ordinal, receiver_access,
   parameter_start, parameter_count, name_start, name_length, body_start,
   body_length`. This slice has no state parameters.
8. Ordinary call, 36 bytes (`9I`): `id, source, caller_machine,
   target_machine, receiver_field, call_start, call_length, argument_count,
   flags`. Call spans begin at the resolved callee name and extend through `)`.
9. Selected store, 32 bytes (`8I`): `id, machine, block, array_field,
   index_field, element_field, parameter, scalar_type`. The nine rows are a
   bijection over observation fields and push parameters.
10. Call argument, 24 bytes (`6I`): `id, call, parameter, type, literal_lo,
    literal_hi`. This profile admits only nonnegative literals representable by
    the authored scalar type; the canonical upper limbs are zero.

The true edge owns the `count < N` range fact used by all nine direct indexes
and by the authored Exact `count + 1`. Array and index policy remain authored
Trapping custody. Scalar u32/u64 parameter policy is retained through this
witness and may be consumed only by the selected stores/calls during lowering.

## Exclusions and failure

Sums, structural parameters/projection, computed or effectful indexes,
slices/views, allocation, recursion, aliases, indirect calls, unrelated u64
operations, unguarded indexing, duplicate/missing stores, and non-pure root
arguments reject. Malformed framing, grammar, identity, policy, span, dense-row,
or exact-EOF mismatch returns 251 with no output. Public input/table capacity
exhaustion returns 252 with no output. Publication begins only after complete
semantic validation.

This witness grants no build/package authority, provider installation, public
ABI, native effect, or final Omega-self admission.
