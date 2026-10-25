# Case payload transport

A multi-variant Outcome sum carries its active payload value <= 50 across the place so matching Outcome::Ready adds 10 within result <= 60.

Every place declares copyable `data Outcome [copy] { case Empty; case Ready(value: u64) where value <= 50; }` and transports the sum across the designated place. Destructuring `Outcome::Ready { value }` evaluates `value + 10` under postcondition `ensures result <= 60`. The test validates `Outcome::Ready { value: 32 }` yields `42` and `Outcome::Empty {}` yields `0`, exiting with 0. Only the transport mechanism of the sum value changes across places.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | root case payload transport |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | copied into a local binding |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | stored in and read from a record field |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | passed as an explicit machine parameter |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | returned from a helper machine call |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | joined from conditional branch arms |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | named copyable constant binding |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | forwarded across an explicit machine state transition |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | forwarded through a helper machine |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | realized across four target-scoped machine bodies |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` and declares
`Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/case_payload_transport/main.omg
omega run --both samples/cli/facts/case_payload_transport/local/main.omg
omega run --both samples/cli/facts/case_payload_transport/field/main.omg
omega run --both samples/cli/facts/case_payload_transport/parameter/main.omg
omega run --both samples/cli/facts/case_payload_transport/call_result/main.omg
omega run --both samples/cli/facts/case_payload_transport/match_join/main.omg
omega run --both samples/cli/facts/case_payload_transport/const_name/main.omg
omega run --both samples/cli/facts/case_payload_transport/other_state/main.omg
omega run --both samples/cli/facts/case_payload_transport/other_machine/main.omg
omega run --both samples/cli/facts/case_payload_transport/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- The oracles were proved to discriminate before they were recorded. Rewriting the transported payload from `32` to `31` in each place — so the same program reaches the same destructuring and merely compares `41` where the caller requires `42` — makes both engines trap: `native signal 5` beside `interp crash: Trap (host presentation agrees)`, exit `255`. An exit-0 oracle that could not be broken by a corrupted payload would have pinned nothing.
- In every reject twin, the payload bound is widened from `value <= 50` to `value <= 80`. Because `value + 10` can evaluate up to 90, the compiler fails proof checking and rejects compilation with `cannot prove ensures contract for exit from ...: result <= 60`. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/case_payload_transport/<place>/invalid/main.omg
```

### Acceptance Gate

```sh
python tools/sample_native_gate.py --cli C:\SoftwareDevelopmentKits\Omega\build\omega-latest.exe --filter cli/facts/case_payload_transport --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64` (and `linux_x86_64`).
- 9/9 intended-invalid controls match their authored rejection (`cannot prove ensures contract`).

## Pinned Fixtures

Nine positive fixtures live under
[`tests/omega/pass/facts/case_payload_transport_through_<place>_exit`](../../../../tests/omega/pass/facts)
with `expected_native_exit.txt` of `0`, and nine reject twins under
[`tests/omega/fail/facts/case_payload_transport_through_<place>_rejected`](../../../../tests/omega/fail/facts).
Each twin differs from its positive place in one line — the payload bound
`value <= 50` widens to `value <= 80` — leaving the transport, the caller and the
consuming operation identical.

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` / `STATIC-MACHINE-MIGRATION` (`TASKS.md`).
- **Ticket:** `7eb79955174c45268e495b9239a45bd5` (previous `f665f940b5224387926cac45f2b04b8d`).
- **Requirement established:** End-to-end nine-place fact preservation and native realization for tagged `case` variant payload bounds across value transport. Specifically, constructing or matching a `case` variant carrying a payload (`Outcome::Ready(value: u64) where value <= 50`) preserves the payload's static bound without dropping path-bound facts or requiring caller equality guards, with exact agreement between Terminal Psi and native execution across hosted targets (`omega run --both` exit 0).

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Constructing `Outcome::Ready { value: 32 }` or matching a `case` variant carrying a payload establishes the payload's static bound (`value <= 50`) directly from the variant arm/contract without any post-hoc caller equality guard (`transition x == 42`).
  - **Identity and lifetime:** The tagged `case` discriminant tag and variant payload retain their exact storage/projection identity across moves, copies (`[copy]`), and pattern matches. Destructuring binds the variant payload to an exact scalar subject without storage aliasing or corruption of inactive variant payload bytes.
  - **Composition:** The `case` payload bound composes through all 9 fact places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming addition operation (`value + 10`) under `ensures result <= 60`. In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the variant payload bound (`value <= 50` widened to `value <= 80`), while preserving all variant declarations, matching logic, and caller structures.
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed during proof checking when the payload bound is widened to `value <= 80`, because `value + 10` can evaluate up to 90 and cannot prove `ensures result <= 60`. All 9 twins fail closed with compile-time error `cannot prove ensures contract for exit from ...: result <= 60`.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Tagged `case` construction, discriminant tag assignment, branch dispatch, and payload extraction lower into canonical target-neutral Terminal Psi (`PSITERM\0`), erasing proof annotations while preserving variant discriminant switching and concrete payload offsets. All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) through companion build declarations without reopening source. Tagged `case` layout geometry (`size_of`/`align_of`) validates: `Outcome` layout contains a discriminant tag followed by aligned payload storage (`value: u64`), total size 16, align 8. `CallPlan` and `StatePlan` lower `case` carriers across registers, stack slots, or indirect aggregate return homes (`sret`) per platform ABI.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects tag comparisons, conditional jumps for match arms, payload loads/stores (`[base + payload_offset]`), and 64-bit integer addition (`add`). Register homes and activation stack-frame slots are assigned for `Outcome` instances, and entry prologues and multi-return epilogues honor the `StatePlan` ceiling.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic stack-frame displacements and variant payload offsets resolve into final machine instructions, match-arm basic blocks and jump targets relax with offset reflow, machine-state footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates against target host limits, and direct PE/ELF/Mach-O executables are emitted. Terminal Psi interpretation and native execution agree with exit 0 (`omega run --both`) across host platforms including `windows_x86_64` and `macos_arm64`.

- **What remains open:** Reference borrowing of `case` payloads (`&payload`), recursive enum variants, and non-copyable affine resource cleanup in discarded variant payloads.
