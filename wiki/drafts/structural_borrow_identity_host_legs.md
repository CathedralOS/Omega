# Structural borrow identity — host-runtime recording recipe

Purpose: recipe for the one open leg of **STRUCTURAL-BORROW-IDENTITY**
(TASKS.md) — "record matching-host runtime results for both Linux targets
and Windows". Every other leg (mixed field/index receiver paths, receiver
preparation, native lowering, replay) is landed; this is the recording
procedure, not new machinery. Delete this draft once all four hosted-target
legs are recorded on that row (or the row is retired).

## What the harness is

`tests/native-differential/tests/terminal_psi_indexed_receivers.rs`
(package `omega-native-differential-test`), 17 submodules, 79 tests.
`owned_subloans` and the sibling modules publish objects, images and
installation records for all four hosted targets —
`NativeTarget::{linux_x64, linux_arm64, macos_arm64, windows_x64}` — then
execute only when `target == NativeTarget::host()`. A non-matching leg
still publishes bytes and continues; that is cross-emission, not runtime
coverage, and the row calls it out as such.

Execution mechanism per host:

- Linux x86-64 / aarch64 and macOS arm64:
  `tests/native-differential/tests/common/native_function.rs::assert_c_text`
  writes the emitted bytes as a `.byte` assembly with `omega_entry`
  (`_omega_entry` on macOS) aliased to `.Lomega_text + <entry_offset>`,
  emits `.section .note.GNU-stack` on non-macOS, links with `cc` and runs
  the produced binary. Requires a host C toolchain on `PATH`.
- Windows x86-64:
  `tests/native-differential/tests/pipeline_ownership/native_execution.rs`
  — kernel32 `VirtualAlloc` RW pages, copy emitted bytes, `VirtualProtect`
  to RX (W^X), `FlushInstructionCache`, direct call; no C driver needed.

## Recorded state

| Host | State |
| --- | --- |
| macOS arm64 | Recorded (earlier wave) |
| Linux x86-64 | Recorded — full harness 79/79 on this host, every emitted byte sequence linked and executed through the host C driver |
| Linux aarch64 | Open — needs a linux-aarch64 host or a QEMU harness (none in-repo) |
| Windows x86-64 | Open — needs a Windows host |

## Per-host recipe

1. Check out the commit being recorded (board tip or the change under
   review) in a worktree; the squalr submodule is not needed for this leg.
2. Install prerequisites: Rust + nextest; plus `cc`/`clang` on Linux and
   macOS hosts. The Windows leg needs only the host toolchain — execution
   goes through `VirtualAlloc`/`VirtualProtect`, not `cc`.
3. Run the harness:

   ```text
   mbx nextest run -p omega-native-differential-test --test terminal_psi_indexed_receivers
   ```

   (`cargo nextest run -p omega-native-differential-test --test
   terminal_psi_indexed_receivers` is the equivalent fallback.)

4. Record on the row: host triple, SHA under test, pass counts, and the
   sentence that every emitted byte sequence was linked and executed
   through the host driver. A green run where `target != host()` legs
   skipped is expected — the recorded leg is the host's own.
5. Preserve the controls: `terminal_psi_indexed_receivers` and
   `primitive_store_return` must stay green alongside.

## Boundaries

- Do not weaken fixtures to force a green leg; a failure here is a real
  structural-borrow-identity defect and routes to the owning lanes
  (`execution/terminal_unit/receiver_calls`, `src/tests/borrow`, the call-operation
  and reconciliation surfaces are claimed under the item's own claim
  ledger).
- A checked-only pass, object publication, or cross-emission is not the
  witness — matching-host execution is.
- Acceptance echo from the row: caller-visible writes, forwarded
  references, legal synchronized shared observations, write-only
  non-reading, and register/stack pointer passing work on both Linux
  targets, including an owned local's field lent `&mut` and `&write`;
  independently formed or substituted access/shape/placement pairs reject;
  a following callee seeing the staged write is not caller-visible
  writeback.

Recipe verified at `2dbfecd98e` (linux x86-64): the harness layout is
unchanged — 17 submodules, 79 tests total across the
`terminal_psi_indexed_receivers` tree; `common/native_function.rs:17`
`assert_c_text` still emits `omega_entry`/`_omega_entry` over
`.Lomega_text + <entry_offset>` and `.note.GNU-stack`; the Windows leg
still runs `VirtualAlloc`/`VirtualProtect`/`FlushInstructionCache` in
`pipeline_ownership/native_execution.rs`. The recorded-state table stays
accurate: linux-aarch64 and Windows x86-64 legs remain open and
host-gated — no in-repo QEMU harness exists.
