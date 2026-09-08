# Alpha tests

These tests exercise the Alpha VM semantics and its independent reference. The
audited VM implementations and normative semantics remain in `bootstrap/0_alpha/`.

| Retained child/files | Role | Deletion condition |
| --- | --- | --- |
| `conformance.sh` | Pins every Alpha opcode and the selected seed profile. | Delete only when a stronger executable conformance gate subsumes every case. |
| `reference/` | Independent VM differential checks. | Delete when checked native correspondence subsumes the diagnostic. |
| `tape-assembly/` | Off-chain assembler reconstruction, differential, grammar, and example tests. | Delete with the tool or when stronger checked coverage subsumes every relation. |

The current committed seeds predate the ratified bounds checks in
[`SEMANTICS.md`](../../bootstrap/0_alpha/SEMANTICS.md#8-bounds-and-fixed-capacity).
`conformance.sh` adds runtime fetch/data/stack and loader-trap cases atomically
with the native hardening; existing in-bound and trap coverage does not claim
that implementation has landed.
