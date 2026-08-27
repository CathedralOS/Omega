# Omega bootstrap provider-plan lowering, outer version 18

[`OMGCOMP3`](OMEGA_BOOTSTRAP_COMPILATION_V3.md) |
[`OMGRSW9`](OMEGA_BOOTSTRAP_RESOLUTION_V9.md) |
[`CKIR17`](OMEGA_BOOTSTRAP_CHECKED_IR_V17.md)

`OMGLOWI` version 18 is the private focused producer relation from one exact
OMGCOMP3 source envelope and its exact OMGRSW9 provider-plan witness to CKIR17.
It is implemented by
[`omega-bootstrap-provider-plan-to-ckir.alp`](omega-bootstrap-provider-plan-to-ckir.alp),
not by extending the historical resolved-source lowerer.

The exact 32-byte little-endian header is:

```text
offset  width  field
0       8      magic: ASCII "OMGLOWI\0"
8       u16    outer version: 18
10      u16    minor: zero
12      u16    flags: zero
14      u16    header size: 32
16      u32    exact total frame length
20      u32    exact OMGCOMP3 length
24      u32    exact OMGRSW9 length
28      u32    resolution selector: 9
32      ...    exact OMGCOMP3 || exact OMGRSW9 || exact EOF
```

The component ceilings are 267,280 OMGCOMP3 bytes and 524,288 OMGRSW9 bytes;
the focused exact-pair frame ceiling is 269,616 bytes (32 + 267,280 + the
exact 2,304-byte OMGRSW9 selected witness). Identity, version, selector,
component extents, and exact EOF are one relation. A header relabel,
cross-version witness, or source/witness cross-pair cannot manufacture CKIR17.

## Selected source and resolved relation

The producer accepts only the bounded OMGRSW9 Console plan selected by the
authoritative OMGCOMP3 build source. It validates dense tables and spans,
typed requirement and candidate signatures, the complete six-row plan,
retained requirement calls, checked adapter calls, helper ranking and reaches,
and source ownership against the paired source envelope. It does not select by
filename, readable source label, whole-source digest, declaration count, token
ordinal, or AST permutation.

The executable source closure is deliberately smaller than the full plan:

- free helper `console_write_bytes(Console, &[u8], bool)`;
- static-attached `ConsoleNativeProvider::write(Console, &[u8])`;
- static-attached `ConsoleNativeProvider::write_line(Console, &[u8])`;
- the helper's two exact `Console::write_byte` requirement calls;
- the adapters' exact receiverless calls to the helper with `false` and `true`;
- the exact `Slice::Length` recurrent descent; and
- one Console reach for each of those three machines.

The four bodyless compiler-intrinsic candidates remain provider-plan custody.
Only candidate 4, which realizes requirement 4 `write_byte(i32) -> Unit`, is
named by CKIR17's boundary target. The boundary event preserves that identity;
it neither dispatches the candidate nor installs or admits a provider.

## Lowering

The helper is CKIR17 machine 0 with flag `FREE`, owner `NO_ID`, receiver access
zero, and parameters `(service, shared-byte-view, bool)`. The adapters are
machines 1 and 2 with flag `STATIC_ATTACHED`, provider owner 0, receiver access
zero, and parameters `(service, shared-byte-view)`. Their calls use opcode 28
`ReceiverlessCall`; the explicit service value is never synthesized.

The helper preserves both authored nonempty tests. Each true edge enters a
synthetic block that applies inherited `SliceHead` and `SliceTailOne`. The two
authored `output as i32` expressions lower to opcode 30 `U8ToI32`; call-context
typing never invents an implicit conversion. Opcode 29 `BoundaryEvent` then
consumes `(Console service, widened i32)`. The initial and recurrent events
retain OMGRSW9 requirement-call rows 0 and 1 through the same boundary-target
identity. False edges preserve the authored finish path; newline true emits
mathematical byte 10 through that same explicit cast/event path.

The one ranking row names helper parameter ordinal 1 with strict recurrent
`SliceLength` descent. Exactly three reach rows cover helper and adapters.
Service values occur only in the positions admitted by CKIR17; no constant,
field, constructor, ordinary scalar operation, or result can mint them.

## Failure and resources

Malformed framing, component identity or pairing, source span, dense ID,
signature, ownership, plan, reach, ranking, call, service custody, control-flow
shape, opcode mapping, or exact-EOF mismatch selects status 251. A declared
component, table, output, or parser resource excess selects 252. Neither
failure status publishes bytes.

The focused gate owns native/self producer identity, the independent CKIR17
byte comparison, source/witness cross-pairs, version/selector/EOF mutations,
semantic mutations, and adjacent 251/252 no-publication controls. CKIR17
remains a receiverless library with no entry, backend, host call, provider
installation, artifact authority, or final `Omega_self` admission.
