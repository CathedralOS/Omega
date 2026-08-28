# Conservative CKIR6 backend implementation note

[`CKIR6`](OMEGA_BOOTSTRAP_CHECKED_IR_V6.md) defines the normative private byte
and meaning contract. This note owns only its conservative Delta artifact
implementation and focused evidence.

The shared `omega-bootstrap-checked-ir-v5-to-elf.alp` implementation accepts
schema majors 4, 5, and 6. Sharing the implementation does not widen the
identities: schema 4 and 5 retain their frozen decoders, validations,
instruction templates, and ELF bytes, and opcode 15 rejects unless the input is
exact CKIR6. CKIR6 inherits the complete CKIR5 physical table layout.

After independently validating a `LogicalNot` row, the backend emits this exact
x86-64 template:

```text
mov eax, dword ptr [rbp - operand_slot]
xor eax, 1
mov dword ptr [rbp - result_slot]
```

The encoded logical operation is therefore the inherited value load, bytes
`83 f0 01`, and the inherited value store. Canonical Boolean validation makes
XOR-with-one exact for both truth-table rows. No comparison, branch,
truthiness normalization, constant folding, or special object representation
is permitted in this relation.

The ordinary dry sizing pass runs the same emitter and therefore includes the
three new instruction bytes without a separate text-size claim. Logical-not
values consume the inherited four-byte scalar slots and all frame, text, ELF,
operation, operand, and value ceilings remain unchanged.

The retained `../gates/delta-checked-ir-v6-backend-fixture.py` derives a backend-canonical
  result-70 carrier from the existing CKIR5 composition and makes one existing
  Boolean value depend on opcode 15. The former producer-backed wrappers joined
  it to frozen CKIR4/5 parity, exact ELF identity, load/XOR-one/store bytes,
  mutation rejection, and independent validation. Replay is suspended until
  canonical Delta publication.

This is private bootstrap evidence. It does not define a public Omega ABI,
admit other unary operators, or optimize adjacent negations. The separate
lower-rooted `OMGRFN8` contract composes this artifact relation with exact
source lowering; backend evidence alone does not imply that closure.
