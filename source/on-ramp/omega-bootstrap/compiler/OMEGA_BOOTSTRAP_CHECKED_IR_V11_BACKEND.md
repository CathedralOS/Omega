# Conservative CKIR11 backend implementation note

[`CKIR11`](OMEGA_BOOTSTRAP_CHECKED_IR_V11.md) inherits CKIR10's complete table
layout and opcodes 1 through 21. It selects an already-defined opcode 8 `Add`
relation rather than adding an opcode: at least one operation has identical
canonical `u32 in Trapping` operands and result. CKIR11 retains opcode 21
`IntegerWiden` as an optional inherited operation; CKIR10 continues to require
it under its own frozen major.

The shared `omega-bootstrap-checked-ir-v5-to-elf.alp` implementation accepts
schema majors 4 through 11. For major 11 it independently reconstructs at
least one selected Add row. Other inherited Add rows remain governed by their
original CKIR rules and do not satisfy the CKIR11 feature requirement. Earlier
schema relations and artifact bytes remain frozen.

The selected operation reuses the existing conservative x86-64 sequence:

```text
mov eax, dword ptr [rbp - left_slot]
add eax, dword ptr [rbp - right_slot]
jc trap
cmp eax, 0
jb trap
cmp eax, 2147483647
ja trap
mov dword ptr [rbp - result_slot], eax
```

The two `cmp`/branch pairs are retained even though the low bound is zero and
the preceding carry test already constrains unsigned addition. CKIR's
conservative template is a stable artifact contract: static source bounds do
not authorize this backend to erase the declared result-range check. The dry
sizing pass emits the identical byte count, and every trap branch targets the
existing shared `ud2` stub. No new frame slot kind, table, allocator, or text
ceiling is introduced.

The focused split is:

- `../gates/delta-checked-ir-v11-reference.sh` independently owns schema,
  selected-feature, meaning, and mutation checks;
- `../gates/delta-checked-ir-v11-backend-fixture.py` recognizes the complete
  Add/carry/range/store sequence rather than merely searching for an `add`
  instruction; and
- `../gates/delta-checked-ir-v11-backend.sh` checks native/self artifact
  identity, 0+70, 69+1, near-limit success, instruction mutation rejection,
  the retained runtime-overflow trap path, resource statuses, and empty rejected
  publication.

Because CKIR11 introduces a required profile relation rather than a new opcode,
the same table body can be valid CKIR8 under major 8. The shared historical
backend correctly continues to accept that old artifact under its own schema;
the CKIR11-specific reference rejects it as a cross-major input. A version bump
does not retroactively invalidate an older independently valid module.

This is backend evidence only. It does not define source syntax, full-width
Omega `u32`, a public ABI, or a general arithmetic-policy implementation.
