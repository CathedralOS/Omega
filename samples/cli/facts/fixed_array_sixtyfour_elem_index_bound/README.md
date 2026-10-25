# Fixed Array Sixty-Four Element Index Bound

a static index bound `idx < 64u64` carried through the place proves safe fixed-size array indexing `arr[idx]` on `[u64; 64]`.

Every place performs the same operation: define a 64-element fixed-size array `arr: [u64; 64] = [0, 1, 2, ..., 63]`, carry an in-bounds index `idx < 64u64` (specifically index `42u64`) through the place, read element `arr[idx]`, and verify in `main` that `elem == 42u64` transitioning to `done()` exiting 0. Only the transport mechanism of the bounded index and array changes across places.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | root fixed-size array indexing evaluation |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | the array and bounded index are copied into locals before indexing |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | a record field carries the bounded index alongside the fixed-size array |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | the reading machine receives the bounded index and array directly as parameters |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | the index arrives from a helper call with an ensures result < 64 contract |
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
omega run --both samples/cli/facts/fixed_array_sixtyfour_elem_index_bound/main.omg
omega run --both samples/cli/facts/fixed_array_sixtyfour_elem_index_bound/local/main.omg
omega run --both samples/cli/facts/fixed_array_sixtyfour_elem_index_bound/field/main.omg
omega run --both samples/cli/facts/fixed_array_sixtyfour_elem_index_bound/parameter/main.omg
omega run --both samples/cli/facts/fixed_array_sixtyfour_elem_index_bound/call_result/main.omg
omega run --both samples/cli/facts/fixed_array_sixtyfour_elem_index_bound/match_join/main.omg
omega run --both samples/cli/facts/fixed_array_sixtyfour_elem_index_bound/const_name/main.omg
omega run --both samples/cli/facts/fixed_array_sixtyfour_elem_index_bound/other_state/main.omg
omega run --both samples/cli/facts/fixed_array_sixtyfour_elem_index_bound/other_machine/main.omg
omega run --both samples/cli/facts/fixed_array_sixtyfour_elem_index_bound/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- In every positive place, evaluating fixed-size array indexing on `[u64; 64]` with index `42u64` statically verified to satisfy `idx < 64u64` retrieves element `42u64`. In `main`, `transition elem == 42` transitions to `machine done() {}`, terminating with native exit 0. If indexing produces any unexpected value, execution branches to `machine bad() { crash Trap; }`, causing a Trap crash.
- Measured discrimination: perturbing `elem == 42` to `elem == 43` on `main.omg` causes native execution to crash with STATUS_ILLEGAL_INSTRUCTION (-1073741795) and the Terminal Psi interpreter to crash with `Trap (host presentation agrees)`. Both engines trap and reject the perturbation.
- Each invalid twin widens the index bound contract to `idx <= 64` (or `LIMIT = 65`), so the index may reach or exceed length 64. Fixed-size array indexing `arr[idx]` fails closed at compile time with `cannot prove index ... is within length 64`. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/fixed_array_sixtyfour_elem_index_bound/<place>/invalid/main.omg
```

### Acceptance Gate

```sh
python tools/sample_native_gate.py --filter cli/facts/fixed_array_sixtyfour_elem_index_bound
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64`.
- 9/9 intended-invalid controls match their authored rejection (`cannot prove index`).

## Pinned Fixtures

The row is pinned under:
- `tests/omega/pass/facts/fixed_array_sixtyfour_elem_index_bound_through_<place>_exit/`
- `tests/omega/fail/facts/fixed_array_sixtyfour_elem_index_bound_through_<place>_rejected/`

The positive fixtures bind `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and record native exit 0.

## Requirements and Evidence

- **Parent outcome:** `LAYOUT-PLANS` / `COUNTS-AND-ADDRESSES` / `STRUCTURAL-ACCESS` / `CONSERVATION-CONTRACT` / `TERMINAL-CONTENT-CLAIMS` ([`TASKS.md`](../../../../TASKS.md)).
- **Requirement established:** End-to-end nine-place fact preservation and native realization for 64-element fixed-size array indexing (`[u64; 64]`, `[layout plans](wiki/spec/layouts/plans.md)`). Specifically, a static index bound `idx < 64u64` carried through the place proves safe fixed-size array indexing `arr[idx]` on `[u64; 64]` without out-of-bounds access across all nine transport places.
- **Obligation class:** Layout geometry preservation and array index boundary verification across transport places.

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Static precondition `requires idx < 64` (or record constraint `where idx < 64` / named constant `LIMIT = 64`) establishes that `idx` is strictly less than the length 64 of `[u64; 64]`. The index bound is checked and verified at compile time before any array dereference occurs.
  - **Identity and lifetime:** The independent element values and bounded index are conserved across all value transfers and transitions in all nine places (local variables, nominal record fields, parameter bindings, function return contracts, branch joins, constant references, state arguments, cross-machine calls, and target bodies). Accessing element `arr[42]` yields `42u64`, verified by transition guard `elem == 42` transitioning to `done()` exiting 0.
  - **Composition:** The static index bound `idx < 64u64` composes through all 9 fact places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming indexing operation (`arr[idx]`). In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the index bound (`idx < 64` vs `idx <= 64` or `LIMIT = 64` vs `LIMIT = 65`), leaving the indexing obligation unproved without any caller guard difference.
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed at compile time when `idx <= 64` (or `LIMIT = 65`) is specified: the index bound permits `idx == 64`, which violates the fixed-size array bound `idx < 64`, strictly rejecting with `cannot prove index ... is within length 64`. No unproved out-of-bounds array access reaches native execution.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Source constructs lower into canonical `PSITERM\0` target-neutral representations, erasing mathematical bounds while emitting verified array projection operations with statically validated length claims. Both Terminal Psi interpretation and native execution agree (`omega run --both` reports exit 0).
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Companion build bindings (`ProgramEntry`) settle across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) without reopening source. Target layout plans validate 512-byte contiguous memory layout (`64 * 8` bytes, 8-byte aligned), while calling plans assign stack homes or indirect destination pointers (`sret`/by-ref) appropriate for 512-byte aggregate values.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects scaled-index addressing modes (e.g. `[base + idx * 8]` on `x86_64` and `[base, idx, lsl #3]` on `arm64`). Activation frames allocate disjoint stack storage for the 512-byte array honoring the `StatePlan` ceiling.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Direct PE/ELF/Mach-O images are emitted with valid machine-state footprint evidence, executing natively and agreeing with Terminal Psi (`omega run --both` reports `native exit 0` and `interp exit: 0`) across all hosted targets.

- **What remains open:** Dynamically sized slice views `&[u64]`, multidimensional array indexing layout plans, and mutable array element writes through bounded indices.
