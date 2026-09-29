# Alpha tests

These tests exercise the Alpha VM semantics and its independent reference. The
audited VM implementations and normative semantics remain in `bootstrap/0_alpha/`.

| Retained child/files | Role | Deletion condition |
| --- | --- | --- |
| `conformance.sh` | Pins every Alpha opcode and the selected seed profile. | Delete only when a stronger executable conformance gate subsumes every case. |
| `container.sh`, `container.py` | Validates both audited seed containers as native executables on every host: bound identity for the non-host seed too, PE32+/Mach-O structure, hole offset equal to the tape section's raw extent, and the stamped-artifact contract. | Delete only when an identity or provenance gate covers the same native-container contract. |
| `bounds.py` | Hand-encoded bounds cases shared by native and reference checks. | Delete when stronger conformance controls subsume these exact/adjacent observations. |
| `native_exit.py` | Runs a stamped seed through a native subprocess and reduces its raw exit DWORD to the POSIX surface, restoring byte-level exit expectations on MSYS hosts whose shell collapses every other sign-bit-set code to 127. | Delete when no seed-executing host normalizes process exit codes this way. |
| `io-registers.hex` | Shared raw-tape regression for host scratch/register isolation. | Delete when stronger I/O conformance checks subsume its full-word and operand controls. |
| `parity.sh` | On Linux x86-64, runs the conformance opcode battery plus the diamond edge corpus through both the committed `alpha_x64_linux` container and the independent reference, failing on any exit-code or stdout disagreement. | Delete when the reference interpreter is deleted, or when host-seed agreement coverage reaches the Linux container another way. |
| `reference/` | Independent VM differential checks. | Delete when checked native correspondence subsumes the diagnostic. |
| `tape-assembly/` | Off-chain assembler reconstruction, differential, grammar, and example tests. | Delete with the tool or when stronger checked coverage subsumes every relation. |

## Bounds conformance

`sh tests/alpha/conformance.sh` runs the native bounds cases through Python 3;
`sh tests/alpha/reference/diamond-py.sh` runs the same short cases against the
independent reference. Both commands work from the repository root on macOS
arm64, Windows x64 with Git Bash, and Linux x86-64 with Python 3 and the
existing bootstrap tools.
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

macOS arm64 and Linux x86-64 execution are validated for the identities in the
[seed inventory](../../bootstrap/0_alpha/README.md#retention-inventory).
These tests do not discharge native correspondence proofs.

Windows x64 execution is measured on Git Bash with the selected seed:
`bounds.py` passes 78/78 native cases, the Beta reconstruction tape reproduces
its compiler byte-identically, `io-registers.hex` returns 0 with stdout
`ABCDEF` for input `AB`, and all 34 `conformance.sh` cases pass. The former
`div_zero_trap`/`div_ovf_trap` divergence — the hardware divide faults
`0xC0000094`/`0xC0000095` surfacing instead of the audited illegal-instruction
routine — is closed by the divisor pre-check the PE seed now shares with the
Linux and arm64 seeds, so every trap class reports `0xC000001D` (132 through
the shell).

Git Bash still normalizes the raw exit DWORD: every sign-bit-set code other
than `0xC000001D` observes as 127, so a halt whose register sign-extends
(`div_neg` = 253, `mod_neg`/`read_eof` = 255, the diamond's `jlt_signed`)
hides its authored byte behind `$?`. The cases' VM result was always correct —
PowerShell reads `$LASTEXITCODE = -3` for `-7/2` — the loss was the MSYS
shell's observation. `conformance.sh` and `diamond-py.sh` therefore run the
stamped container through `native_exit.py` on these hosts
(`ALPHA_SEED_EXIT_COLLAPSED` in `seed_env.sh`): a Windows-native subprocess
sees the real DWORD, which the helper reduces to the POSIX surface — 132 for
the audited `0xC000001D` trap, the raw DWORD for any foreign exception (so a
leaked `0xC0000094` can never alias a legal expectation), and the authored
low byte otherwise.

## Register isolation

The same two shell gates include `io-registers.hex`: a 438-byte raw tape with
comments, not assembler output. Given stdin `AB`, it must exit zero with exact
stdout `ABCDEF`. It checks registers 16–19 start at zero, retain distinct 64-bit
sentinels across read/write/EOF through another register, preserve their own
values when used as write operands, and receive EOF when used as read operands.
This catches the Windows seed's former overlap of those registers with its host
handles, byte buffer, and count slot. The native conformance gate pins the result;
the reference gate independently compares it. The fixture passes on macOS arm64,
Windows x64, and the Python reference.
