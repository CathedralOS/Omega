# Conservative CKIR7 backend implementation note

[`CKIR7`](OMEGA_BOOTSTRAP_CHECKED_IR_V7.md) inherits the complete CKIR6 table
layout and opcode-15 `LogicalNot` relation. It adds the pure, nontrapping,
Boolean-only opcodes 16 `LogicalAnd` and 17 `LogicalOr`. A CKIR7 module must
contain at least one of those two binary operations; opcode 15 remains optional.

The shared `omega-bootstrap-checked-ir-v5-to-elf.alp` implementation accepts
schema majors 4 through 7. Each logical-binary row must have an exact Boolean
value result, two visible exact-Boolean value operands, and zero immediate and
reserved fields. Opcodes 16 and 17 reject under every earlier schema. Earlier
schema decoders, validation relations, instruction templates, and ELF bytes
remain frozen.

After validation, the backend emits one of these exact conservative x86-64
templates:

```text
mov eax, dword ptr [rbp - left_slot]
and eax, dword ptr [rbp - right_slot]  # opcode 16
mov dword ptr [rbp - result_slot], eax
```

```text
mov eax, dword ptr [rbp - left_slot]
or eax, dword ptr [rbp - right_slot]   # opcode 17
mov dword ptr [rbp - result_slot], eax
```

The inherited load and store begin with bytes `8b 85` and `89 85`; the middle
instructions begin with `23 85` for AND and `0b 85` for OR. Each instruction is
followed by its signed 32-bit frame displacement. Canonical Boolean validation
makes these bitwise operations the exact Boolean truth functions. This private
IR relation makes no source-level short-circuiting claim and performs no
constant folding, truthiness normalization, or control-flow rewrite.

The ordinary dry sizing pass uses the same emitter, so no separate text-size
accounting is introduced. Values retain the inherited four-byte scalar slots,
and all existing table, frame, text, ELF, operation, operand, and value ceilings
remain unchanged.

Focused evidence is split by responsibility:

- `../gates/delta-checked-ir-v7-reference.sh` checks independent decoding,
  canonical AND/OR meaning for all four Boolean input rows, schema and opcode
  isolation, malformed-row rejection, and inherited resource ceilings;
- `../gates/delta-checked-ir-v7-backend-fixture.py` recognizes the exact
  load/AND-or-OR-memory/store instruction templates.

The former producer-backed wrapper joined them to native/self artifact
identity, result 70, opcode-byte mutation rejection, and empty rejected
publication. Replay is suspended until canonical Delta publication.

This is backend evidence only. It does not define a public Omega ABI, source
syntax or evaluation order, or a source-to-CKIR7 lowering relation.
