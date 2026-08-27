# Omega-bootstrap guarded full-u64 fixed-buffer witness, schema major 10

[`OMGRSW8`](OMEGA_BOOTSTRAP_RESOLUTION_V8.md) |
[`OMGLOWJ`](OMEGA_BOOTSTRAP_LOWERING_V19.md)

`OMGRSWA` is the focused normalized source witness for the first general
full-width `u64` fixed-buffer mutation/indexing relation.  It consumes an exact
one-unit `OMGCOMP1` envelope.  It is implemented by the focused
`omega-bootstrap-u64-buffer-resolve.alp`, not by widening the historical shared
resolver.

The source family is structural and name-independent.  It is a capability
projection, not an exact whole-record description of the product
`SourceUnit`: unrelated inert fields and parameters may be present and are not
published.  It admits renamed and declaration/field/state-reordered
SourceUnit-like records with:

- one trapping fixed `[u8; N]`, `1 <= N <= 65,536`;
- one `u64 [0..=N]` retained length and ordinary Boolean status;
- a mutable clear operation;
- a mutable guarded append whose true state stores at `bytes[length]`, assigns
  an authored exact leaf-plus-literal increment `length + 1` with defensive
  checked lowering, and records success;
- a shared guarded lookup whose true state returns `bytes[index]` and whose
  false state returns zero; its index is exact authored `u64 in Trapping`; and
- one finite root harness using ordinary direct-field receiver calls.

The resolver parses declarations and each machine production, resolves field,
state, parameter and call identities, and derives spans from the matched
nodes.  Readable names, source labels, declaration order, whole-source token
counts/digests, and fixed token ordinals are not semantic selectors.

Computed or effectful indexes, mutable slices/views, indirect calls, recursion,
other `u64` arithmetic or relations, cross-carrier operands, more than one
observable trap in an expression, and unguarded indexed access reject.

## Identity and header

All words are little-endian.  The exact identity is magic `OMGRSWA\0`, major
10, minor zero, flags zero, and header size 128.  The header is
`8s + 4*u16 + 28*u32`:

```text
16 total length                 20 exact OMGCOMP1 input length
24 unit count                   28 normalized type count
32 record count                 36 field count
40 machine count                44 machine-parameter count
48 block count                  52 ordinary-call count
56 selected root machine        60 selected buffer record
64 clear machine                68 append machine
72 lookup machine               76 fixed-array length N
80 u8 type                      84 bool type
88 trapping full-u64 type       92 constrained length-u64 type
96 fixed-array type             100 buffer nominal type
104 root nominal type           108 relation flags (exactly 1 = complete)
112..127 reserved zero
```

The canonical fixture has counts `1,8,2,6,4,2,8,5`, selected identities
`3,0,0,1,2`, `N=65536`, type identities `1..7`, and exact witness length
1,376 bytes with SHA-256
`7ee027659ff1da971055f3c659dc298f1cc5417048a3a89000872ec4ad568ae5`.
Other accepted field/declaration orders rebuild semantic rows and spans; they
are not required to preserve canonical bytes.

## Tables

Tables follow in this exact order.  IDs are dense and every source span is
relative to the named unit content.

1. Unit, 20 bytes (`5I`): `id, source, source_start, source_length, flags`.
   The sole row covers the complete independently delimited source; flags zero.
2. Type, 32 bytes (`I B B H 6I`): `id, kind, flags, reserved, payload0,
   payload1, lower_lo, lower_hi, upper_lo, upper_hi`.  Kinds are Unit 0, u8 1,
   bool 3, nominal record 4, fixed array 5, and u64 10.  Flag bit zero retains
   authored `in Trapping`; it is required on the array and full-u64 lookup
   index, absent on the constrained length.  U64 endpoints retain all four
   words.  Array payloads are element type and N.
3. Record, 28 bytes (`7I`): `id, source, nominal_type, field_start,
   field_count, name_start, name_length`.  Counts cover selected projected
   fields, not unrelated inert source fields.
4. Field, 24 bytes (`6I`): `id, owner, ordinal, type, name_start, name_length`.
5. Machine, 56 bytes (`14I`): `id, source, owner, receiver_access,
   result_type_or_NO_ID, parameter_start, parameter_count, block_start,
   block_count, name_start, name_length, body_start, body_length, flags`.
   Receiver access is shared 1 or mutable 2; flags zero.
6. Machine parameter, 24 bytes (`6I`): `id, machine, ordinal, type,
   name_start, name_length`.
7. Block, 40 bytes (`10I`): `id, machine, ordinal, receiver_access,
   parameter_start, parameter_count, name_start, name_length, body_start,
   body_length`.  Entry blocks use zero name span.  This family has no state
   parameters.
8. Ordinary call, 36 bytes (`9I`): `id, source, caller_machine,
   target_machine, receiver_field, call_start, call_length, argument_count,
   flags`.  Flags zero.  The span begins at the resolved called-machine name
   token and extends through its closing `)`; it never names the declaration
   header.  Calls remain calls and are never rewritten to body copies.

The full-u64 `in Trapping` policy on the lookup index is therefore preserved explicitly through
resolution.  OMGLOWJ validates and consumes it before mapping both u64 source
types to CKIR18's policy-neutral kind-8 carrier; CKIR opcode 4 owns runtime
bounds trapping and opcode 8 owns defensive carry/result-range trapping.  The
length itself is the flags-zero constrained type.  Its authored `length + 1`
has no textual arithmetic-policy qualifier; the true-edge fact proves the
result lies in `0..=N`, while CKIR still retains its normal defensive trap.

## Failure and resources

Malformed framing, source grammar, duplicate or unresolved identity, wrong
type/policy, missing guard fact, unsafe index, wrong increment, recursive call,
span, dense-row, or exact-EOF mismatch returns 251 without output.  A valid
array length 65,537 exceeds the public fixed-array ceiling and is malformed 251,
not resource exhaustion.  Source/table/carrier capacity exhaustion returns 252
without output.  Publication begins only after complete semantic validation.

This witness grants no compilation/package authority, provider installation,
native effect, public ABI, or final Omega-self admission.
