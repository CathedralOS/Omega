# Conservative CKIR10 backend implementation note

[`CKIR10`](OMEGA_BOOTSTRAP_CHECKED_IR_V10.md) inherits CKIR9's complete table
layout and opcodes 1 through 20. It adds opcode 21 `IntegerWiden`. A CKIR10
module must contain at least one opcode-21 row; inherited operations are
optional.

The shared `omega-bootstrap-checked-ir-v5-to-elf.alp` implementation accepts
schema majors 4 through 10. Opcode 21 requires one visible exact unqualified
`u8` value operand, the exact canonical `u32 in Trapping` result type, and zero
immediate and reserved fields. It rejects under every earlier schema. Earlier
schema relations, templates, and ELF bytes remain frozen.

After validation the backend emits this exact conservative x86-64 template:

```text
mov eax, dword ptr [rbp - source_slot]
movzx eax, al
mov dword ptr [rbp - result_slot], eax
```

The instructions begin with bytes `8b 85`, `0f b6 c0`, and `89 85`; the two
memory instructions are followed by signed 32-bit frame displacements. The
source's validated range is 0 through 255 and inherited scalar slots are
zero-extended, so this template preserves the unsigned payload and produces
the canonical four-byte target carrier. No trap, call, allocation, policy
dispatch, or extra runtime check occurs.

The dry sizing pass uses the same emitter. No separate text accounting, table,
frame slot, or ceiling is introduced.

Focused responsibility is split between
`../gates/delta-checked-ir-v10-reference.sh`, which owns independent schema and
meaning checks; `../gates/delta-checked-ir-v10-backend-fixture.py`, which pins
the exact instruction sequence. The former producer-backed wrapper joined them
to native/self artifact identity, 0/70/255, instruction mutation rejection,
and empty rejected publication. Replay is suspended until canonical Delta
publication.

This is backend evidence only. It does not define source syntax, a public ABI,
or a general cast relation.
