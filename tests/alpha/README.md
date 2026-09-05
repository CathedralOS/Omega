# Alpha tests

These tests exercise the Alpha VM semantics and its independent reference. The
audited VM implementations and normative semantics remain in `bootstrap/alpha/`.

| Retained child/files | Role | Deletion condition |
| --- | --- | --- |
| `conformance.sh` | Pins every Alpha opcode and the selected seed profile. | Delete only when a stronger executable conformance gate subsumes every case. |
| `reference/` | Independent VM differential checks. | Delete when checked native correspondence subsumes the diagnostic. |
| `tape-assembly/` | Off-chain assembler reconstruction, differential, grammar, and example tests. | Delete with the tool or when stronger checked coverage subsumes every relation. |
