# Conservative CKIR9 backend implementation note

[`CKIR9`](OMEGA_BOOTSTRAP_CHECKED_IR_V9.md) inherits the complete CKIR8 table
layout and opcodes 15 through 18. It adds unsigned primitive opcode 19
`Greater` and opcode 20 `GreaterEqual`. A CKIR9 module must contain at least one
of those two operations; inherited logical and equality operations remain
optional.

The shared `omega-bootstrap-checked-ir-v5-to-elf.alp` implementation accepts
schema majors 4 through 9. Each new ordered row must have an exact Boolean
value result, two visible value operands with the same `u8` or `u32` carrier
kind, and zero immediate and reserved fields. Opcodes 19 and 20 reject under
every earlier schema. Opcode 18 remains valid in CKIR9 but is required only by
exact CKIR8. Earlier schema validation relations, instruction templates, and
ELF bytes remain frozen.

After validation, the backend emits one of these exact conservative x86-64
templates without exchanging operands:

```text
mov eax, dword ptr [rbp - left_slot]
cmp eax, dword ptr [rbp - right_slot]
seta al                              # opcode 19
movzx eax, al
mov dword ptr [rbp - result_slot], eax
```

```text
mov eax, dword ptr [rbp - left_slot]
cmp eax, dword ptr [rbp - right_slot]
setae al                             # opcode 20
movzx eax, al
mov dword ptr [rbp - result_slot], eax
```

The load, compare, condition-code, zero-extension, and store begin with bytes
`8b 85`, `3b 85`, `0f 97 c0` or `0f 93 c0`, `0f b6 c0`, and `89 85`,
respectively. The memory instructions are followed by their signed 32-bit frame
displacement. All admitted values occupy inherited zero-extended four-byte
scalar slots, so the unsigned x86-64 conditions compute exact `u8`/`u32`
carrier ordering and materialize canonical Boolean zero or one.

The ordinary dry sizing pass uses the same emitter. No separate text-size
accounting, table, frame slot, or ceiling is introduced.

Focused evidence is split by responsibility:

- `../gates/delta-checked-ir-v9-reference.sh` checks independent decoding,
  unsigned truth rows, same-carrier enforcement, inherited-operation
  composition, schema/opcode isolation, malformed-row rejection, and inherited
  resource ceilings;
- `../gates/delta-checked-ir-v9-backend-fixture.py` recognizes the exact
  load/CMP/SETA-or-SETAE/MOVZX/store templates associated with each opcode; and
- `../gates/delta-checked-ir-v9-backend.sh` checks Delta-native/self artifact
  identity, result 70, condition-code mutation rejection, and no partial
  artifact publication for rejected inputs.

This is backend evidence only. It does not define a public Omega ABI, source
syntax, source evaluation order, or transition-fact relation.
