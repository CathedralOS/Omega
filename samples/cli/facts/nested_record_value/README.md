# Nested Record Value Transport

A nested record `Envelope` preserves its inner `Header` bound `header.limit <= 50` and sibling `payload` across value transport so `header.limit + payload <= 100` in all nine places.

Every place performs the same operation: add the inner header limit (`<= 50`) and sibling payload (`<= 50`) of an `Envelope` and return the sum under `ensures result <= 100`. The oracle verifies that `envelope.header.tag == 7` remains untouched and the sum is `42` (`limit = 20, payload = 22`), exiting 0 only when both hold. Only the value transport of the nested record changes across places.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | root nested record value transport |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | envelope is copied into a local variable before reading fields |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | an enclosing packet holds the envelope in a field before reading |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | machine reads fields directly from its explicit parameter |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | envelope is returned from a contracted forwarding call |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms yield an envelope, preserving bounds across the join |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | inner bound is governed by a named constant |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | successor machine receives the envelope and sums its fields |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | forwarding machine passes the envelope to another machine |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each of four target-scoped machine bodies preserves the bound |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` and declares
`Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/nested_record_value/main.omg
omega run --both samples/cli/facts/nested_record_value/local/main.omg
omega run --both samples/cli/facts/nested_record_value/field/main.omg
omega run --both samples/cli/facts/nested_record_value/parameter/main.omg
omega run --both samples/cli/facts/nested_record_value/call_result/main.omg
omega run --both samples/cli/facts/nested_record_value/match_join/main.omg
omega run --both samples/cli/facts/nested_record_value/const_name/main.omg
omega run --both samples/cli/facts/nested_record_value/other_state/main.omg
omega run --both samples/cli/facts/nested_record_value/other_machine/main.omg
omega run --both samples/cli/facts/nested_record_value/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- The probe machine computes `sum = envelope.header.limit + envelope.payload`. The main transition checks `envelope.header.tag == 7 && sum == 42`. If either the untouched tag field is corrupted or the sum fails to compute correctly, execution transitions to `machine bad() { crash Trap; }`, causing a native signal 5 / interpreter `Trap` crash (exit 255).
- Each invalid twin removes or widens the inner header bound (`header.limit <= 50`), so the sum is no longer provably in range. 8 twins fail closed with compile-time proof obligation error `exact arithmetic in machine \`sum_envelope\` state \`entry\` terminal expression may overflow \`u64\``, while `const_name/invalid/main.omg` fails closed with `cannot prove ensures contract for exit from sum_envelope at statement 0: result <= 100`.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/nested_record_value/<place>/invalid/main.omg
```

### Measured discrimination

Corrupting the inner member's own value (`header: Header { tag: 7, limit: 20 }` ->
`limit: 19`) while leaving the outer record and the sibling `tag` untouched changes
the observed result, so the authored exit 0 is a live oracle rather than a vacuous
success. Both engines must then trap:

```
native signal 5
interp crash: Trap (host presentation agrees)
```

Measured on `macos_arm64` at `c14e052cf9d` for `local` and `other_machine`; each
returns exit 255 against the authored exit 0, and both restore to
`native exit 0` / `interp exit: 0`.

### Acceptance Gate

```sh
python3 tools/sample_native_gate.py --cli ./target/debug/omega --filter cli/facts/nested_record_value --jobs 4
python3 tools/corpus_gate.py --native --filter facts/nested_record_value
```

`sample_native_gate.py` reports, measured on `macos_arm64` at `c14e052cf9d`:
- 10/10 samples meet their documented oracle (0 unscored; check 89s, review 206s, run 200s).
- 9/9 intended-invalid controls match their authored rejection (`may overflow \`u64\``, or `cannot prove ensures contract` for `const_name`), with no `rejection-failed`.

`corpus_gate.py --native` reports `clean` for the nine
`tests/omega/pass/facts/nested_record_value_through_<place>_exit` fixtures:
9 `built`, 9 executed, `exit codes {'0': 9}`. Their golden rows live in
`tests/omega/corpus_native_macos_arm64.txt`. The matching reject twins are pinned
under `tests/omega/fail/facts/nested_record_value_through_<place>_rejected/` with
`expected.txt`.

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`).
- **Ticket:** `5116b13df42742d9839df19a9be7fc4a`.
- **Requirement established:** Compound value preservation across value transport. Specifically, value transport (across locals, fields, parameters, call returns, branch joins, named constants, state transitions, machine calls, and target-scoped bodies) preserves nested record invariants and sibling fields without dropping path-bound facts or perturbing untouched payload fields, with exact agreement between Terminal Psi and native execution across hosted targets (`omega run --both` exit 0).

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Constructing or receiving the nested structural record `Envelope { header: Header { tag: 7, limit: 20 }, payload: 22 }` or checking preconditions `envelope.header.limit <= 50 && envelope.payload <= 50` establishes the inner field's static bound (`header.limit <= 50`) and sibling scalar (`payload <= 50`) directly, without relying on any post-hoc caller equality guard (`transition x == 42`) to justify forming the sum `header.limit + payload` under `ensures result <= 100`.
  - **Identity and lifetime:** Multi-hop nested field paths (`outer.inner.field`, i.e., `envelope.header.limit` and `envelope.header.tag`) retain their exact storage/projection identity across copies, moves, and borrows as pure structural value types (`[copy]`). Unrelated field writes or sibling payload projections do not invalidate nested header field facts, and unread fields (`envelope.header.tag == 7`) retain their exact identity across all value transport mechanisms.
  - **Composition:** The nested record field fact composes through all 9 fact places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming addition operation and ensures contract. Callee calls, successor transitions, and match joins compose exact actuals and preserve preconditions on all incoming paths. In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ only in the nested field value, constant, or contract bound (omitting `envelope.header.limit <= 50` or widening `LIMIT_CEILING` to 100).
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed during proof checking when the inner field bound is weakened or omitted, preventing proof that `local.header.limit + local.payload` is within bounds without overflow. 8 twins fail closed with compile-time proof obligation error `exact arithmetic in machine \`sum_envelope\` state \`entry\` terminal expression may overflow \`u64\``, while `const_name/invalid/main.omg` fails closed with `cannot prove ensures contract for exit from sum_envelope at statement 0: result <= 100`.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Nested structural record construction, value copying, and multi-hop field projections (`envelope.header.limit`) lower into canonical target-neutral Terminal Psi (`PSITERM\0`), erasing proof obligations while retaining concrete record layout and field projection offsets. All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) through companion build declarations without reopening source. Nested structural record layout plans (`size_of`/`align_of`) validate: `Header` (`tag: u64`, `limit: u64`, size 16, align 8) and `Envelope` (`header: Header` at offset 0, `payload: u64` at offset 16, total size 24, align 8), composing outer + inner byte offsets (`envelope.header.limit` at byte offset 8). `CallPlan` and `StatePlan` lower nested record carriers across registers, stack slots, or indirect aggregate return homes (`sret`) per platform ABI.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects composed-offset memory/register loads (`[base + 8]` for `header.limit`, `[base + 16]` for `payload`), register moves, and 64-bit integer addition (`add`). Low-level register allocators assign register homes and disjoint activation stack-frame slots for record copies, and entry prologues and multi-return epilogues honor the `StatePlan` ceiling.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic stack-frame displacements and nested field offsets resolve into final machine instructions, function basic blocks and conditional transitions relax with offset reflow, machine-state footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates against target host limits, and direct PE/ELF/Mach-O executables are emitted. Native execution and Terminal Psi interpretation agree with exit 0 (`omega run --both`) across host platforms including `windows_x86_64` and `macos_arm64`.

- **Pinned evidence:** Nine positive fixtures at `tests/omega/pass/facts/nested_record_value_through_<place>_exit/` (each binding `ProgramEntry` for all four hosted targets with `expected_native_exit.txt` = `0`), with their nine measured golden rows in `tests/omega/corpus_native_macos_arm64.txt`. Nine reject twins at `tests/omega/fail/facts/nested_record_value_through_<place>_rejected/` with authored `expected.txt`. The row was measured, not repaired: all ten places already ran green on both engines, and the gap was that no native golden row had been recorded.

- **What remains open:** Reference and mutable loan transport of nested record fields (`&envelope.header.limit`), partial field assignment invalidation of sibling path facts, and dynamic partition conservation under `core::content`.
