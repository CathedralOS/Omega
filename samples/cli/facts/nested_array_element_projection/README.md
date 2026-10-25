# Nested Array Element Projection

Fact: indexing a fixed-size array field inside a structural record (row.cells[idx]) with a bounded index (idx < 4u64) proves safe element projection and preserves the element bound (elem <= 80u64) to discharge exact subtraction 80u64 - elem across all nine places.

Every place performs the same operation: index a fixed-size array field inside a structural record (`Row { cells: [u64; 4] }`) using a verified bounded index (`idx < 4`), extract element `elem = row.cells[idx]` with bound `elem <= 80u64`, discharge exact subtraction `80u64 - elem`, verify `probe(row, 2) == 30` (since `cells[2] == 50u64`, `80u64 - 50u64 == 30u64`), and exit 0. Only the transport mechanism of the record, index, and element projection changes across places.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | root record array element projection and exact subtraction |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | the record and index are copied into locals before indexing the array and subtracting |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | an enclosing structural record holds the row and index in fields before indexing and subtracting |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | machine receives the row and index directly as parameters to index the array and subtract |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | the bounded element is returned from a contracted helper call result before subtracting |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms project the bounded element and join before subtracting |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | the element ceiling bound is governed by a named constant |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a successor machine receives the row and index to project the element and subtract |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | an intermediate forwarding machine passes the row and index to the projection machine |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each of four target bodies indexes the array and subtracts |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64`, carries an
authored `expected_native_exit.txt` (`0`), and declares `Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/nested_array_element_projection/main.omg
omega run --both samples/cli/facts/nested_array_element_projection/local/main.omg
omega run --both samples/cli/facts/nested_array_element_projection/field/main.omg
omega run --both samples/cli/facts/nested_array_element_projection/parameter/main.omg
omega run --both samples/cli/facts/nested_array_element_projection/call_result/main.omg
omega run --both samples/cli/facts/nested_array_element_projection/match_join/main.omg
omega run --both samples/cli/facts/nested_array_element_projection/const_name/main.omg
omega run --both samples/cli/facts/nested_array_element_projection/other_state/main.omg
omega run --both samples/cli/facts/nested_array_element_projection/other_machine/main.omg
omega run --both samples/cli/facts/nested_array_element_projection/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- At runtime, `row = Row { cells: [10, 20, 50, 80] }` and `idx = 2`. The projected element is `row.cells[2] == 50u64`, and exact subtraction `80u64 - 50u64 == 30u64`. The transition `spare == 30 { true -> done() _ -> bad() }` exits 0. A 1-token perturbation of the demanded result (`transition spare == 30` to `transition spare == 31`) routes execution to `machine bad() { crash Trap; }`, trapping both engines (`native exit -1073741795` / `interp crash: Trap (host presentation agrees)`).
- In each invalid twin, the element bound is widened to `81` (allowing `elem == 81u64`). Exact subtraction `80u64 - elem` (`80u64 - 81u64`) underflows below zero, causing compile-time proof obligation checking under the **Exact arithmetic** obligation class to fail closed with `exact arithmetic in machine ... may overflow \`u64\``. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/nested_array_element_projection/<place>/invalid/main.omg
```

| Place | Reject Twin | Obligation Class | Authored Diagnostic Fragment |
| --- | --- | --- | --- |
| local | [`local/invalid/main.omg`](local/invalid/main.omg) | Exact arithmetic | `may overflow \`u64\`` |
| field | [`field/invalid/main.omg`](field/invalid/main.omg) | Exact arithmetic | `may overflow \`u64\`` |
| parameter | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | Exact arithmetic | `may overflow \`u64\`` |
| call result | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | Exact arithmetic | `may overflow \`u64\`` |
| match join | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | Exact arithmetic | `may overflow \`u64\`` |
| const name | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | Exact arithmetic | `may overflow \`u64\`` |
| a nested machine | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | Exact arithmetic | `may overflow \`u64\`` |
| another machine | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | Exact arithmetic | `may overflow \`u64\`` |
| another target body | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | Exact arithmetic | `may overflow \`u64\`` |

### Acceptance Gate

```sh
python tools/sample_native_gate.py --filter cli/facts/nested_array_element_projection --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64` (and `linux_x86_64`).
- 9/9 intended-invalid controls match their authored rejection (`may overflow \`u64\``).

## Pinned Fixtures

All 18 positive and reject places are pinned in the compiler corpus:
- **Pass fixtures (`expected_native_exit.txt` = `0`):**
  - `tests/omega/pass/facts/nested_array_element_projection_through_local_exit/`
  - `tests/omega/pass/facts/nested_array_element_projection_through_field_exit/`
  - `tests/omega/pass/facts/nested_array_element_projection_through_parameter_exit/`
  - `tests/omega/pass/facts/nested_array_element_projection_through_call_result_exit/`
  - `tests/omega/pass/facts/nested_array_element_projection_through_match_join_exit/`
  - `tests/omega/pass/facts/nested_array_element_projection_through_const_name_exit/`
  - `tests/omega/pass/facts/nested_array_element_projection_through_other_state_exit/`
  - `tests/omega/pass/facts/nested_array_element_projection_through_other_machine_exit/`
  - `tests/omega/pass/facts/nested_array_element_projection_through_target_body_exit/`
- **Fail fixtures (`expected.txt`):**
  - `tests/omega/fail/facts/nested_array_element_projection_through_local_rejected/`
  - `tests/omega/fail/facts/nested_array_element_projection_through_field_rejected/`
  - `tests/omega/fail/facts/nested_array_element_projection_through_parameter_rejected/`
  - `tests/omega/fail/facts/nested_array_element_projection_through_call_result_rejected/`
  - `tests/omega/fail/facts/nested_array_element_projection_through_match_join_rejected/`
  - `tests/omega/fail/facts/nested_array_element_projection_through_const_name_rejected/`
  - `tests/omega/fail/facts/nested_array_element_projection_through_other_state_rejected/`
  - `tests/omega/fail/facts/nested_array_element_projection_through_other_machine_rejected/`
  - `tests/omega/fail/facts/nested_array_element_projection_through_target_body_rejected/`
The positive fixtures bind `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and record native exit 0.

## Requirements and Evidence

- **Parent outcome:** `LAYOUT-PLANS` / `STRUCTURAL-ACCESS` / `CONSERVATION-CONTRACT` (`TASKS.md`).
- **Ticket:** `28110acf535b48eab1a1b42bb7223a1d`.
- **Requirement established:** Safe element projection from a fixed-size array field inside a structural record (`row.cells[idx]`) with a bounded index (`idx < 4u64`) and preservation of element bounds (`elem <= 80u64`) to discharge exact arithmetic (`80u64 - elem`) without overflow across all nine places, with exact agreement between Terminal Psi and native execution across hosted targets (`omega run --both` exit 0).

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Construction of the structural record `Row { cells: [10, 20, 50, 80] }` and precondition contract `idx < 4 && row.cells[idx] <= 80` establish the array index validity (`idx < 4`) and element interval (`row.cells[idx] <= 80`). Safe projection `let elem: u64 = row.cells[idx]` introduces the element value with verified interval `[0, 80]`, directly discharging exact subtraction `80u64 - elem` without requiring post-hoc caller equality guards.
  - **Identity and lifetime:** Fixed-size array elements within structural records (`Row.cells[idx]`) retain their exact byte layout, value identity, and interval bounds across copies, field nestings, explicit parameters, call return results, match join branches, named constants, state transitions, machine calls, and target bodies. Accessing an element does not invalidate untouched sibling cells (`cells[0] == 10`), preserving structural value semantics (`[copy]`).
  - **Composition:** The element projection and bound fact compose through all 9 fact places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming exact subtraction operation (`80u64 - elem`). In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the element bound (`<= 80` vs `<= 81`), preserving all other structures, array lengths, and indexing expressions.
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed during compile-time proof obligation checking when the element bound is widened to `81` (allowing `elem == 81`), since `80u64 - 81u64` underflows `u64`. Compilation strictly rejects with `exact arithmetic in machine ... may overflow \`u64\``. No unproved underflow or out-of-bounds indexing reaches native execution.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Fixed-size array record field projections (`row.cells[idx]`), index bounds checking (`idx < 4`), and exact integer subtraction lower into canonical target-neutral Terminal Psi (`PSITERM\0`), erasing mathematical proof obligations while retaining array element byte offsets (`elem_offset = idx * 8`). All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) through companion build declarations without reopening source. The composite structural record layout validates: `Row` contains `cells: [u64; 4]` (size 32, align 8), with contiguous 8-byte element slots at relative offsets `[0, 8, 16, 24]`. Target `CallPlan`/`StatePlan` assign arguments and return homes across register lanes, stack slots, or indirect aggregate return homes (`sret`) per platform ABI.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects scaled-index memory loads (`[base + idx * 8]` or indexed addressing modes on `x86_64`/`arm64`), register moves, and 64-bit integer subtraction (`sub`). Low-level register allocators assign register homes and disjoint activation stack-frame slots for array records, and entry prologues and multi-return epilogues honor the `StatePlan` ceiling.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic stack-frame displacements and indexed array element addresses encode into target ISA machine bytes, basic blocks and jump targets relax with offset reflow, machine-state footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates against target host limits, and direct PE/ELF/Mach-O executables are emitted. Terminal Psi interpretation and native execution agree with exit 0 (`omega run --both`) across host platforms including `windows_x86_64` and `macos_arm64`.

- **What remains open:** Dynamic array slice projections (`&row.cells[start..end]`), multi-dimensional nested array stride indexing, and mutable slice borrows (`&mut row.cells[idx]`).
