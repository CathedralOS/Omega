# Structural borrow identity — matching-host runtime records

Records ledger for the one open leg of **STRUCTURAL-BORROW-IDENTITY**
(TASKS.md): "record matching-host runtime results for both Linux targets
and Windows". Procedure lives in
[structural_borrow_identity_host_legs.md](structural_borrow_identity_host_legs.md);
this file holds the verdicts. A record counts only when the emitted bytes
were linked and executed on the matching host — cross-emission, object
publication, and checked-only passes are not runtime coverage. Do not
record a leg that was not executed; an open row is the honest state.

## Harness under test

`mbx nextest run -p omega-native-differential-test --test terminal_psi_indexed_receivers`
(`cargo nextest run -p omega-native-differential-test --test
terminal_psi_indexed_receivers` equivalent) — 17 submodules, 79 tests;
`owned_subloans` and siblings publish objects, images and installation
records for `NativeTarget::{linux_x64, linux_arm64, macos_arm64,
windows_x64}` and execute only when `target == NativeTarget::host()`.
Controls that must stay green beside each leg: the full
`terminal_psi_indexed_receivers` target and `primitive_store_return`.

## Records

| Host | SHA under test | Run | Result | Evidence |
| --- | --- | --- | --- | --- |
| macOS arm64 | (earlier wave) | full harness | Recorded | emitted bytes linked + executed through host C driver |
| Linux x86-64 | (earlier wave) | full harness, 79/79 | Recorded | every emitted byte sequence linked and executed through the host C driver |
| Linux aarch64 | — | — | Open | needs a linux-aarch64 host or a QEMU harness (none in-repo) |
| Windows x86-64 | — | — | Open | needs a Windows host; execution route is `VirtualAlloc`/`VirtualProtect`/`FlushInstructionCache` + direct call, no `cc` |

Re-record a row when the harness, the fixtures, or the target codegen
changes beneath it; keep the SHA beside each row.

## What a row must show

Host triple, SHA under test, pass counts, and the sentence that every
emitted byte sequence was linked and executed through the host driver.
A green run whose non-host legs skipped is expected — the recorded leg
is the host's own. A red matching-host leg is a real
structural-borrow-identity defect: route it to the owning lanes per the
recipe's boundaries, never weaken a fixture to force a record.
