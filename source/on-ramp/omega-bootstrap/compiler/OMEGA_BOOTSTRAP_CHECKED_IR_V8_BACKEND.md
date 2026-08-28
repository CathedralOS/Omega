# Conservative CKIR8 backend implementation note

[`CKIR8`](OMEGA_BOOTSTRAP_CHECKED_IR_V8.md) inherits the complete CKIR7 table
layout and opcodes 15 through 17. It adds primitive opcode 18 `ScalarEqual`.
A CKIR8 module must contain at least one opcode-18 operation; the inherited
logical operations remain optional.

The shared `omega-bootstrap-checked-ir-v5-to-elf.alp` implementation accepts
schema majors 4 through 8. Each scalar-equality row must have an exact Boolean
value result, two visible value operands with the same `bool`, `u8`, or `u32`
carrier kind, and zero immediate and reserved fields. Opcode 18 rejects under
every earlier schema. Earlier schema decoders, validation relations,
instruction templates, and ELF bytes remain frozen.

After validation, the backend emits this exact conservative x86-64 template:

```text
mov eax, dword ptr [rbp - left_slot]
cmp eax, dword ptr [rbp - right_slot]
sete al
movzx eax, al
mov dword ptr [rbp - result_slot], eax
```

The load, compare, `SETE`, `MOVZX`, and store begin with bytes `8b 85`, `3b 85`,
`0f 94 c0`, `0f b6 c0`, and `89 85`, respectively. The memory instructions are
followed by their signed 32-bit frame displacement. All admitted values occupy
the inherited zero-extended four-byte scalar slot, so this sequence computes
exact carrier equality and materializes canonical Boolean zero or one. It does
not define record or sum equality and performs no constant folding, widening,
coercion, or source-level evaluation-order rewrite.

The ordinary dry sizing pass uses the same emitter, so no separate text-size
accounting is introduced. All existing table, frame, text, ELF, operation,
operand, and value ceilings remain unchanged.

Focused evidence is split by responsibility:

- `../gates/delta-checked-ir-v8-reference.sh` checks independent decoding,
  Boolean/`u8`/`u32` meaning, same-carrier enforcement, schema and opcode
  isolation, malformed-row rejection, and inherited resource ceilings;
- `../gates/delta-checked-ir-v8-backend-fixture.py` recognizes the exact
  load/CMP/SETE/MOVZX/store instruction template.

The former producer-backed wrapper joined them to native/self artifact
identity, result 70, `SETE` mutation rejection, and empty rejected publication.
Replay is suspended until canonical Delta publication.

This is backend evidence only. It does not define a public Omega ABI, source
syntax or evaluation order, or a source-to-CKIR8 lowering relation.
