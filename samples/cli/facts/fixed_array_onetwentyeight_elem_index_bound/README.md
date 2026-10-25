# Fixed Array One Hundred Twenty-Eight Element Index Bound

a static index bound `idx < 128u64` carried through the place proves safe fixed-size array indexing `arr[idx]` on `[u64; 128]`.

Every place performs the same operation: define a 128-element fixed-size array `arr: [u64; 128] = [0, 1, 2, ..., 127]`, carry an in-bounds index `idx < 128u64` (specifically index `73u64`) through the place, read element `arr[idx]`, and verify in `main` that `elem == 73u64` transitioning to `done()` exiting 0. Only the transport mechanism of the bounded index and array changes across places.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | - | root fixed-size array indexing evaluation |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | the array and bounded index are copied into locals before indexing |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | a record field carries the bounded index alongside the fixed-size array |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | the reading machine receives the bounded index and array directly as parameters |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | the index arrives from a helper call with an ensures result < 128 contract |
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
omega run --both samples/cli/facts/fixed_array_onetwentyeight_elem_index_bound/main.omg
omega run --both samples/cli/facts/fixed_array_onetwentyeight_elem_index_bound/local/main.omg
omega run --both samples/cli/facts/fixed_array_onetwentyeight_elem_index_bound/field/main.omg
omega run --both samples/cli/facts/fixed_array_onetwentyeight_elem_index_bound/parameter/main.omg
omega run --both samples/cli/facts/fixed_array_onetwentyeight_elem_index_bound/call_result/main.omg
omega run --both samples/cli/facts/fixed_array_onetwentyeight_elem_index_bound/match_join/main.omg
omega run --both samples/cli/facts/fixed_array_onetwentyeight_elem_index_bound/const_name/main.omg
omega run --both samples/cli/facts/fixed_array_onetwentyeight_elem_index_bound/other_state/main.omg
omega run --both samples/cli/facts/fixed_array_onetwentyeight_elem_index_bound/other_machine/main.omg
omega run --both samples/cli/facts/fixed_array_onetwentyeight_elem_index_bound/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- In every positive place, evaluating fixed-size array indexing on `[u64; 128]` with index `73u64` statically verified to satisfy `idx < 128u64` retrieves element `73u64`. In `main`, `transition elem == 73` transitions to `machine done() {}`, terminating with native exit 0. If indexing produces any unexpected value, execution branches to `machine bad() { crash Trap; }`, causing a Trap crash.
- Measured discrimination: perturbing `elem == 73` to `elem == 74` on `main.omg` causes native execution to crash with STATUS_ILLEGAL_INSTRUCTION (-1073741795) and the Terminal Psi interpreter to crash with `Trap (host presentation agrees)`. Both engines trap and reject the perturbation.
- Each invalid twin widens the index bound contract to `idx <= 128` (or `LIMIT = 129`), so the index may reach or exceed length 128. Fixed-size array indexing `arr[idx]` fails closed at compile time with `cannot prove index ... is within length 128`. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/fixed_array_onetwentyeight_elem_index_bound/<place>/invalid/main.omg
```

### Acceptance Gate

```sh
python tools/sample_native_gate.py --filter cli/facts/fixed_array_onetwentyeight_elem_index_bound
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64`.
- 9/9 intended-invalid controls match their authored rejection (`cannot prove index`).

## Pinned Fixtures

The row is pinned under:
- `tests/omega/pass/facts/fixed_array_onetwentyeight_elem_index_bound_through_<place>_exit/`
- `tests/omega/fail/facts/fixed_array_onetwentyeight_elem_index_bound_through_<place>_rejected/`

The positive fixtures bind `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and record native exit 0.

## Requirements and Evidence

- **Parent outcome:** `LAYOUT-PLANS` / `COUNTS-AND-ADDRESSES` / `STRUCTURAL-ACCESS` ([TASKS.md](../../../../TASKS.md)).
- **Requirement established:** End-to-end nine-place fact preservation and native realization for 128-element fixed-size array indexing (`[u64; 128]`, `[layout plans](wiki/spec/layouts/plans.md)`). Specifically, a static index bound `idx < 128u64` carried through the place proves safe fixed-size array indexing `arr[idx]` on `[u64; 128]` without out-of-bounds access across all nine places.
- **Obligation class:** Layout geometry preservation and array index boundary verification across transport places.

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Fixed-size 128-element array `[u64; 128]` literal construction and static entry precondition `requires idx < 128` (or record constraint `where idx < 128` / named constant `LIMIT = 128`) establish proven in-bounds access before indexing `arr[idx]`. `omega run --both` reports native exit 0 and interpreter exit 0 agreeing for all nine places.
  - **Identity and lifetime:** The independent element values are conserved across all value transfers and transitions. Accessing element `arr[73]` yields `73u64`, verified by transition guard `elem == 73` transitioning to `done()` with exit code 0.
  - **Composition:** Machine calls, pattern match joins, and state transitions preserve exact preconditions and actual values without narrowing or corruption. In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the index bound (`idx < 128` vs `idx <= 128` or `LIMIT = 128` vs `LIMIT = 129`), leaving the indexing obligation unproved without any caller guard difference:
    - `local`: `arr` and `idx` are copied into local variables before indexing.
    - `field`: `arr` and `idx` are packaged into `Container where idx < 128` and projected.
    - `parameter`: `arr` and `idx` are passed as machine arguments under `requires idx < 128`.
    - `call_result`: `supply_index()` returns `idx` under `ensures result < 128`.
    - `match_join`: `idx` converges across boolean match arms into a safe in-bounds index.
    - `const_name`: `LIMIT = 128` bounds the entry contract.
    - `other_state`: `arr` and `idx` transition to state `element(passed_arr, passed_idx)`.
    - `other_machine`: `fetch_element()` forwards the bounded indexing operation.
    - `target_body`: target-scoped machine bodies for all four hosted targets (`linux_x86_64`, `linux_arm64`, `windows_x86_64`, `macos_arm64`) perform the read.
  - **Soundness boundaries:** All nine reject twins under `<place>/invalid/` reject during compile-time verification (`omega --check`) with `error: cannot prove index ... is within length 128`, confirming that out-of-bounds indexing is strictly prevented across all nine places.
  - **Checked-to-Terminal Psi lowering (`Psi 05-07`):** Source constructs lower into canonical `PSITERM\0` target-neutral representations, erasing mathematical bounds while emitting verified array projection operations with statically validated length claims. Both Terminal Psi interpretation and native execution agree (`omega run --both` reports exit 0).
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00-02`):** Target realization joins Build companion rows (`ProgramEntry`) across the four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Target layout plans validate 1024-byte contiguous memory layout (`128 * 8` bytes, 8-byte aligned), while calling plans assign stack homes or indirect destination pointers (`sret`/by-ref) appropriate for 1024-byte aggregate values.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03-06`):** Backend lowering selects scaled-index addressing modes (e.g. `[base + idx * 8]` on `x86_64` and `[base, idx, lsl #3]` on `arm64`). Activation frames allocate disjoint stack storage for the 1024-byte array honoring the `StatePlan` ceiling.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07-09` + emission):** Direct PE/ELF/Mach-O images are emitted with valid machine-state footprint evidence, executing natively and agreeing with Terminal Psi (`omega run --both` reports `native exit 0` and `interp exit: 0`) across all hosted targets.

- **What remains open:** Multi-dimensional array indexing bounds (`[[u64; 128]; 128]`), dynamically-sized slice indexing with runtime slice headers, and mutable array element writes (`arr[idx] = val`).
