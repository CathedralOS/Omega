# Alpha tests

These tests exercise the Alpha VM semantics and its independent reference. The
audited VM implementations and normative semantics remain in `bootstrap/0_alpha/`.

| Retained child/files | Role | Deletion condition |
| --- | --- | --- |
| `conformance.sh` | Pins every Alpha opcode and the selected seed profile. | Delete only when a stronger executable conformance gate subsumes every case. |
| `bounds.py` | Hand-encoded bounds cases shared by native and reference checks. | Delete when stronger conformance controls subsume these exact/adjacent observations. |
| `reference/` | Independent VM differential checks. | Delete when checked native correspondence subsumes the diagnostic. |
| `tape-assembly/` | Off-chain assembler reconstruction, differential, grammar, and example tests. | Delete with the tool or when stronger checked coverage subsumes every relation. |

## Bounds conformance

`sh tests/alpha/conformance.sh` runs the native bounds cases through Python 3;
`sh tests/alpha/reference/diamond-py.sh` runs the same short cases against the
independent reference. Both commands work from the repository root on macOS
arm64 and Windows x64 with Git Bash, Python 3, and the existing bootstrap tools.
The native runner stamps copies directly to test hostile embedded lengths that
the normal stamper correctly refuses. It compares exact stdout (including empty
bytes), empty stderr, and native illegal-instruction termination rather than
accepting arbitrary process failures. A 300-second per-case watchdog fails,
never counts as a semantic Trap.

Coverage includes all 21 instruction widths at the semantic memory end, adjacent
missing operands, invalid/wrapping targets and data addresses, unaligned and
full-word round trips, untaken invalid targets, ordinary mutable code, overlapping
call operands/return storage, and loader extents. An exact-end non-control
instruction can still trap on the following fetch; its external observation
alone cannot prove the preceding effect occurred. Review the native width tables
and handlers alongside these controls.

Native-only full-profile loops check the last legal push at `sp = 8`, the next
push at zero, the last legal return word, the next return at `MEMSIZE`, and a
valid call after returning to `sp = MEMSIZE`. They use 33,554,432 calls or
201,326,592 returns, not a reduced profile or debugger-modified stack pointer.
Run these alone with `python3 tests/alpha/bounds.py --stack-only`. The reference
excludes these large loops explicitly; it still checks return without a preceding
call, access above the stack origin, and overlapping call storage.

macOS arm64 execution is validated for the identities in the
[seed inventory](../../bootstrap/0_alpha/README.md#retention-inventory).
Windows native execution remains outstanding; exact PE reconstruction and source
review do not establish that host result. These tests do not discharge native
correspondence proofs or the separate Windows register-storage defect.
