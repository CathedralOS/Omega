# Fixed Array Thirty-Two Element Index Bound

a static index bound `idx < 32u64` carried through the place proves safe fixed-size array indexing `arr[idx]` on `[u64; 32]`.

Every place performs the same operation: define a 32-element fixed-size array `arr: [u64; 32] = [0, 1, 2, ..., 31]`, carry an in-bounds index `idx < 32u64` (specifically index `25u64`) through the place, read element `arr[idx]`, and verify in `main` that `elem == 25u64` transitioning to `done()` exiting 0. Only the transport mechanism of the bounded index and array changes across places.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | root fixed-size array indexing evaluation |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | the array and bounded index are copied into locals before indexing |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | a record field carries the bounded index alongside the fixed-size array |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | the reading machine receives the bounded index and array directly as parameters |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | the index arrives from a helper call with an ensures result < 32 contract |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both branches of a match expression produce in-bounds indices |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | a named constant bounds the index contract for the fixed-size array |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a transition to a successor machine forwards the bounded index and array |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | a forwarding machine transports the bounded index and array to the reader |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each of four target bodies reads the array element using the bounded index |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` and declares
`Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/fixed_array_thirtytwo_elem_index_bound/main.omg
omega run --both samples/cli/facts/fixed_array_thirtytwo_elem_index_bound/local/main.omg
omega run --both samples/cli/facts/fixed_array_thirtytwo_elem_index_bound/field/main.omg
omega run --both samples/cli/facts/fixed_array_thirtytwo_elem_index_bound/parameter/main.omg
omega run --both samples/cli/facts/fixed_array_thirtytwo_elem_index_bound/call_result/main.omg
omega run --both samples/cli/facts/fixed_array_thirtytwo_elem_index_bound/match_join/main.omg
omega run --both samples/cli/facts/fixed_array_thirtytwo_elem_index_bound/const_name/main.omg
omega run --both samples/cli/facts/fixed_array_thirtytwo_elem_index_bound/other_state/main.omg
omega run --both samples/cli/facts/fixed_array_thirtytwo_elem_index_bound/other_machine/main.omg
omega run --both samples/cli/facts/fixed_array_thirtytwo_elem_index_bound/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- In every positive place, evaluating fixed-size array indexing on `[u64; 32]` with index `25u64` statically verified to satisfy `idx < 32u64` retrieves element `25u64`. In `main`, `transition elem == 25` transitions to `machine done() {}`, terminating with native exit 0. If indexing produces any unexpected value, execution branches to `machine bad() { crash Trap; }`, causing a Trap crash.
- Measured discrimination: perturbing `elem == 25` to `elem == 26` on `main.omg` causes native execution to crash with STATUS_ILLEGAL_INSTRUCTION (-1073741795) and the Terminal Psi interpreter to crash with `Trap (host presentation agrees)`. Both engines trap and reject the perturbation.
- Each invalid twin widens the index bound contract to `idx <= 32` (or `LIMIT = 33`), so the index may reach or exceed length 32. Fixed-size array indexing `arr[idx]` fails closed at compile time with `cannot prove index ... is within length 32`. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/fixed_array_thirtytwo_elem_index_bound/<place>/invalid/main.omg
```

### Acceptance Gate

```sh
python tools/sample_native_gate.py --filter cli/facts/fixed_array_thirtytwo_elem_index_bound
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64`.
- 9/9 intended-invalid controls match their authored rejection (`cannot prove index`).

## Pinned Fixtures

The row is pinned under:
- `tests/omega/pass/facts/fixed_array_thirtytwo_elem_index_bound_through_<place>_exit/`
- `tests/omega/fail/facts/fixed_array_thirtytwo_elem_index_bound_through_<place>_rejected/`

The positive fixtures bind `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and record native exit 0.

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` / `TERMINAL-CONTENT-CLAIMS` (`TASKS.md`).
- **Requirement established:** End-to-end nine-place fact preservation and native realization for 32-element fixed-size array indexing (`[u64; 32]`, `[layout plans](wiki/spec/layouts/plans.md)`). Specifically, a static index bound `idx < 32u64` carried through the place proves safe fixed-size array indexing `arr[idx]` on `[u64; 32]` without out-of-bounds access across all nine places.
- **Obligation class:** Layout geometry preservation and array index boundary verification across transport places.

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Static requires precondition `requires idx < 32` establishes that `idx` is strictly less than the length 32 of `[u64; 32]`. The index bound is checked and verified at compile time before any array dereference occurs.
  - **Identity and lifetime:** The fixed-size array values and bounded index are preserved across local variables, struct fields, machine parameters, return results, and control flow joins throughout their lifespans.
  - **Composition:** The static index bound `idx < 32u64` composes through all 9 fact places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming indexing operation (`arr[idx]`). In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the index bound (`idx < 32` vs `idx <= 32` or `LIMIT = 32` vs `LIMIT = 33`), leaving the indexing obligation unproved without any caller guard difference.
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed at compile time when `idx <= 32` (or `LIMIT = 33`) is specified: the index bound permits `idx == 32`, which violates the fixed-size array bound `idx < 32`, strictly rejecting with `cannot prove index ... is within length 32`. No unproved out-of-bounds array access reaches native execution.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Array indexing operations lower into canonical target-neutral Terminal Psi (`PSITERM\0`), erasing mathematical proof obligations while preserving contiguous element indexing layout and bounds evidence. All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Companion build bindings (`ProgramEntry`) settle across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) without reopening source. The array layout plan (`[u64; 32]` size 256 bytes, alignment 8) validates, and target `CallPlan`/`StatePlan` assign array aggregate references and element index parameters across target registers and stack slots per ABI.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA indexed memory operands (e.g. `mov r64, [base + index * 8]` on `x86_64`; `ldr x, [base, index, lsl #3]` on `arm64`), assigns register homes and disjoint activation frame storage, and inserts entry prologues and multi-return epilogues honoring the `StatePlan`.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Target ISA instructions encode into machine bytes (SIB byte addressing on `x86_64`), basic blocks and branch offsets relax with offset reflow, machine-state footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates against target host limits, and direct PE/ELF/Mach-O executables are emitted. Terminal Psi interpretation and native execution agree with exit 0 (`omega run --both`) across host platforms including `windows_x86_64` and `macos_arm64`.

- **What remains open:** Multi-dimensional array indexing bounds (`[[u64; 32]; 32]`), dynamically-sized slice indexing with runtime slice headers, and mutable array element writes (`arr[idx] = val`).
